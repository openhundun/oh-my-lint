//! `with` 框架：CTE 名称、闭括号与主查询的缩进。

use crate::sql::decision;
use crate::sql::postgresql::dql::checker::Violation;
use crate::sql::postgresql::dql::clause::Mode;
use crate::sql::postgresql::dql::util::block_indent;

/// `with` 框架：单个简单 CTE 与 `with` 同行；多个或非简单 CTE 分行。
pub(super) fn check(statement_start: usize, statement_text: &str, block: crate::span::Span, found: &mut Vec<Violation>) {
    let text = &statement_text[block.start..block.end];
    let indent = block_indent(statement_text, block.start);
    let tokens = crate::sql::lexer::tokens(text);
    let Some(first) = tokens.first() else {
        return;
    };
    let single = single_simple_cte(text);
    if single.is_none() {
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
                condition: None,
            });
        }
    } else if let Some((_, close, _, _)) = &single
        && text[..=*close].contains('\n')
    {
        found.push(Violation {
            kind: "with-inline",
            element_count: 1,
            expected: Mode::Inline,
            span: crate::span::Span::new(statement_start + block.start, statement_start + block.start + close + 1),
            indent,
            next_indent: None,
            message: "single simple cte should stay on the with line".to_owned(),
            condition: None,
        });
    }
    if single.is_none() {
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
            let close = inner.end;
            if close >= text.len() {
                continue;
            }
            if text.as_bytes().get(close) == Some(&b')') {
                let mut comma = close + 1;
                while text.as_bytes().get(comma).is_some_and(|byte| *byte == b' ' || *byte == b'\t') {
                    comma += 1;
                }
                if text.as_bytes().get(comma) == Some(&b',') {
                    let mut next = comma + 1;
                    let mut same_line = true;
                    while let Some(byte) = text.as_bytes().get(next) {
                        if *byte == b'\n' {
                            same_line = false;
                            break;
                        }
                        if *byte != b' ' && *byte != b'\t' {
                            break;
                        }
                        next += 1;
                    }
                    if same_line {
                        found.push(Violation {
                            kind: "with-separator",
                            element_count: 0,
                            expected: Mode::Break,
                            span: crate::span::Span::new(statement_start + block.start + comma + 1, statement_start + block.start + next),
                            indent,
                            next_indent: None,
                            message: "cte should start a new line after comma".to_owned(),
                            condition: None,
                        });
                    }
                }
            }
            if !crate::sql::splitter::has_top_level_newline(&text[inner.start..inner.end]) {
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
                condition: None,
            });
        }
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
            condition: None,
        });
    }
}

/// 单个、正文简单的 CTE：返回 `(开括号, 闭括号, 头部, 正文)`。
fn single_simple_cte(text: &str) -> Option<(usize, usize, String, String)> {
    let bodies: Vec<crate::span::Span> = crate::sql::splitter::all_statements(text).into_iter().skip(1).collect();
    if bodies.len() != 1 {
        return None;
    }
    let span = bodies[0];
    let body = &text[span.start..span.end];
    if !decision::is_simple(body) {
        return None;
    }
    let open = text[..span.start].rfind('(')?;
    let close = span.end + text[span.end..].find(')')?;
    let with = crate::sql::lexer::tokens(text)
        .into_iter()
        .find(|token| token.kind == crate::sql::lexer::Kind::Word && token.text.eq_ignore_ascii_case("with"))?;
    let header = collapse(&text[with.start + with.text.len()..open]);
    (!header.is_empty()).then(|| (open, close, header, collapse(body)))
}

/// 折叠空白；字符串与引号标识符逐字节保留。
fn collapse(text: &str) -> String {
    let mut output = String::new();
    let mut pending = false;
    for token in crate::sql::lexer::tokens(text) {
        match token.kind {
            crate::sql::lexer::Kind::Whitespace | crate::sql::lexer::Kind::Comment => {
                if !output.is_empty() {
                    pending = true;
                }
            }
            _ => {
                if pending {
                    output.push(' ');
                    pending = false;
                }
                output.push_str(token.text);
            }
        }
    }
    output
}

/// `with` 独行；闭括号对齐 CTE 名称；主查询另起一行。
pub(super) fn render(text: &str, violation: &Violation) -> Option<String> {
    if violation.kind == "with-inline" {
        let (_, _, header, body) = single_simple_cte(text)?;
        return Some(format!("with {header} ({body})"));
    }
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
    if violation.kind == "with-separator" {
        return Some(format!("\n{}", " ".repeat(violation.indent)));
    }
    if violation.kind == "with-query" {
        let keyword = text[violation.span.start..violation.span.end].trim();
        return Some(format!("\n{}{keyword}", " ".repeat(violation.indent)));
    }
    None
}
