//! SQL 断行规则：检查与增量修复。

mod astmap;
mod checker;
mod comments;
mod ddl;
mod ddl_misc;
mod decision;
mod dml;
mod fix;
mod groups;
mod lexer;
mod parse;
mod splitter;
mod token;
mod values;

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
    let mut findings = Vec::new();
    let spans = crate::sql::splitter::statements(sql);
    let mut last_kind = None;
    for span in &spans {
        let statement = &sql[span.start..span.end];
        last_kind = crate::sql::parse::classify_one(statement, dialect).ok();
        let Some(kind) = last_kind else {
            continue;
        };
        if crate::sql::lexer::has_comment(statement) {
            // 注释块不做结构判定；保留注释，只跑能安全落地的 token 规则。
            if token_only(statement, dialect).is_some() {
                findings.extend(crate::sql::token::issues(statement).into_iter().map(|issue| finding(issue.message, issue.span.shift(span.start))));
            }
            continue;
        }
        match kind {
            StatementKind::Query => {
                findings.extend(
                    crate::sql::checker::violations_with(statement, dialect)
                        .into_iter()
                        .map(|violation| finding(violation.message, violation.span.shift(span.start))),
                );
                findings.extend(token_findings(statement, *span, dialect));
            }
            StatementKind::Other => {
                findings.extend(token_findings(statement, *span, dialect));
            }
            StatementKind::Delete | StatementKind::Explain | StatementKind::CreateTable | StatementKind::CreateIndex | StatementKind::AlterTable | StatementKind::Insert => {
                findings.extend(token_findings(statement, *span, dialect));
                if layout(kind, statement, dialect).is_some_and(|output| {
                    let output = comments::trim_trailing_spaces(&output, dialect);
                    let output = if kind == StatementKind::Insert { values::format(&output, dialect) } else { output };
                    if kind == StatementKind::CreateIndex {
                        // 同组索引的 `on` 对齐会引入额外空格；结构错误仍由 token 规则报告。
                        comparable(&output) != comparable(statement)
                    } else {
                        output != statement
                    }
                }) {
                    findings.push(finding(format!("{} layout differs", label(kind)), *span));
                }
            }
        }
    }
    let last_has_comment = spans.last().is_some_and(|span| crate::sql::lexer::has_comment(&sql[span.start..span.end]));
    if let Some(last) = spans.last()
        && last_kind.is_some()
        && !last_has_comment
        && !has_terminating_semicolon(sql, last.end)
    {
        findings.push(finding("statement must end with a semicolon".to_owned(), crate::span::Span::empty(last.end)));
    }
    findings
}

/// 忽略空白宽度差异的比较键。
fn comparable(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 语句之后（允许空白）是否紧接分号。
fn has_terminating_semicolon(sql: &str, end: usize) -> bool {
    sql.get(end..).is_some_and(|tail| tail.trim_start().starts_with(';'))
}

/// 只报告 token 规范化后仍能通过 sqlparser 的问题；否则修复必然被跳过。
fn token_findings(statement: &str, span: crate::span::Span, dialect: Dialect) -> Vec<crate::model::Finding> {
    if token_only(statement, dialect).is_none() {
        return Vec::new();
    }
    crate::sql::token::issues(statement)
        .into_iter()
        .map(|issue| finding(issue.message, issue.span.shift(span.start)))
        .collect()
}

/// 一条块内诊断；语句级 span 已加基址。
fn finding(message: String, span: crate::span::Span) -> crate::model::Finding {
    crate::model::Finding { rule: "sql-break".to_owned(), severity: crate::model::Severity::Warning, message, span, fix: None }
}

/// 派发版式模块；`None` 表示该门类没有模块能渲染。
fn layout(kind: StatementKind, sql: &str, dialect: Dialect) -> Option<String> {
    match kind {
        StatementKind::Delete | StatementKind::Explain => crate::sql::dml::layout(sql, dialect),
        StatementKind::CreateTable => crate::sql::ddl::layout(sql, dialect).or_else(|| crate::sql::ddl_misc::layout(sql, dialect)),
        StatementKind::CreateIndex | StatementKind::AlterTable => crate::sql::ddl_misc::layout(sql, dialect),
        StatementKind::Insert => crate::sql::dml::layout(sql, dialect),
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
        StatementKind::Insert => "insert",
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
    let source = comments::strip_comments(sql, dialect);
    let spans = crate::sql::splitter::statements(&source);
    let mut output = source.clone();
    let mut last_processed = false;
    for (index, span) in spans.iter().enumerate().rev() {
        let statement = &source[span.start..span.end];
        let Some(replacement) = format_statement(statement, dialect) else {
            continue;
        };
        output.replace_range(span.start..span.end, &replacement);
        if index + 1 == spans.len() {
            last_processed = true;
        }
    }
    if last_processed
        && let Some(last) = spans.last()
        && !crate::sql::lexer::has_comment(&source[last.start..last.end])
    {
        // 末尾字节不属于任何语句，替换后位置不变；据此换算最后一条语句的终点。
        let end = output.len() - (source.len() - last.end);
        output = ensure_final_semicolon(output, end);
    }
    let output = values::format(&output, dialect);
    let output = groups::group_ddl(&output, dialect);
    let output = comments::format_comment_blocks(&output, dialect);
    let output = comments::trim_trailing_spaces(&output, dialect);
    Ok(comments::collapse_blank_lines(&output))
}

/// 渲染一条语句；`None` 表示跳过该语句、逐字节保留原文。
fn format_statement(statement: &str, dialect: Dialect) -> Option<String> {
    if crate::sql::lexer::has_comment(statement) {
        return token_only(statement, dialect).or_else(|| Some(statement.to_owned()));
    }
    let kind = crate::sql::parse::classify_one(statement, dialect).ok()?;
    let output = match kind {
        StatementKind::Query => {
            let mut current = crate::sql::token::normalize(statement);
            // token 规范化可能让原本布局器拒绝的表达式变得可布局；继续跑到不动点。
            for _ in 0..4 {
                let Ok(formatted) = crate::sql::fix::format_with(&current, dialect) else {
                    break;
                };
                let next = crate::sql::token::normalize(&formatted);
                if next == current {
                    break;
                }
                current = next;
            }
            current
        }
        StatementKind::Delete | StatementKind::Explain | StatementKind::CreateTable | StatementKind::CreateIndex | StatementKind::AlterTable | StatementKind::Insert => {
            layout(kind, statement, dialect).or_else(|| token_only(statement, dialect)).unwrap_or_else(|| statement.to_owned())
        }
        StatementKind::Other => token_only(statement, dialect).unwrap_or_else(|| statement.to_owned()),
    };
    if crate::sql::parse::classify_one(&output, dialect).is_err() {
        return token_only(statement, dialect);
    }
    Some(output)
}

/// DQL 布局器不支持的语句仍执行 token 规范化；结构版式保持原样。
fn token_only(statement: &str, dialect: Dialect) -> Option<String> {
    let normalized = crate::sql::token::normalize(statement);
    if normalized == statement {
        return None;
    }
    crate::sql::parse::classify_one(&normalized, dialect).ok()?;
    Some(normalized)
}

/// R26：最后一条语句被处理时，确保语句之后有一个分号。
fn ensure_final_semicolon(mut text: String, end: usize) -> String {
    if has_terminating_semicolon(&text, end) {
        return text;
    }
    text.insert(end, ';');
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
            edits: vec![crate::model::TextEdit { span: crate::span::Span::new(0, block.text.len()), replacement: formatted }],
            scope: crate::model::FixScope::Local,
            safety: crate::model::FixSafety::Safe,
        })
    }

    fn can_fix(&self, _block: &crate::model::Block) -> crate::error::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
