//! SQL 断行规则：检查与增量修复。

mod checker;
mod decision;
mod fix;
mod lexer;
mod splitter;
mod token;

/// 检查一段 SQL，返回块内坐标的诊断。
pub fn check(sql: &str) -> Vec<crate::model::Finding> {
    if crate::sql::lexer::has_comment(sql) {
        return Vec::new();
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

/// 把一段 SQL 修成房规版式；合规输入原样返回。
pub fn format(sql: &str) -> crate::error::Result<String> {
    if crate::sql::lexer::has_comment(sql) {
        return Err(crate::error::Error::Rejected(
            "comments are not supported in fix mode".to_owned(),
        ));
    }
    let normalized = crate::sql::token::normalize(sql);
    crate::sql::fix::format(&normalized)
}

/// 规则入口：`sql-break`。
pub struct SqlRule;

impl crate::model::Rule for SqlRule {
    fn id(&self) -> &'static str {
        "sql-break"
    }

    fn description(&self) -> &'static str {
        "SQL 断行：子句与元素按数量/复杂度决定内联或逐元素换行"
    }

    fn check(&self, block: &crate::model::Block) -> Vec<crate::model::Finding> {
        crate::sql::check(&block.text)
    }

    fn fix(&self, block: &crate::model::Block) -> Option<crate::model::Fix> {
        let formatted = crate::sql::format(&block.text).ok()?;
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
        Ok(())
    }
}

#[cfg(test)]
mod tests;
