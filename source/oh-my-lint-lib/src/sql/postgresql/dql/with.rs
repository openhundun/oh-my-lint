//! `with` 框架：CTE 名称、闭括号与主查询的缩进。

use crate::sql::postgresql::dql::checker::Violation;
use crate::sql::postgresql::dql::clause::Mode;
use crate::sql::postgresql::dql::util::block_indent;

/// `with` 框架：`with` 独行、CTE 展开时闭括号与名字同列、主查询另起一行。
pub(super) fn check(statement_start: usize, statement_text: &str, block: crate::span::Span, found: &mut Vec<Violation>) {
    let text = &statement_text[block.start..block.end];
    let indent = block_indent(statement_text, block.start);
    let tokens = crate::sql::lexer::tokens(text);
    let Some(first) = tokens.first() else {
        return;
    };
    let multiline = matches!(
        tokens.get(1),
        Some(token)
            if token.kind == crate::sql::lexer::Kind::Whitespace && token.text.contains('\n')
    );
    if !multiline {
        let mut end = first.text.len();
        let bytes = text.as_bytes();
        while end < bytes.len() && matches!(bytes[end], b' ' | b'\t') {
            end += 1;
        }
        found.push(Violation {
            kind: "with",
            element_count: 0,
            expected: Mode::Break,
            span: crate::span::Span::new(statement_start + block.start, statement_start + block.start + end),
            indent,
            next_indent: None,
            message: "with should start its own line".to_owned(),
        });
    }
    for inner in crate::sql::splitter::all_statements(text).into_iter().skip(1) {
        let before = &text[..inner.start];
        let trimmed = before.trim_end();
        if !trimmed.ends_with('(') {
            continue;
        }
        let before_paren = trimmed[..trimmed.len() - 1].trim_end();
        if !before_paren.to_ascii_lowercase().ends_with("as") {
            continue;
        }
        if !crate::sql::splitter::has_top_level_newline(&text[inner.start..inner.end]) {
            continue;
        }
        let close = inner.end;
        if close >= text.len() {
            continue;
        }
        let line_start = text[..close].rfind('\n').map_or(0, |index| index + 1);
        let prefix = &text[line_start..close];
        let aligned = prefix.chars().all(|character| character == ' ' || character == '\t') && prefix.len() == indent;
        if aligned {
            continue;
        }
        let same_line = !prefix.chars().all(|character| character == ' ' || character == '\t');
        let span_start = if same_line { close } else { line_start };
        found.push(Violation {
            kind: "with-close",
            element_count: 0,
            expected: Mode::Break,
            span: crate::span::Span::new(statement_start + block.start + span_start, statement_start + block.start + close + 1),
            indent,
            next_indent: None,
            message: "cte close should align with with".to_owned(),
        });
    }
    let clauses = crate::sql::splitter::clause_spans(text);
    if let Some(clause) = clauses.first() {
        let relative = block.start + clause.keyword_start;
        let line_start = statement_text[..relative].rfind('\n').map_or(0, |index| index + 1);
        let prefix = &statement_text[line_start..relative];
        if prefix.chars().all(|character| character == ' ' || character == '\t') && prefix.len() == indent {
            return;
        }
        let mut start = relative;
        let bytes = statement_text.as_bytes();
        while start > 0 && matches!(bytes[start - 1], b' ' | b'\t') {
            start -= 1;
        }
        found.push(Violation {
            kind: "with-query",
            element_count: 0,
            expected: Mode::Break,
            span: crate::span::Span::new(statement_start + start, statement_start + relative + clause.kind.len()),
            indent,
            next_indent: None,
            message: "query should start a new line".to_owned(),
        });
    }
}

/// `with` 独占一行；闭括号对齐 CTE 名称；主查询另起一行。
pub(super) fn render(text: &str, violation: &Violation) -> Option<String> {
    if violation.kind == "with" {
        return Some(format!("with\n{}", " ".repeat(violation.indent)));
    }
    if violation.kind == "with-close" {
        let pad = " ".repeat(violation.indent);
        let original = &text[violation.span.start..violation.span.end];
        return Some(if original.starts_with(' ') || original.starts_with('\t') {
            format!("{pad})")
        } else {
            format!("\n{pad})")
        });
    }
    if violation.kind == "with-query" {
        let keyword = text[violation.span.start..violation.span.end].trim();
        return Some(format!("\n{}{keyword}", " ".repeat(violation.indent)));
    }
    None
}
