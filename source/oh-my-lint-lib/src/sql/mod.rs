//! SQL 断行规则：检查与增量修复。

mod checker;
mod decision;
mod fix;
mod lexer;
mod parse;
mod splitter;
mod token;

pub use parse::Dialect;

/// 检查一段 SQL（Generic 方言），返回块内坐标的诊断。
pub fn check(sql: &str) -> Vec<crate::model::Finding> {
    check_with(sql, Dialect::Generic)
}

/// 按指定方言检查；解析失败只报错误，不跑布局规则。
pub fn check_with(sql: &str, dialect: Dialect) -> Vec<crate::model::Finding> {
    if crate::sql::lexer::has_comment(sql) {
        return Vec::new();
    }
    if let Err(error) = crate::sql::parse::validate(sql, dialect) {
        return vec![crate::model::Finding {
            rule: "sql-break".to_owned(),
            severity: crate::model::Severity::Warning,
            message: format!("sql does not parse: {error}"),
            span: crate::span::Span::default(),
            fix: None,
        }];
    }
    let mut findings: Vec<crate::model::Finding> = crate::sql::checker::violations(sql)
        .into_iter()
        .map(|violation| crate::model::Finding {
            rule: "sql-break".to_owned(),
            severity: crate::model::Severity::Warning,
            message: violation.message,
            span: violation.span,
            fix: None,
        })
        .collect();
    findings.extend(crate::sql::token::issues(sql).into_iter().map(|issue| {
        crate::model::Finding {
            rule: "sql-break".to_owned(),
            severity: crate::model::Severity::Warning,
            message: issue.message,
            span: issue.span,
            fix: None,
        }
    }));
    findings
}

/// 把一段 SQL 修成房规版式（Generic 方言）；合规输入原样返回。
pub fn format(sql: &str) -> crate::error::Result<String> {
    format_with(sql, Dialect::Generic)
}

/// 按指定方言格式化；解析失败拒绝修复。
pub fn format_with(sql: &str, dialect: Dialect) -> crate::error::Result<String> {
    if crate::sql::lexer::has_comment(sql) {
        return Err(crate::error::Error::Rejected(
            "comments are not supported in fix mode".to_owned(),
        ));
    }
    crate::sql::parse::validate(sql, dialect)
        .map_err(|error| crate::error::Error::Rejected(format!("sql does not parse: {error}")))?;
    let normalized = crate::sql::token::normalize(sql);
    let formatted = crate::sql::fix::format(&normalized)?;
    Ok(crate::sql::token::normalize(&formatted))
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
        Self::new(Dialect::Generic)
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
        if crate::sql::lexer::has_comment(&block.text) {
            return Err(crate::error::Error::Rejected(
                "comments are not supported in fix mode".to_owned(),
            ));
        }
        crate::sql::parse::validate(&block.text, self.dialect).map_err(|error| {
            crate::error::Error::Rejected(format!("sql does not parse: {error}"))
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
