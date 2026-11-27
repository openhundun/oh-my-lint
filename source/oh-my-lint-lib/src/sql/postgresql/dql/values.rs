//! `VALUES` 行构造器与派生表别名列表的版式。

use crate::span::Span;
use crate::sql::lexer::{self, Kind, Token};
use crate::sql::parse::{Dialect, classify_one};
use crate::sql::splitter;

/// `values` 出现的上下文；决定是否折叠元组以及派生表缩进。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ValuesInput {
    CtxInsert,
    CtxSubquery,
}

/// 一个 `values` 块的输出；`span` 是应被替换的源区间。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ValuesOutput {
    Inline { text: String, span: Span },
    Exline { text: String, span: Span },
}

pub(crate) fn format(text: &str, dialect: Dialect) -> String {
    let protected = protected_ranges(text, dialect);
    let text = space_alias_columns(text, &protected);
    let protected = protected_ranges(&text, dialect);
    let text = format_insert_headers(&text, &protected);
    let protected = protected_ranges(&text, dialect);
    format_values(&text, &protected)
}

/// `insert into t` 与字段列表始终同一行；`values` 是否另起一行由 `format_values` 决定。
fn format_insert_headers(text: &str, protected: &[Span]) -> String {
    map_statements(text, protected, format_insert_headers_one)
}

fn format_insert_headers_one(text: &str) -> String {
    let tokens = lexer::tokens(text);
    let mut depth = 0usize;
    let mut insert = None;
    let mut values = None;
    for (index, token) in tokens.iter().enumerate() {
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
        return text.to_owned();
    };
    let start = tokens[insert].start;
    let end = tokens[values].start;
    let header = collapse_whitespace(&text[start..end]);
    let replacement = format!("{} ", header.trim_end());
    if text[start..end] == replacement {
        text.to_owned()
    } else {
        format!("{}{}{}", &text[..start], replacement, &text[end..])
    }
}

/// 用一条语句自己的 tokenizer 结果做替换，避免整文件 tokenizer 失败导致全文件跳过。
fn map_statements(text: &str, protected: &[Span], render: impl Fn(&str) -> String) -> String {
    let spans = splitter::statements(text);
    let mut output = text.to_owned();
    for span in spans.iter().rev() {
        if is_protected(protected, span.start) {
            continue;
        }
        let statement = &text[span.start..span.end];
        let formatted = render(statement);
        if formatted != statement {
            output.replace_range(span.start..span.end, &formatted);
        }
    }
    output
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
    map_statements(text, protected, space_alias_columns_one)
}

fn space_alias_columns_one(text: &str) -> String {
    let tokens = lexer::tokens(text);
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        if token.kind != Kind::Punct || token.text != "(" {
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
    map_statements(text, protected, format_statement_values)
}

/// 单条语句内的 `values` 排版；不跨语句累计括号深度。
fn format_statement_values(text: &str) -> String {
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
        if token.kind != Kind::Word || !token.text.eq_ignore_ascii_case("values") {
            continue;
        }
        let Some(previous) = previous_non_ws(&tokens, index) else {
            continue;
        };
        if word(&tokens[previous], "for") {
            continue;
        }
        let insert = depth == 0 && has_insert(&tokens, 0, index);
        let derived = depth > 0 && punct(&tokens[previous], "(");
        if derived
            && previous_open(&tokens, index)
                .and_then(|open| previous_non_ws(&tokens, open))
                .is_some_and(|before| word(&tokens[before], "as"))
        {
            // CTAS 的 `as (values ...)` 由 DDL CTAS 渲染器负责，不按派生表展开。
            continue;
        }
        if !insert && !derived {
            continue;
        }
        let Some(tuples) = tuple_spans(&tokens, index + 1) else {
            continue;
        };
        if derived && inside_insert_values_tuple(&tokens, index) {
            let Some(close) = matching_paren(&tokens, previous) else {
                continue;
            };
            let Some((_, last_end)) = tuples.last().copied() else {
                continue;
            };
            if next_non_ws(&tokens, tuple_end_index(&tokens, last_end) + 1) == Some(close) {
                let rows = tuples.iter().map(|(start, end)| collapse_whitespace(&text[*start..*end])).collect::<Vec<_>>().join(", ");
                edits.push((token_end(&tokens[previous]), tokens[close].start, format!("values {rows}")));
            }
            continue;
        }
        if !insert && tuples.len() < 2 {
            let Some((start, end)) = tuples.first().copied() else {
                continue;
            };
            let Some(close) = matching_paren(&tokens, previous) else {
                continue;
            };
            if next_non_ws(&tokens, tuple_end_index(&tokens, end) + 1) != Some(close) {
                continue;
            }
            let close_indent = line_indent_of(text, tokens[previous].start);
            let values_indent = format!("{close_indent}    ");
            edits.push((
                token_end(&tokens[previous]),
                tokens[close].start,
                format!("\n{values_indent}values {}\n{close_indent}", collapse_whitespace(&text[start..end])),
            ));
            continue;
        }

        let line_start = text[..token.start].rfind('\n').map_or(0, |offset| offset + 1);
        let line_prefix = &text[line_start..token.start];
        let inline = !line_prefix.trim().is_empty();
        let line_indent = leading_whitespace(line_prefix);
        let header_indent = if inline { line_indent.to_owned() } else { line_indent_of(text, tokens[previous].start) };
        let indent = if insert {
            header_indent.clone()
        } else if inline {
            format!("{line_indent}    ")
        } else {
            line_prefix.to_owned()
        };

        if insert {
            if tuples.len() > 1 {
                edits.push((token_end(&tokens[previous]), token.start, format!("\n{header_indent}")));
            } else if !inline {
                edits.push((token_end(&tokens[previous]), token.start, " ".to_owned()));
            }
        }
        if derived && inline {
            edits.push((token_end(&tokens[previous]), token.start, format!("\n{indent}")));
        }

        let input = if insert { ValuesInput::CtxInsert } else { ValuesInput::CtxSubquery };
        let output = layout_values(input, text, token_end(token), &tuples, &indent);
        let (replacement, span) = match &output {
            ValuesOutput::Inline { text: replacement, span } => (replacement, *span),
            ValuesOutput::Exline { text: replacement, span } => (replacement, *span),
        };
        if text[span.start..span.end] != *replacement {
            edits.push((span.start, span.end, replacement.clone()));
        }

        if let ValuesOutput::Exline { span, .. } = output {
            let suffix = next_non_ws(&tokens, tuple_end_index(&tokens, span.end) + 1);
            if let Some(suffix) = suffix {
                if punct(&tokens[suffix], ")") {
                    if let Some(open) = previous_open(&tokens, index) {
                        let close_indent = line_indent_of(text, tokens[open].start);
                        edits.push((span.end, tokens[suffix].start, format!("\n{close_indent}")));
                    }
                } else if !punct(&tokens[suffix], ";") {
                    edits.push((span.end, tokens[suffix].start, " ".to_owned()));
                }
            }
        }
    }
    apply(text, edits)
}

/// 把一个 `values` 块渲染为输入/输出枚举；单行元组保持单行，多行元组逐行。
/// `indent` 在多行 INSERT 中与 `values` 对齐，在派生表中是括号内缩进。
fn layout_values(input: ValuesInput, text: &str, values_end: usize, tuples: &[(usize, usize)], indent: &str) -> ValuesOutput {
    if tuples.len() == 1 {
        let (start, end) = tuples[0];
        let tuple = &text[start..end];
        let replacement = match input {
            ValuesInput::CtxInsert => collapse_whitespace(tuple),
            ValuesInput::CtxSubquery => tuple.to_owned(),
        };
        return ValuesOutput::Inline { text: replacement, span: Span::new(start, end) };
    }

    let mut replacement = String::new();
    for (position, (start, end)) in tuples.iter().enumerate() {
        if position > 0 {
            replacement.push(',');
        }
        replacement.push('\n');
        replacement.push_str(indent);
        replacement.push_str(&collapse_whitespace(&text[*start..*end]));
    }
    let last_end = tuples.last().map_or(values_end, |(_, end)| *end);
    ValuesOutput::Exline { text: replacement, span: Span::new(values_end, last_end) }
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

/// 派生 `values` 是否位于 `insert ... values (...)` 的顶层元组内。
/// 元组本身按 M3 不折行，顶层 values 渲染会折叠其内容，这里不能再展开。
fn inside_insert_values_tuple(tokens: &[Token<'_>], index: usize) -> bool {
    let mut depth = 0usize;
    let mut insert = false;
    let mut values = false;
    for token in tokens.iter().take(index) {
        if token.kind == Kind::Punct {
            match token.text {
                "(" => depth += 1,
                ")" => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
        if depth == 0 {
            if word(token, "insert") {
                insert = true;
            } else if insert && word(token, "values") {
                values = true;
            }
        }
    }
    insert && values && depth > 0
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
    fn breaks_multi_row_insert_values_after_the_header() {
        let sql = "insert into t (a, b) values (1, 2), (3, 4);";
        let expected = "insert into t (a, b)\nvalues\n(1, 2),\n(3, 4);";
        assert!(!crate::sql::check(sql).is_empty(), "check should report values on the header line");
        assert_eq!(format(sql), expected);
        assert_eq!(format(expected), expected);
        assert!(crate::sql::check(expected).is_empty(), "{:?}", crate::sql::check(expected));
    }

    #[test]
    fn breaks_single_row_derived_values() {
        let sql = "select * from (values (1) ) t(c1);";
        let expected = "select * from (\n    values (1)\n) t (c1);";
        assert!(!crate::sql::check(sql).is_empty(), "check should report inline derived values");
        assert_eq!(format(sql), expected);
        assert_eq!(format(expected), expected);
        assert!(crate::sql::check(expected).is_empty());
    }

    #[test]
    fn breaks_multi_row_derived_values_without_indenting_rows() {
        let sql = "select * from (values (1, 2), (3, 4)) t(a, b);";
        let expected = "select * from (\n    values\n    (1, 2),\n    (3, 4)\n) t (a, b);";
        assert_eq!(format(sql), expected);
    }

    #[test]
    fn keeps_nested_values_in_insert_tuple_inline() {
        let sql = "insert into t values (1, (select i from ( values (3) ) foo (i)));";
        let expected = "insert into t values (1, (select i from (values (3)) foo (i)));";
        assert_eq!(format(sql), expected);
        assert_eq!(format(expected), expected);
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

    #[test]
    fn formats_multiple_insert_statements_independently() {
        let sql = "insert into a (x)\nvalues (1);\ninsert into b (y)\nvalues (2);";
        assert_eq!(format(sql), "insert into a (x) values (1);\ninsert into b (y) values (2);");
    }

    #[test]
    fn keeps_ctas_values_inline() {
        let sql = "create table t as (\n    values\n    (1),\n    (2)\n);";
        assert_eq!(format(sql), "create table t as (values (1), (2));");
    }
}
