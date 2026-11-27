//! 通用语法区间：`case`、函数调用与嵌套区间归并。

use crate::span::Span;

/// 提取 `case … end` 区间（含嵌套）。
pub(crate) fn case_spans(text: &str) -> Vec<Span> {
    let tokens = crate::sql::lexer::tokens(text);
    let mut spans = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        if token.kind != crate::sql::lexer::Kind::Word || !token.text.eq_ignore_ascii_case("case") {
            continue;
        }
        let mut nesting = 1;
        let mut end = None;
        for next in &tokens[index + 1..] {
            if next.kind == crate::sql::lexer::Kind::Word && next.text.eq_ignore_ascii_case("case") {
                nesting += 1;
            } else if next.kind == crate::sql::lexer::Kind::Word && next.text.eq_ignore_ascii_case("end") {
                nesting -= 1;
                if nesting == 0 {
                    end = Some(next.start + next.text.len());
                    break;
                }
            }
        }
        if let Some(end) = end {
            spans.push(Span::new(token.start, end));
        }
    }
    spans
}

/// 只保留最外层的区间。
pub(crate) fn outermost(spans: Vec<Span>) -> Vec<Span> {
    let mut ordered = spans;
    ordered.sort_by_key(|span| (span.start, std::cmp::Reverse(span.end)));
    let mut result: Vec<Span> = Vec::new();
    for span in ordered {
        if result.last().is_some_and(|last| last.start <= span.start && span.end <= last.end) {
            continue;
        }
        result.push(span);
    }
    result
}

/// 函数调用括号的区间（从函数名到闭括号）。
pub(crate) fn function_paren_spans(text: &str) -> Vec<Span> {
    let tokens = crate::sql::lexer::tokens(text);
    let mut spans = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        if token.kind != crate::sql::lexer::Kind::Punct || token.text != "(" {
            continue;
        }
        let mut probe = index;
        while probe > 0 && tokens[probe - 1].kind == crate::sql::lexer::Kind::Whitespace {
            probe -= 1;
        }
        if probe == 0 {
            continue;
        }
        let name = tokens[probe - 1];
        let is_name = match name.kind {
            crate::sql::lexer::Kind::Word => crate::sql::core::keywords::is_function_name(name.text),
            crate::sql::lexer::Kind::QuotedIdent => true,
            _ => false,
        };
        if !is_name {
            continue;
        }
        let mut depth = 0;
        for next in &tokens[index..] {
            if next.kind == crate::sql::lexer::Kind::Punct && next.text == "(" {
                depth += 1;
            } else if next.kind == crate::sql::lexer::Kind::Punct && next.text == ")" {
                depth -= 1;
                if depth == 0 {
                    spans.push(Span::new(name.start, next.start + next.text.len()));
                    break;
                }
            }
        }
    }
    spans
}
