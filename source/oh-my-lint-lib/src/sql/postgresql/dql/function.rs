//! 函数实参：含子查询时逐实参展开，并沿嵌套调用链递归。

use crate::sql::postgresql::dql::checker::Violation;
use crate::sql::postgresql::dql::clause::Mode;
use crate::sql::postgresql::dql::util::{collapse_whitespace, line_indent};

/// 函数实参内出现子查询时，实参逐行展开；已展开的保持原样。
pub(super) fn check(statement_start: usize, statement_text: &str, body_base: usize, body: &str, found: &mut Vec<Violation>) {
    let nested: Vec<crate::span::Span> = crate::sql::splitter::all_statements(body).into_iter().skip(1).collect();
    for call in crate::sql::core::spans::function_paren_spans(body) {
        if nested.iter().any(|block| block.start <= call.start && call.end <= block.end) {
            continue;
        }
        let tokens = crate::sql::lexer::tokens(body);
        let Some(name) = tokens.iter().find(|token| token.start == call.start) else {
            continue;
        };
        if name.kind == crate::sql::lexer::Kind::Word
            && matches!(
                name.text.to_ascii_lowercase().as_str(),
                "over" | "filter" | "within" | "partition" | "group" | "order" | "by" | "using" | "on" | "rollup"
            )
        {
            continue;
        }
        let Some(open) = tokens
            .iter()
            .find(|token| token.start > call.start && token.kind == crate::sql::lexer::Kind::Punct && token.text == "(")
        else {
            continue;
        };
        let inner_start = open.start + 1;
        let inner_end = call.end.saturating_sub(1);
        if inner_start >= inner_end {
            continue;
        }
        let inner = &body[inner_start..inner_end];
        if inner.trim_start_matches([' ', '\t', '\r']).starts_with('\n') || !crate::sql::decision::has_subquery(inner) {
            continue;
        }
        found.push(Violation {
            kind: "function-args",
            element_count: 0,
            expected: Mode::Break,
            span: crate::span::Span::new(statement_start + body_base + inner_start, statement_start + body_base + inner_end),
            indent: line_indent(statement_text, body_base + call.start),
            next_indent: None,
            message: "function args with a subquery should break".to_owned(),
        });
    }
}

/// 函数实参列表：含子查询时逐实参一行，闭括号回到函数名缩进。
pub(super) fn render(text: &str, violation: &Violation) -> Option<String> {
    let inner = &text[violation.span.start..violation.span.end];
    let args = crate::sql::splitter::split_elements("select", inner);
    if args.is_empty() {
        return None;
    }
    let padding = " ".repeat(violation.indent + 4);
    let pad = " ".repeat(violation.indent);
    let rendered = args.iter().map(|argument| collapse_whitespace(&argument.text)).collect::<Vec<_>>().join(&format!(",\n{padding}"));
    Some(format!("\n{padding}{rendered}\n{pad}"))
}
