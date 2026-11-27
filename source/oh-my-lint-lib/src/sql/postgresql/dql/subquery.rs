//! 子查询：括号定位、递归重排与闭合缩进。

use crate::span::Span;
use crate::sql::parse::Dialect;
use crate::sql::postgresql::dql::checker::Violation;

/// 子查询块两侧的括号；块内部用于统一重排。
pub(super) fn query_parens(text: &str, block: Span) -> Option<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut open = block.start;
    while open > 0 && bytes[open - 1].is_ascii_whitespace() {
        open -= 1;
    }
    if open == 0 || bytes[open - 1] != b'(' {
        return None;
    }
    let mut close = block.end;
    while close < bytes.len() && bytes[close].is_ascii_whitespace() {
        close += 1;
    }
    if close >= bytes.len() || bytes[close] != b')' {
        return None;
    }
    Some((open - 1, close))
}

/// 只把 `select` / `with` 开头的嵌套块视为子查询；`(1), (2)` 这类行构造器不报。
pub(super) fn is_query_block(text: &str) -> bool {
    let mut tokens = crate::sql::lexer::tokens(text).into_iter().filter(|token| token.kind != crate::sql::lexer::Kind::Whitespace);
    let mut first = tokens.next();
    if first.is_some_and(|token| token.kind == crate::sql::lexer::Kind::Punct && token.text == "(") {
        first = tokens.next();
    }
    first.is_some_and(|token| token.kind == crate::sql::lexer::Kind::Word && (token.text.eq_ignore_ascii_case("select") || token.text.eq_ignore_ascii_case("with")))
}

/// 子查询渲染：复杂块展开，简单块压回一行。
pub(super) fn render(text: &str, violation: &Violation, dialect: Dialect) -> Option<String> {
    let block = &text[violation.span.start..violation.span.end];
    match violation.expected {
        crate::sql::postgresql::dql::clause::Mode::Break => format_nested(block, violation.indent, dialect),
        crate::sql::postgresql::dql::clause::Mode::Inline => {
            let flat = crate::sql::postgresql::dql::util::collapse_whitespace(block);
            if flat.trim().is_empty() {
                return None;
            }
            let formatted = crate::sql::postgresql::dql::fix::format_with(&flat, dialect).ok()?;
            Some(crate::sql::postgresql::dql::util::collapse_whitespace(&formatted))
        }
    }
}

/// 把子查询体压平后按 DQL 重排，并统一加一级缩进；检查与修复共用。
pub(crate) fn format_nested(block: &str, indent: usize, dialect: Dialect) -> Option<String> {
    let flat = crate::sql::postgresql::dql::util::collapse_whitespace(block);
    if flat.trim().is_empty() {
        return None;
    }
    let body = crate::sql::postgresql::dql::fix::format_with(&flat, dialect).ok()?;
    Some(indent_block(&body, indent))
}

/// 子查询/函数实参展开后的缩进：正文 +4，闭括号回到起始缩进。
fn indent_block(body: &str, indent: usize) -> String {
    let padding = " ".repeat(indent + 4);
    let pad = " ".repeat(indent);
    let trimmed = body.trim_start();
    let mut lines = trimmed.lines();
    let Some(first) = lines.next() else {
        return String::new();
    };
    let mut rendered = format!("\n{padding}{}", first.trim_end());
    for line in lines {
        rendered.push('\n');
        if !line.trim().is_empty() {
            rendered.push_str(&padding);
            rendered.push_str(line.trim_end());
        }
    }
    rendered.push('\n');
    rendered.push_str(&pad);
    rendered
}
