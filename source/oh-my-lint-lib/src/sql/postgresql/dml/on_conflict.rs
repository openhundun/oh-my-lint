//! `insert ... on conflict`：子句另起一行并与语句同缩进。

use crate::span::Span;
use crate::sql::lexer::{self, Kind, Token};
use crate::sql::parse::Dialect;
use crate::sql::splitter;

/// 把 `on conflict` 移到新行；不可解析语句不动。
pub(crate) fn format(text: &str, dialect: Dialect) -> String {
    let protected = protected_ranges(text, dialect);
    let spans = splitter::statements(text);
    let mut output = text.to_owned();
    for span in spans.iter().rev() {
        if is_protected(&protected, span.start) {
            continue;
        }
        let statement = &text[span.start..span.end];
        let indent = line_indent_of(text, span.start);
        let formatted = format_statement(statement, &indent);
        if formatted != statement {
            output.replace_range(span.start..span.end, &formatted);
        }
    }
    output
}

fn format_statement(text: &str, base_indent: &str) -> String {
    let tokens = lexer::tokens(text);
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate() {
        if token.kind == Kind::Punct {
            match token.text {
                "(" => depth += 1,
                ")" => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
        if depth != 0 || !word(token, "on") {
            continue;
        }
        let Some(next) = next_non_ws(&tokens, index + 1) else {
            continue;
        };
        if !word(&tokens[next], "conflict") || !has_insert(&tokens, 0, index) {
            continue;
        }
        let mut start = token.start;
        while start > 0 && text.as_bytes()[start - 1].is_ascii_whitespace() {
            start -= 1;
        }
        let replacement = format!("\n{base_indent}");
        if text[start..token.start] != replacement {
            edits.push((start, token.start, replacement));
        }
    }
    apply(text, edits)
}

fn protected_ranges(text: &str, dialect: Dialect) -> Vec<Span> {
    splitter::statements(text)
        .into_iter()
        .filter(|span| crate::sql::parse::classify_one(&text[span.start..span.end], dialect).is_err())
        .collect()
}

fn is_protected(protected: &[Span], position: usize) -> bool {
    protected.iter().any(|span| span.start <= position && position < span.end)
}

fn has_insert(tokens: &[Token<'_>], start: usize, end: usize) -> bool {
    let mut depth = 0usize;
    for token in tokens.iter().take(end).skip_while(|token| token.start < start) {
        if token.kind == Kind::Punct {
            match token.text {
                "(" => depth += 1,
                ")" => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
        if depth == 0 && word(token, "insert") {
            return true;
        }
    }
    false
}

fn line_indent_of(text: &str, position: usize) -> String {
    let line_start = text[..position].rfind('\n').map_or(0, |offset| offset + 1);
    let prefix = &text[line_start..position];
    prefix[..prefix.len() - prefix.trim_start().len()].to_owned()
}

fn next_non_ws(tokens: &[Token<'_>], index: usize) -> Option<usize> {
    (index..tokens.len()).find(|index| !matches!(tokens[*index].kind, Kind::Whitespace | Kind::Comment))
}

fn word(token: &Token<'_>, expected: &str) -> bool {
    token.kind == Kind::Word && token.text.eq_ignore_ascii_case(expected)
}

fn apply(text: &str, mut edits: Vec<(usize, usize, String)>) -> String {
    if edits.is_empty() {
        return text.to_owned();
    }
    edits.sort_by_key(|(start, end, _)| (*start, *end));
    let mut output = text.to_owned();
    for (start, end, replacement) in edits.into_iter().rev() {
        output.replace_range(start..end, &replacement);
    }
    output
}
