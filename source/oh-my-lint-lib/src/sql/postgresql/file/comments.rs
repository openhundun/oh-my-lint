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
