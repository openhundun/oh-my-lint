//! `group by` 里的 `rollup(...)` 参数列表。

use crate::sql::postgresql::dql::checker::Violation;
use crate::sql::postgresql::dql::clause::Mode;
use crate::sql::postgresql::dql::util::line_indent;

/// `rollup(...)` 参数列表：≥4 或含复杂表达式时逐元素一行。
pub(super) fn check(statement_start: usize, statement_text: &str, body_base: usize, body: &str, found: &mut Vec<Violation>) {
    let tokens = crate::sql::lexer::tokens(body);
    for (index, token) in tokens.iter().enumerate() {
        if token.kind != crate::sql::lexer::Kind::Word || !token.text.eq_ignore_ascii_case("rollup") {
            continue;
        }
        let mut probe = index + 1;
        while probe < tokens.len() && tokens[probe].kind == crate::sql::lexer::Kind::Whitespace {
            probe += 1;
        }
        if probe >= tokens.len() || tokens[probe].text != "(" {
            continue;
        }
        let open = tokens[probe].start;
        let mut depth = 0;
        let mut close = None;
        for next in &tokens[probe..] {
            if next.kind == crate::sql::lexer::Kind::Punct && next.text == "(" {
                depth += 1;
            } else if next.kind == crate::sql::lexer::Kind::Punct && next.text == ")" {
                depth -= 1;
                if depth == 0 {
                    close = Some(next.start);
                    break;
                }
            }
        }
        let Some(close) = close else {
            continue;
        };
        let inner = &body[open + 1..close];
        let args = crate::sql::splitter::split_elements("group by", inner);
        if args.is_empty() {
            continue;
        }
        let expected = if args.len() >= 4 || args.iter().any(|argument| crate::sql::decision::is_complex(&argument.text, "group by")) {
            Mode::Break
        } else {
            Mode::Inline
        };
        let actual = crate::sql::splitter::has_top_level_newline(inner);
        if (expected == Mode::Break) == actual {
            continue;
        }
        found.push(Violation {
            kind: "rollup",
            element_count: args.len(),
            expected,
            span: crate::span::Span::new(statement_start + body_base + open + 1, statement_start + body_base + close),
            indent: line_indent(statement_text, body_base + token.start),
            next_indent: None,
            message: format!("rollup n={} expected={}", args.len(), expected.as_str()),
            condition: None,
        });
    }
}

/// `rollup(...)` 的参数列表：展开时一元素一行，`)` 回到 `rollup` 缩进。
pub(super) fn render(text: &str, violation: &Violation) -> Option<String> {
    let inner = &text[violation.span.start..violation.span.end];
    let args = crate::sql::splitter::split_elements("group by", inner);
    if args.is_empty() {
        return None;
    }
    let texts: Vec<String> = args.into_iter().map(|argument| argument.text).collect();
    Some(match violation.expected {
        Mode::Break => {
            let padding = " ".repeat(violation.indent + 4);
            let pad = " ".repeat(violation.indent);
            let joined = texts.join(&format!(",\n{padding}"));
            format!("\n{padding}{joined}\n{pad}")
        }
        Mode::Inline => texts.join(", "),
    })
}
