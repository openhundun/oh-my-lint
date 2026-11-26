//! SQL 断行规则：检查与增量修复。

mod astmap;
mod checker;
mod ddl;
mod ddl_misc;
mod decision;
mod dml;
mod fix;
mod lexer;
mod parse;
mod splitter;
mod token;

pub use parse::Dialect;

use crate::sql::parse::StatementKind;

/// 检查一段 SQL（Postgres 方言），返回块内坐标的诊断。
pub fn check(sql: &str) -> Vec<crate::model::Finding> {
    check_with(sql, Dialect::Postgres)
}

/// 按指定方言检查；无法解析的语句跳过，不报 finding。
pub fn check_with(sql: &str, dialect: Dialect) -> Vec<crate::model::Finding> {
    crate::sql::lexer::with_dialect(dialect, || check_with_inner(sql, dialect))
}

fn check_with_inner(sql: &str, dialect: Dialect) -> Vec<crate::model::Finding> {
    if crate::sql::lexer::has_comment(sql) {
        return Vec::new();
    }
    let mut findings = Vec::new();
    for span in crate::sql::splitter::statements(sql) {
        let statement = &sql[span.start..span.end];
        let Ok(kind) = crate::sql::parse::classify_one(statement, dialect) else {
            continue;
        };
        match kind {
            StatementKind::Query => {
                findings.extend(
                    crate::sql::checker::violations_with(statement, dialect)
                        .into_iter()
                        .map(|violation| {
                            finding(violation.message, violation.span.shift(span.start))
                        }),
                );
                findings.extend(
                    crate::sql::token::issues(statement)
                        .into_iter()
                        .map(|issue| finding(issue.message, issue.span.shift(span.start))),
                );
            }
            StatementKind::Other => {}
            StatementKind::Delete
            | StatementKind::Explain
            | StatementKind::CreateTable
            | StatementKind::CreateIndex
            | StatementKind::AlterTable => {
                findings.extend(
                    crate::sql::token::issues(statement)
                        .into_iter()
                        .map(|issue| finding(issue.message, issue.span.shift(span.start))),
                );
                if layout(kind, statement, dialect).is_some_and(|output| output != statement) {
                    findings.push(finding(format!("{} layout differs", label(kind)), span));
                }
            }
        }
    }
    findings
}

/// 一条块内诊断；语句级 span 已加基址。
fn finding(message: String, span: crate::span::Span) -> crate::model::Finding {
    crate::model::Finding {
        rule: "sql-break".to_owned(),
        severity: crate::model::Severity::Warning,
        message,
        span,
        fix: None,
    }
}

/// 派发版式模块；`None` 表示该门类没有模块能渲染。
fn layout(kind: StatementKind, sql: &str, dialect: Dialect) -> Option<String> {
    match kind {
        StatementKind::Delete | StatementKind::Explain => crate::sql::dml::layout(sql, dialect),
        StatementKind::CreateTable => crate::sql::ddl::layout(sql, dialect)
            .or_else(|| crate::sql::ddl_misc::layout(sql, dialect)),
        StatementKind::CreateIndex | StatementKind::AlterTable => {
            crate::sql::ddl_misc::layout(sql, dialect)
        }
        StatementKind::Query | StatementKind::Other => None,
    }
}

fn label(kind: StatementKind) -> &'static str {
    match kind {
        StatementKind::Query => "query",
        StatementKind::Delete => "delete",
        StatementKind::Explain => "explain",
        StatementKind::CreateTable => "create table",
        StatementKind::CreateIndex => "create index",
        StatementKind::AlterTable => "alter table",
        StatementKind::Other => "other",
    }
}

/// 把一段 SQL 修成房规版式（Postgres 方言）；合规输入原样返回。
pub fn format(sql: &str) -> crate::error::Result<String> {
    format_with(sql, Dialect::Postgres)
}

/// 按指定方言格式化；无法解析的语句逐字节保留。
pub fn format_with(sql: &str, dialect: Dialect) -> crate::error::Result<String> {
    crate::sql::lexer::with_dialect(dialect, || format_with_inner(sql, dialect))
}

fn format_with_inner(sql: &str, dialect: Dialect) -> crate::error::Result<String> {
    if crate::sql::lexer::has_comment(sql) {
        return Err(crate::error::Error::Rejected(
            "comments are not supported in fix mode".to_owned(),
        ));
    }
    let spans = crate::sql::splitter::statements(sql);
    let mut output = sql.to_owned();
    let mut last_processed = false;
    for (index, span) in spans.iter().enumerate().rev() {
        let statement = &sql[span.start..span.end];
        let Some(replacement) = format_statement(statement, dialect) else {
            continue;
        };
        output.replace_range(span.start..span.end, &replacement);
        if index + 1 == spans.len() {
            last_processed = true;
        }
    }
    if last_processed && let Some(last) = spans.last() {
        // 末尾字节不属于任何语句，替换后位置不变；据此换算最后一条语句的终点。
        let end = output.len() - (sql.len() - last.end);
        output = drop_final_semicolon(output, end);
    }
    Ok(output)
}

/// 渲染一条语句；`None` 表示跳过该语句、逐字节保留原文。
fn format_statement(statement: &str, dialect: Dialect) -> Option<String> {
    let kind = crate::sql::parse::classify_one(statement, dialect).ok()?;
    let output = match kind {
        StatementKind::Query => {
            let normalized = crate::sql::token::normalize(statement);
            let formatted = crate::sql::fix::format_with(&normalized, dialect).ok()?;
            crate::sql::token::normalize(&formatted)
        }
        StatementKind::Delete
        | StatementKind::Explain
        | StatementKind::CreateTable
        | StatementKind::CreateIndex
        | StatementKind::AlterTable => layout(kind, statement, dialect)?,
        StatementKind::Other => return None,
    };
    crate::sql::parse::classify_one(&output, dialect).ok()?;
    Some(output)
}

/// R26：最后一条语句被处理时，切掉语句之后的尾分号与其间、之后的空白。
fn drop_final_semicolon(mut text: String, end: usize) -> String {
    let has_semicolon = text
        .get(end..)
        .is_some_and(|tail| tail.trim_end().ends_with(';'));
    if has_semicolon {
        text.truncate(end);
    }
    text
}

/// 规则入口：`sql-break`。
pub struct SqlRule {
    dialect: Dialect,
}

impl SqlRule {
    /// 用指定解析方言构造规则。
    pub fn new(dialect: Dialect) -> Self {
        Self { dialect }
    }
}

impl Default for SqlRule {
    fn default() -> Self {
        Self::new(Dialect::Postgres)
    }
}

impl crate::model::Rule for SqlRule {
    fn id(&self) -> &'static str {
        "sql-break"
    }

    fn description(&self) -> &'static str {
        "SQL 断行：子句与元素按数量/复杂度决定内联或逐元素换行"
    }

    fn check(&self, block: &crate::model::Block) -> Vec<crate::model::Finding> {
        crate::sql::check_with(&block.text, self.dialect)
    }

    fn fix(&self, block: &crate::model::Block) -> Option<crate::model::Fix> {
        let formatted = crate::sql::format_with(&block.text, self.dialect).ok()?;
        if formatted == block.text {
            return None;
        }
        Some(crate::model::Fix {
            edits: vec![crate::model::TextEdit {
                span: crate::span::Span::new(0, block.text.len()),
                replacement: formatted,
            }],
            scope: crate::model::FixScope::Local,
            safety: crate::model::FixSafety::Safe,
        })
    }

    fn can_fix(&self, block: &crate::model::Block) -> crate::error::Result<()> {
        crate::sql::lexer::with_dialect(self.dialect, || {
            if crate::sql::lexer::has_comment(&block.text) {
                return Err(crate::error::Error::Rejected(
                    "comments are not supported in fix mode".to_owned(),
                ));
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests;
