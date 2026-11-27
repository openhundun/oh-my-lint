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
    let formatted = apply(text, edits);
    format_update_action(&formatted, base_indent)
}

/// `on conflict ... do update set` 的 `set` 留在 `do update` 行，赋值逐行 +4。
fn format_update_action(text: &str, base_indent: &str) -> String {
    let tokens = lexer::tokens(text);
    let mut depth = 0usize;
    let mut set = None;
    let mut update_end = None;
    for (index, token) in tokens.iter().enumerate() {
        if token.kind == Kind::Punct {
            match token.text {
                "(" => depth += 1,
                ")" => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
        if depth != 0 || set.is_some() || !word(token, "do") {
            continue;
        }
        let Some(update) = next_word(&tokens, index + 1, "update") else {
            continue;
        };
        let Some(set_index) = next_word(&tokens, update + 1, "set") else {
            continue;
        };
        update_end = Some(token_end(&tokens[update]));
        set = Some(set_index);
    }
    let (Some(set), Some(update_end)) = (set, update_end) else {
        return text.to_owned();
    };
    let set_start = tokens[set].start;
    let set_end = token_end(&tokens[set]);
    let mut depth = 0usize;
    let mut body_end = text.len();
    for token in tokens.iter().skip(set + 1) {
        if token.kind == Kind::Punct {
            match token.text {
                "(" => depth += 1,
                ")" => depth = depth.saturating_sub(1),
                ";" if depth == 0 => {
                    body_end = token.start;
                    break;
                }
                _ => {}
            }
        }
        if depth == 0 && (word(token, "where") || word(token, "returning")) {
            body_end = token.start;
            break;
        }
    }
    let assignments = splitter::split_elements("set", &text[set_end..body_end]);
    if assignments.is_empty() {
        return text.to_owned();
    }
    let mut edits = Vec::new();
    if text[update_end..set_start] != *" " {
        edits.push((update_end, set_start, " ".to_owned()));
    }
    let indent = format!("{base_indent}    ");
    let body = assignments
        .iter()
        .map(|assignment| format!("{indent}{}", super::collapse(&assignment.text)))
        .collect::<Vec<_>>()
        .join(",\n");
    let suffix = text.get(body_end..).map(str::trim_start).unwrap_or_default().to_ascii_lowercase();
    let first = suffix.split_whitespace().next().unwrap_or_default();
    let separator = if matches!(first, "where" | "returning") { format!("\n{base_indent}") } else { String::new() };
    let replacement = format!("\n{body}{separator}");
    if text[set_end..body_end] != replacement {
        edits.push((set_end, body_end, replacement));
    }
    apply(text, edits)
}

fn next_word(tokens: &[Token<'_>], start: usize, expected: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate().skip(start) {
        if token.kind == Kind::Punct {
            match token.text {
                "(" => depth += 1,
                ")" => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
        if depth == 0 && word(token, expected) {
            return Some(index);
        }
    }
    None
}

fn token_end(token: &Token<'_>) -> usize {
    token.start + token.text.len()
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

#[cfg(test)]
mod tests {
    fn format(sql: &str) -> String {
        crate::sql::format(sql).expect("format")
    }

    #[test]
    fn keeps_do_update_set_on_the_conflict_line_and_indents_assignments() {
        let sql = "insert into t (a, b, c) values (1, 2, 3) on conflict (a) do update set b = excluded.b, c = excluded.c;";
        let expected = "insert into t (a, b, c) values (1, 2, 3)\non conflict (a) do update set\n    b = excluded.b,\n    c = excluded.c;";
        assert_eq!(format(sql), expected);
        assert_eq!(format(expected), expected);
        assert!(crate::sql::check(expected).is_empty(), "{:?}", crate::sql::check(expected));
    }

    #[test]
    fn joins_do_update_set_when_split() {
        let sql = "insert into t (a, b) values (1, 2)\non conflict (a) do update\nset\n  b = excluded.b;";
        let expected = "insert into t (a, b) values (1, 2)\non conflict (a) do update set\n    b = excluded.b;";
        assert_eq!(format(sql), expected);
    }

    #[test]
    fn keeps_where_after_do_update_assignments() {
        let sql = "insert into t (a, b) values (1, 2) on conflict (a) do update set b = excluded.b where t.a = 1;";
        let expected = "insert into t (a, b) values (1, 2)\non conflict (a) do update set\n    b = excluded.b\nwhere t.a = 1;";
        assert_eq!(format(sql), expected);
    }
}
