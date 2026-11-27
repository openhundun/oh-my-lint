//! 独立 `case` 表达式：分支复杂时展开，函数实参与窗口内的 `case` 不展开。

use crate::sql::core::spans::{case_spans, function_paren_spans, outermost, over_paren_spans};
use crate::sql::postgresql::dql::checker::Violation;
use crate::sql::postgresql::dql::clause::Mode;
use crate::sql::postgresql::dql::util::line_indent;

/// 独立 `case`：分支复杂则展开；函数实参或窗口内的 `case` 不展开。
pub(super) fn check(statement_start: usize, statement_text: &str, body_base: usize, body: &str, found: &mut Vec<Violation>) {
    let nested: Vec<crate::span::Span> = crate::sql::splitter::all_statements(body).into_iter().skip(1).collect();
    let functions = function_paren_spans(body);
    let windows = over_paren_spans(body);
    let cases = outermost(case_spans(body));
    for case in cases {
        if nested.iter().any(|block| block.start <= case.start && case.end <= block.end) {
            continue;
        }
        let text = &body[case.start..case.end];
        if windows.iter().any(|span| span.start <= case.start && case.end <= span.end) {
            if crate::sql::splitter::has_top_level_newline(text) {
                found.push(Violation {
                    kind: "case",
                    element_count: 0,
                    expected: Mode::Inline,
                    span: crate::span::Span::new(statement_start + body_base + case.start, statement_start + body_base + case.end),
                    indent: line_indent(statement_text, body_base + case.start),
                    next_indent: None,
                    message: "window case should stay inline".to_owned(),
                });
            }
            continue;
        }
        if functions.iter().any(|span| span.start <= case.start && case.end <= span.end) {
            continue;
        }
        let expected = if crate::sql::decision::branches_complex(text) { Mode::Break } else { Mode::Inline };
        let actual = crate::sql::splitter::has_top_level_newline(text);
        if (expected == Mode::Break) == actual {
            continue;
        }
        found.push(Violation {
            kind: "case",
            element_count: 0,
            expected,
            span: crate::span::Span::new(statement_start + body_base + case.start, statement_start + body_base + case.end),
            indent: line_indent(statement_text, body_base + case.start),
            next_indent: None,
            message: format!("case expected={}", expected.as_str()),
        });
    }
}

/// 独立 `case`：展开时每个分支关键字独占一行。
pub(super) fn render(text: &str, violation: &Violation) -> Option<String> {
    let case_text = &text[violation.span.start..violation.span.end];
    let parts = case_parts(case_text)?;
    match violation.expected {
        Mode::Break => {
            let padding = " ".repeat(violation.indent + 4);
            let pad = " ".repeat(violation.indent);
            let mut lines = vec!["case".to_owned()];
            for (keyword, content) in parts {
                lines.push(format!("{padding}{keyword} {content}"));
            }
            lines.push(format!("{pad}end"));
            Some(lines.join("\n"))
        }
        Mode::Inline => {
            let mut joined = String::from("case");
            for (keyword, content) in parts {
                joined.push(' ');
                joined.push_str(keyword);
                joined.push(' ');
                joined.push_str(&content);
            }
            joined.push_str(" end");
            Some(joined)
        }
    }
}

/// 把 `case … end` 拆成 `(when|then|else, 正文)` 序列；嵌套 `case` 不拆。
fn case_parts(text: &str) -> Option<Vec<(&'static str, String)>> {
    let tokens = crate::sql::lexer::tokens(text);
    let mut parts: Vec<(&'static str, String)> = Vec::new();
    let mut current: Option<(&'static str, usize)> = None;
    let mut depth = 0;
    for token in &tokens {
        if token.kind != crate::sql::lexer::Kind::Word {
            continue;
        }
        let lower = token.text.to_ascii_lowercase();
        if lower == "case" {
            depth += 1;
            continue;
        }
        if lower == "end" {
            depth -= 1;
            if depth == 0 {
                if let Some((keyword, start)) = current.take() {
                    parts.push((keyword, text[start..token.start].trim().to_owned()));
                }
                return Some(parts);
            }
            continue;
        }
        if depth == 1 && matches!(lower.as_str(), "when" | "then" | "else") {
            if let Some((keyword, start)) = current.take() {
                parts.push((keyword, text[start..token.start].trim().to_owned()));
            }
            let keyword = match lower.as_str() {
                "when" => "when",
                "then" => "then",
                _ => "else",
            };
            current = Some((keyword, token.start + token.text.len()));
        }
    }
    None
}
