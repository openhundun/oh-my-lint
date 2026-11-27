//! `VALUES` 行构造器与派生表别名列表的版式。

use crate::span::Span;
use crate::sql::lexer::{self, Kind, Token};
use crate::sql::parse::{Dialect, classify_one};
use crate::sql::splitter;

pub(crate) fn format(text: &str, dialect: Dialect) -> String {
    let protected = protected_ranges(text, dialect);
    let text = space_alias_columns(text, &protected);
    let protected = protected_ranges(&text, dialect);
    let text = format_insert_headers(&text, &protected);
    let protected = protected_ranges(&text, dialect);
    let text = format_values(&text, &protected);
    format_on_conflict(&text, dialect)
}

/// `insert into t` 与字段列表、`values` 始终同一行；不可解析语句不动。
fn format_insert_headers(text: &str, protected: &[Span]) -> String {
    let tokens = lexer::tokens(text);
    let spans = splitter::statements(text);
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    for span in spans {
        if is_protected(protected, span.start) {
            continue;
        }
        let mut depth = 0usize;
        let mut insert = None;
        let mut values = None;
        for (index, token) in tokens.iter().enumerate() {
            if token.start < span.start {
                continue;
            }
            if token.start >= span.end {
                break;
            }
            if token.kind == Kind::Punct {
                match token.text {
                    "(" => depth += 1,
                    ")" => depth = depth.saturating_sub(1),
                    _ => {}
                }
            }
            if depth != 0 {
                continue;
            }
            if insert.is_none() && word(token, "insert") {
                insert = Some(index);
            } else if insert.is_some() && word(token, "values") {
                values = Some(index);
                break;
            }
        }
        let (Some(insert), Some(values)) = (insert, values) else {
            continue;
        };
        let start = tokens[insert].start;
        let end = tokens[values].start;
        let header = collapse_whitespace(&text[start..end]);
        let replacement = format!("{} ", header.trim_end());
        if text[start..end] != replacement {
            edits.push((start, end, replacement));
        }
    }
    apply(text, edits)
}

/// `on conflict` 子句另起一行，并与语句同缩进。
fn format_on_conflict(text: &str, dialect: Dialect) -> String {
    let protected = protected_ranges(text, dialect);
    let tokens = lexer::tokens(text);
    let spans = splitter::statements(text);
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
        if depth != 0 || !word(token, "on") || is_protected(&protected, token.start) {
            continue;
        }
        let Some(next) = next_non_ws(&tokens, index + 1) else {
            continue;
        };
        if !word(&tokens[next], "conflict") {
            continue;
        }
        let Some(span) = spans.iter().find(|span| span.start <= token.start && token.start < span.end) else {
            continue;
        };
        if !has_insert(&tokens, span.start, index) {
            continue;
        }
        let indent = line_indent_of(text, span.start);
        let mut start = token.start;
        while start > 0 && text.as_bytes()[start - 1].is_ascii_whitespace() {
            start -= 1;
        }
        let replacement = format!("\n{indent}");
        if text[start..token.start] != replacement {
            edits.push((start, token.start, replacement));
        }
    }
    apply(text, edits)
}

/// 无法解析的语句按 R34 保留，`values` 与别名列表也不碰。
fn protected_ranges(text: &str, dialect: Dialect) -> Vec<Span> {
    splitter::statements(text)
        .into_iter()
        .filter(|span| classify_one(&text[span.start..span.end], dialect).is_err())
        .collect()
}

fn is_protected(protected: &[Span], position: usize) -> bool {
    protected.iter().any(|span| span.start <= position && position < span.end)
}

/// `) t(col, ...)` → `) t (col, ...)`；只处理派生表别名，不碰函数调用。
fn space_alias_columns(text: &str, protected: &[Span]) -> String {
    let tokens = lexer::tokens(text);
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        if token.kind != Kind::Punct || token.text != "(" || is_protected(protected, token.start) {
            continue;
        }
        let Some(previous) = previous_non_ws(&tokens, index) else {
            continue;
        };
        if !is_ident(&tokens[previous]) || token_end(&tokens[previous]) != token.start {
            continue;
        }
        if tokens[previous].kind == Kind::Word
            && matches!(
                tokens[previous].text.to_ascii_lowercase().as_str(),
                "over" | "filter" | "within" | "partition" | "group" | "order" | "by" | "using" | "on"
            )
        {
            continue;
        }
        let Some(before) = previous_non_ws(&tokens, previous) else {
            continue;
        };
        let alias_after_paren = punct(&tokens[before], ")");
        let alias_after_as = word(&tokens[before], "as") && previous_non_ws(&tokens, before).is_some_and(|index| punct(&tokens[index], ")"));
        if alias_after_paren || alias_after_as {
            edits.push((token.start, token.start, " ".to_owned()));
        }
    }
    apply(text, edits)
}

fn format_values(text: &str, protected: &[Span]) -> String {
    let tokens = lexer::tokens(text);
    let spans = splitter::statements(text);
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
        if is_protected(protected, token.start) {
            continue;
        }
        if token.kind != Kind::Word || !token.text.eq_ignore_ascii_case("values") {
            continue;
        }
        let Some(previous) = previous_non_ws(&tokens, index) else {
            continue;
        };
        if word(&tokens[previous], "for") {
            continue;
        }
        let statement_start = spans.iter().find(|span| span.start <= token.start && token.start < span.end).map_or(0, |span| span.start);
        let insert = depth == 0 && has_insert(&tokens, statement_start, index);
        let derived = depth > 0 && punct(&tokens[previous], "(");
        if !insert && !derived {
            continue;
        }
        let Some(tuples) = tuple_spans(&tokens, index + 1) else {
            continue;
        };
        if tuples.len() < 2 && !insert {
            continue;
        }

        let line_start = text[..token.start].rfind('\n').map_or(0, |offset| offset + 1);
        let line_prefix = &text[line_start..token.start];
        let inline = !line_prefix.trim().is_empty();
        let line_indent = leading_whitespace(line_prefix);
        let indent = if insert {
            if inline { line_indent.to_owned() } else { line_indent_of(text, tokens[previous].start) }
        } else if inline {
            format!("{line_indent}    ")
        } else {
            line_prefix.to_owned()
        };

        if insert && !inline {
            edits.push((token_end(&tokens[previous]), token.start, " ".to_owned()));
        }
        if derived && inline {
            edits.push((token_end(&tokens[previous]), token.start, format!("\n{indent}")));
        }
        if tuples.len() < 2 {
            if insert {
                for (start, end) in &tuples {
                    let collapsed = collapse_whitespace(&text[*start..*end]);
                    if text[*start..*end] != collapsed {
                        edits.push((*start, *end, collapsed));
                    }
                }
            }
            continue;
        }

        let Some(&(_, last_end)) = tuples.last() else {
            continue;
        };
        let mut rendered = String::new();
        for (position, (start, end)) in tuples.iter().enumerate() {
            if position > 0 {
                rendered.push(',');
            }
            rendered.push('\n');
            rendered.push_str(&indent);
            rendered.push_str(&collapse_whitespace(&text[*start..*end]));
        }
        edits.push((token_end(token), last_end, rendered));

        let suffix = next_non_ws(&tokens, tuple_end_index(&tokens, last_end) + 1);
        if let Some(suffix) = suffix {
            if punct(&tokens[suffix], ")") {
                if let Some(open) = previous_open(&tokens, index) {
                    let close_indent = line_indent_of(text, tokens[open].start);
                    edits.push((last_end, tokens[suffix].start, format!("\n{close_indent}")));
                }
            } else if !punct(&tokens[suffix], ";") {
                edits.push((last_end, tokens[suffix].start, " ".to_owned()));
            }
        }
    }
    apply(text, edits)
}

fn tuple_spans(tokens: &[Token<'_>], start: usize) -> Option<Vec<(usize, usize)>> {
    let mut tuples = Vec::new();
    let mut index = next_non_ws(tokens, start)?;
    loop {
        if !punct(&tokens[index], "(") {
            break;
        }
        let close = matching_paren(tokens, index)?;
        tuples.push((tokens[index].start, token_end(&tokens[close])));
        let next = next_non_ws(tokens, close + 1);
        if let Some(next) = next
            && punct(&tokens[next], ",")
        {
            let Some(after) = next_non_ws(tokens, next + 1) else {
                break;
            };
            if punct(&tokens[after], "(") {
                index = after;
                continue;
            }
        }
        break;
    }
    (!tuples.is_empty()).then_some(tuples)
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

fn previous_open(tokens: &[Token<'_>], index: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut probe = index;
    while probe > 0 {
        probe -= 1;
        let token = &tokens[probe];
        if token.kind != Kind::Punct {
            continue;
        }
        if token.text == ")" {
            depth += 1;
        } else if token.text == "(" {
            if depth == 0 {
                return Some(probe);
            }
            depth -= 1;
        }
    }
    None
}

fn matching_paren(tokens: &[Token<'_>], open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate().skip(open) {
        if token.kind != Kind::Punct {
            continue;
        }
        match token.text {
            "(" => depth += 1,
            ")" => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

fn tuple_end_index(tokens: &[Token<'_>], end: usize) -> usize {
    tokens.iter().position(|token| token_end(token) == end).unwrap_or(tokens.len().saturating_sub(1))
}

fn collapse_whitespace(text: &str) -> String {
    let mut output = String::new();
    let mut pending = false;
    for token in lexer::tokens(text) {
        if token.kind == Kind::Whitespace {
            pending = true;
            continue;
        }
        if pending && !output.is_empty() {
            output.push(' ');
        }
        output.push_str(token.text);
        pending = false;
    }
    output
}

fn line_indent_of(text: &str, position: usize) -> String {
    let line_start = text[..position].rfind('\n').map_or(0, |offset| offset + 1);
    leading_whitespace(&text[line_start..position]).to_owned()
}

fn leading_whitespace(text: &str) -> &str {
    &text[..text.len() - text.trim_start().len()]
}

fn previous_non_ws(tokens: &[Token<'_>], index: usize) -> Option<usize> {
    (0..index).rev().find(|index| !matches!(tokens[*index].kind, Kind::Whitespace | Kind::Comment))
}

fn next_non_ws(tokens: &[Token<'_>], index: usize) -> Option<usize> {
    (index..tokens.len()).find(|index| !matches!(tokens[*index].kind, Kind::Whitespace | Kind::Comment))
}

fn token_end(token: &Token<'_>) -> usize {
    token.start + token.text.len()
}

fn word(token: &Token<'_>, expected: &str) -> bool {
    token.kind == Kind::Word && token.text.eq_ignore_ascii_case(expected)
}

fn is_ident(token: &Token<'_>) -> bool {
    matches!(token.kind, Kind::Word | Kind::QuotedIdent)
}

fn punct(token: &Token<'_>, expected: &str) -> bool {
    token.kind == Kind::Punct && token.text == expected
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
    fn breaks_multi_row_insert_values_on_the_header_line() {
        let sql = "insert into t (a, b) values (1, 2), (3, 4);";
        let expected = "insert into t (a, b) values\n(1, 2),\n(3, 4);";
        assert_eq!(format(sql), expected);
    }

    #[test]
    fn breaks_multi_row_derived_values_without_indenting_rows() {
        let sql = "select * from (values (1, 2), (3, 4)) t(a, b);";
        let expected = "select * from (\n    values\n    (1, 2),\n    (3, 4)\n) t (a, b);";
        assert_eq!(format(sql), expected);
    }

    #[test]
    fn keeps_single_row_values_inline() {
        let sql = "insert into t (a) values (1);";
        assert_eq!(format(sql), sql);
    }

    #[test]
    fn joins_single_row_values_to_insert_header() {
        let sql = "insert into t (a)\nvalues (1);";
        assert_eq!(format(sql), "insert into t (a) values (1);");
    }

    #[test]
    fn keeps_insert_column_list_on_the_header() {
        let sql = "insert into t\n    (a, b) values (1, 2);";
        assert_eq!(format(sql), "insert into t (a, b) values (1, 2);");
    }

    #[test]
    fn collapses_a_multiline_values_tuple() {
        let sql = "insert into t (a, b) values (1,\n2);";
        assert_eq!(format(sql), "insert into t (a, b) values (1, 2);");
    }

    #[test]
    fn starts_on_conflict_on_its_own_line() {
        let sql = "insert into t (a, b) values (1, 2) on conflict (a) do nothing;";
        assert_eq!(format(sql), "insert into t (a, b) values (1, 2)\non conflict (a) do nothing;");
    }

    #[test]
    fn starts_on_conflict_after_select_on_its_own_line() {
        let sql = "insert into t (a) select x from s where y = 1 on conflict (a) do nothing;";
        assert_eq!(format(sql), "insert into t (a)\nselect x from s where y = 1\non conflict (a) do nothing;");
    }
}
