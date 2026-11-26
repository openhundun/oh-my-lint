//! `comment on table/column` 块：去除夹注并对齐 `is`。

use crate::span::Span;
use crate::sql::lexer::{self, Kind};
use crate::sql::parse::{Dialect, classify_one};
use crate::sql::splitter;

/// 去掉语句前后的一切 `--` / `/* */` 注释；无法解析的语句正文原样保留。
pub(crate) fn strip_comments(text: &str, dialect: Dialect) -> String {
    let protected = protected_ranges(text, dialect);
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    for token in lexer::tokens(text) {
        if token.kind != Kind::Comment {
            continue;
        }
        let end = token.start + token.text.len();
        if protected.iter().any(|span| span.start <= token.start && end <= span.end) {
            continue;
        }
        let (start, end) = comment_line(text, token.start, end);
        edits.push((start, end, String::new()));
    }
    apply(text, edits)
}

fn protected_ranges(text: &str, dialect: Dialect) -> Vec<Span> {
    splitter::statements(text)
        .into_iter()
        .filter(|span| classify_one(&text[span.start..span.end], dialect).is_err())
        .collect()
}

/// 去掉可解析语句行尾的空白；无法解析的语句整行保持原样。
pub(crate) fn trim_trailing_spaces(text: &str, dialect: Dialect) -> String {
    let protected = protected_ranges(text, dialect);
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    let mut start = 0;
    while start < text.len() {
        let end = text[start..].find('\n').map_or(text.len(), |offset| start + offset + 1);
        let content_end = text[start..end].trim_end_matches(['\n', '\r']).len() + start;
        let trimmed = text[start..content_end].trim_end_matches([' ', '\t']).len() + start;
        let overlaps = protected.iter().any(|span| span.start < content_end && start < span.end);
        if trimmed < content_end && !overlaps {
            edits.push((trimmed, content_end, String::new()));
        }
        start = end;
    }
    apply(text, edits)
}

/// 独占一行的注释连行一起删；行尾注释只删注释本身。
fn comment_line(text: &str, start: usize, end: usize) -> (usize, usize) {
    let line_start = text[..start].rfind('\n').map_or(0, |index| index + 1);
    let line_end = text[end..].find('\n').map_or(text.len(), |index| end + index + 1);
    let before = &text[line_start..start];
    let after = &text[end..line_end];
    if before.trim().is_empty() && after.trim().is_empty() {
        (line_start, line_end)
    } else {
        (start, end)
    }
}

/// 语句之间最多保留一行空行；语句内部不动。
pub(crate) fn collapse_blank_lines(text: &str) -> String {
    let spans = splitter::statements(text);
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    let mut cursor = 0;
    for span in &spans {
        let gap = &text[cursor..span.start];
        let collapsed = collapse_gap(gap);
        if collapsed != gap {
            edits.push((cursor, span.start, collapsed));
        }
        cursor = span.end;
    }
    let gap = &text[cursor..];
    let collapsed = collapse_gap(gap);
    if collapsed != gap {
        edits.push((cursor, text.len(), collapsed));
    }
    finish(apply(text, edits))
}

/// 文件以 `\n` 结尾，且末尾只有这一行空行；原本没有换行的片段不补。
fn finish(text: String) -> String {
    if !text.ends_with('\n') {
        return text;
    }
    let trimmed = text.trim_end_matches(['\n', '\r']);
    if trimmed.is_empty() { "\n".to_owned() } else { format!("{trimmed}\n") }
}

fn collapse_gap(gap: &str) -> String {
    let mut output = String::with_capacity(gap.len());
    let mut blank = false;
    for line in gap.split_inclusive('\n') {
        if line.trim().is_empty() {
            if blank {
                continue;
            }
            blank = true;
        } else {
            blank = false;
        }
        output.push_str(line);
    }
    output
}

pub(crate) fn format_comment_blocks(text: &str, _dialect: Dialect) -> String {
    let spans = splitter::statements(text);
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    let mut index = 0;
    while index < spans.len() {
        if !is_comment_on(&text[spans[index].start..spans[index].end]) {
            index += 1;
            continue;
        }

        let first_index = index;
        let mut next = index + 1;
        while next < spans.len() && is_comment_on(&text[spans[next].start..spans[next].end]) {
            next += 1;
        }
        let first = spans[first_index].start;
        let last = spans[next - 1];
        let Some(block_end) = statement_end(text, last) else {
            index = next;
            continue;
        };
        if let Some(block) = align(&text[first..block_end]) {
            edits.push((first, block_end, block));
        }
        index = next;
    }
    apply(text, edits)
}

/// `Span` 后面的分号也属于语句，用于删除/替换整块。
fn statement_end(text: &str, span: Span) -> Option<usize> {
    text[span.end..].find(';').map(|offset| span.end + offset + 1)
}

fn is_comment_on(statement: &str) -> bool {
    let lower = statement.trim_start().to_ascii_lowercase();
    lower.starts_with("comment on table ") || lower.starts_with("comment on column ")
}

fn align(block: &str) -> Option<String> {
    let statements = splitter::statements(block);
    let mut parts = Vec::new();
    for statement in statements {
        let text = &block[statement.start..statement.end];
        parts.push(parse_comment(text)?);
    }
    let width = parts.iter().map(|(prefix, _)| prefix.len()).max().unwrap_or(0) + 1;
    let mut output = String::new();
    for (index, (prefix, value)) in parts.iter().enumerate() {
        if index > 0 {
            output.push('\n');
        }
        output.push_str(prefix);
        output.push_str(&" ".repeat(width - prefix.len()));
        output.push_str("is ");
        output.push_str(value);
        output.push(';');
    }
    Some(output)
}

fn parse_comment(statement: &str) -> Option<(String, String)> {
    let text = statement.trim();
    let lower = text.to_ascii_lowercase();
    let (table, name_start) = if lower.starts_with("comment on table") {
        (true, "comment on table".len())
    } else if lower.starts_with("comment on column") {
        (false, "comment on column".len())
    } else {
        return None;
    };
    let is = text[name_start..].find(" is ")? + name_start;
    let name = text[name_start..is].trim();
    if name.is_empty() {
        return None;
    }
    let prefix = if table { format!("comment on table  {name}") } else { format!("comment on column {name}") };
    let value = text[is + 4..].trim_end();
    if value.is_empty() {
        return None;
    }
    Some((prefix, value.to_owned()))
}

fn apply(text: &str, mut edits: Vec<(usize, usize, String)>) -> String {
    if edits.is_empty() {
        return text.to_owned();
    }
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.0));
    let mut output = text.to_owned();
    for (start, end, replacement) in edits {
        if start <= end && end <= output.len() {
            output.replace_range(start..end, &replacement);
        }
    }
    output
}

#[cfg(test)]
mod tests {
    #[test]
    fn aligns_and_removes_comments_between_table_and_columns() {
        let sql = "create table t (id bigint, name varchar(10));\n\
-- 章节\n\
-- 表注释\n\
comment on table t is 'table';\n\
-- 字段注释\n\
comment on column t.id is 'id';\n\
comment on column t.name is 'name';\n";
        let formatted = crate::sql::format_with(sql, crate::sql::Dialect::Postgres).expect("format");
        assert_eq!(
            formatted,
            "create table t (id bigint, name varchar(10));\n\
comment on table  t      is 'table';\n\
comment on column t.id   is 'id';\n\
comment on column t.name is 'name';\n"
        );
    }

    #[test]
    fn aligns_comments_after_an_intervening_statement() {
        let sql = "create table t (id bigint);\n\
select 1;\n\
comment on table t is 'table';\n\
comment on column t.id is 'id';\n";
        let formatted = crate::sql::format_with(sql, crate::sql::Dialect::Postgres).expect("format");
        assert!(formatted.contains("comment on table  t    is 'table';\ncomment on column t.id is 'id';"), "{formatted}");
    }

    #[test]
    fn removes_comments_after_the_block() {
        let sql = "create table t (id bigint primary key);\n\
comment on table t is 'table';\n\
-- drop\n\
select 1;\n";
        let formatted = crate::sql::format_with(sql, crate::sql::Dialect::Postgres).expect("format");
        assert!(!formatted.contains("-- drop"), "{formatted}");
        assert!(formatted.contains("select 1;"), "{formatted}");
    }

    #[test]
    fn keeps_at_most_one_blank_line_between_statements() {
        let sql = "select 1;\n\n\nselect 2;\n\n\n\nselect 3;\n";
        let formatted = crate::sql::format_with(sql, crate::sql::Dialect::Postgres).expect("format");
        assert_eq!(formatted, "select 1;\n\nselect 2;\n\nselect 3;\n");
    }

    #[test]
    fn ends_with_exactly_one_newline() {
        let sql = "select 1;\n\n\n";
        let formatted = crate::sql::format_with(sql, crate::sql::Dialect::Postgres).expect("format");
        assert_eq!(formatted, "select 1;\n");
    }

    #[test]
    fn removes_comments_before_unparseable_statements() {
        let broken = "select a from t where (x between (select 1 from u)) and (select 2 from u);";
        let sql = format!("-- drop\n{broken}\n-- drop2\nselect 1;\n");
        let formatted = crate::sql::format_with(&sql, crate::sql::Dialect::Postgres).expect("format");
        assert!(!formatted.contains("-- drop"), "{formatted}");
        assert!(formatted.contains(broken), "{formatted}");
    }
}
