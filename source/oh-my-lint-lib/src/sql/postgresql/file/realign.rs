//! 文件级 `comment on` 块对齐。

use crate::span::Span;
use crate::sql::parse::Dialect;
use crate::sql::splitter;

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
