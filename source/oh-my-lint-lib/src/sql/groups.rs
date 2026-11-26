//! 同一张表的 DDL 分组（D9）：排序、紧排与索引对齐。

use crate::sql::lexer::{Kind, Token};
use crate::sql::parse::{Dialect, StatementKind, classify_one};
use crate::sql::splitter;

/// 分组内的语句角色；值越小越靠前。
#[derive(Clone, Debug, Eq, PartialEq)]
struct Member {
    rank: u8,
    table: Option<String>,
}

pub(crate) fn group_ddl(text: &str, dialect: Dialect) -> String {
    let spans = splitter::statements(text);
    let members: Vec<Option<Member>> = spans.iter().map(|span| member(&text[span.start..span.end], dialect)).collect();
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    let mut index = 0;
    while index < spans.len() {
        let Some(head) = &members[index] else {
            index += 1;
            continue;
        };
        if head.rank != 0 {
            index += 1;
            continue;
        }
        let Some(table) = &head.table else {
            index += 1;
            continue;
        };
        let mut run = vec![index];
        let mut next = index + 1;
        while next < spans.len() {
            let Some(member) = &members[next] else {
                break;
            };
            let same_table = member.table.as_deref() == Some(table.as_str()) && member.rank > 0;
            if same_table || member.rank == 5 {
                run.push(next);
                next += 1;
            } else {
                break;
            }
        }
        if run.len() > 1 {
            let columns = create_table_columns(&text[spans[index].start..spans[index].end]);
            let mut ordered = run.clone();
            ordered.sort_by_key(|index| {
                let member = members[*index].as_ref();
                let rank = member.map_or(u8::MAX, |member| member.rank);
                let column = if rank == 1 {
                    comment_position(&text[spans[*index].start..spans[*index].end], &columns)
                } else {
                    0
                };
                (rank, column)
            });
            let first = spans[*run.first().expect("non-empty run")].start;
            let last_span = spans[*run.last().expect("non-empty run")];
            let last = statement_end(text, last_span);
            let statements: Vec<String> = ordered
                .iter()
                .map(|index| {
                    let mut statement = text[spans[*index].start..spans[*index].end].trim_end().to_owned();
                    if !statement.ends_with(';') {
                        statement.push(';');
                    }
                    statement
                })
                .collect();
            let statements = align_indexes(statements);
            edits.push((first, last, statements.join("\n")));
        }
        index = next;
    }
    apply(text, edits)
}

fn member(statement: &str, dialect: Dialect) -> Option<Member> {
    let toks = significant(statement);
    if word_at(&toks, 0, "alter") && word_at(&toks, 1, "sequence") {
        return Some(Member { rank: 5, table: None });
    }
    let kind = classify_one(statement, dialect).ok()?;
    match kind {
        StatementKind::CreateTable => Some(Member { rank: 0, table: create_table_name(&toks) }),
        StatementKind::CreateIndex => {
            let unique = toks.get(1).is_some_and(|token| word(token, "unique")) || toks.get(2).is_some_and(|token| word(token, "unique"));
            Some(Member { rank: if unique { 2 } else { 3 }, table: create_index_table(&toks) })
        }
        StatementKind::AlterTable => Some(Member { rank: 4, table: alter_table_name(&toks) }),
        StatementKind::Other => {
            if word_at(&toks, 0, "comment") && word_at(&toks, 1, "on") {
                Some(Member { rank: 1, table: comment_table(&toks) })
            } else {
                None
            }
        }
        StatementKind::Query | StatementKind::Delete | StatementKind::Explain | StatementKind::Insert => None,
    }
}

fn create_table_name(toks: &[Token<'_>]) -> Option<String> {
    let mut index = 1;
    while index < toks.len() && !word_at(toks, index, "table") {
        index += 1;
    }
    index += 1;
    if word_at(toks, index, "if") && word_at(toks, index + 1, "not") && word_at(toks, index + 2, "exists") {
        index += 3;
    }
    qualified_last(toks, index)
}

fn alter_table_name(toks: &[Token<'_>]) -> Option<String> {
    let mut index = 2;
    if word_at(toks, index, "if") && word_at(toks, index + 1, "exists") {
        index += 2;
    }
    if word_at(toks, index, "only") {
        index += 1;
    }
    qualified_last(toks, index)
}

fn create_index_table(toks: &[Token<'_>]) -> Option<String> {
    let mut depth = 0usize;
    let mut on = None;
    for (index, token) in toks.iter().enumerate() {
        if token.kind == Kind::Punct {
            match token.text {
                "(" => depth += 1,
                ")" => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
        if depth == 0 && index > 1 && word(token, "on") {
            on = Some(index);
            break;
        }
    }
    qualified_last(toks, on? + 1)
}

fn comment_table(toks: &[Token<'_>]) -> Option<String> {
    if word_at(toks, 2, "table") {
        return qualified_last(toks, 3);
    }
    if !word_at(toks, 2, "column") {
        return None;
    }
    let mut index = 3;
    let mut parts = Vec::new();
    while index < toks.len() {
        if !is_ident(&toks[index]) {
            break;
        }
        parts.push(toks[index].text.to_owned());
        index += 1;
        if !punct_at(toks, index, ".") {
            break;
        }
        index += 1;
    }
    parts.get(parts.len().checked_sub(2)?).cloned()
}

fn qualified_last(toks: &[Token<'_>], mut index: usize) -> Option<String> {
    let mut last = None;
    while index < toks.len() {
        if !is_ident(&toks[index]) {
            break;
        }
        last = Some(toks[index].text.to_owned());
        index += 1;
        if !punct_at(toks, index, ".") {
            break;
        }
        index += 1;
    }
    last
}

/// `create table` 的字段顺序；表级约束与 `like` 不算字段。
fn create_table_columns(statement: &str) -> Vec<String> {
    let toks = significant(statement);
    let Some(table) = toks.iter().position(|token| word(token, "table")) else {
        return Vec::new();
    };
    let mut index = table + 1;
    if word_at(&toks, index, "if") && word_at(&toks, index + 1, "not") && word_at(&toks, index + 2, "exists") {
        index += 3;
    }
    let Some(open) = (index..toks.len()).find(|index| punct_at(&toks, *index, "(")) else {
        return Vec::new();
    };
    let Some(close) = matching_paren(&toks, open) else {
        return Vec::new();
    };
    let body = &statement[token_end(&toks[open])..toks[close].start];
    let mut columns = Vec::new();
    for element in splitter::split_elements("table", body) {
        let toks = significant(&element.text);
        let Some(first) = toks.first() else {
            continue;
        };
        if !is_ident(first) {
            continue;
        }
        if first.kind == Kind::Word && matches!(first.text.to_ascii_lowercase().as_str(), "constraint" | "primary" | "unique" | "check" | "foreign" | "exclude" | "like") {
            continue;
        }
        columns.push(first.text.to_owned());
    }
    columns
}

/// `comment on column` 在 `create table` 字段列表中的位置；表注释排最前。
fn comment_position(statement: &str, columns: &[String]) -> usize {
    let toks = significant(statement);
    if word_at(&toks, 2, "table") {
        return 0;
    }
    if !word_at(&toks, 2, "column") {
        return usize::MAX;
    }
    let Some(column) = comment_column(&toks) else {
        return usize::MAX;
    };
    columns.iter().position(|name| name.eq_ignore_ascii_case(&column)).map_or(columns.len() + 1, |index| index + 1)
}

fn comment_column(toks: &[Token<'_>]) -> Option<String> {
    if !(word_at(toks, 0, "comment") && word_at(toks, 1, "on") && word_at(toks, 2, "column")) {
        return None;
    }
    let mut index = 3;
    let mut last = None;
    while index < toks.len() {
        if !is_ident(&toks[index]) {
            break;
        }
        last = Some(toks[index].text.to_owned());
        index += 1;
        if !punct_at(toks, index, ".") {
            break;
        }
        index += 1;
    }
    last
}

fn matching_paren(toks: &[Token<'_>], open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (index, token) in toks.iter().enumerate().skip(open) {
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

/// 同组索引对齐：索引名、`on`、`where` 各自落在同一列。
fn align_indexes(statements: Vec<String>) -> Vec<String> {
    let parts: Vec<Option<IndexParts<'_>>> = statements.iter().map(|statement| index_parts(statement)).collect();
    let head_width = parts.iter().flatten().map(|part| part.head.chars().count()).max().unwrap_or(0);
    let options_width = parts.iter().flatten().map(|part| part.options.chars().count()).max().unwrap_or(0);
    let name_start = head_width + 1 + if options_width == 0 { 0 } else { options_width + 1 };
    let name_width = parts.iter().flatten().map(|part| part.name.chars().count()).max().unwrap_or(0);
    let on_column = name_start + name_width + 1;

    let mut rendered = Vec::with_capacity(statements.len());
    for (statement, part) in statements.iter().zip(parts.iter()) {
        let Some(part) = part else {
            rendered.push(statement.clone());
            continue;
        };
        let mut line = String::new();
        line.push_str(part.head);
        push_spaces(&mut line, head_width.saturating_sub(part.head.chars().count()));
        line.push(' ');
        if options_width > 0 {
            line.push_str(part.options);
            push_spaces(&mut line, options_width.saturating_sub(part.options.chars().count()));
            line.push(' ');
        }
        line.push_str(part.name);
        let width = line.chars().count();
        push_spaces(&mut line, on_column.saturating_sub(width));
        line.push_str(part.on);
        rendered.push(line);
    }

    let where_column = rendered
        .iter()
        .zip(parts.iter())
        .filter(|(_, part)| part.as_ref().is_some_and(|part| part.where_clause.is_some()))
        .map(|(line, _)| line.chars().count())
        .max()
        .unwrap_or(0)
        + 1;
    rendered
        .into_iter()
        .zip(parts.iter())
        .map(|(mut line, part)| {
            if let Some(part) = part
                && let Some(where_clause) = part.where_clause
            {
                let width = line.chars().count();
                push_spaces(&mut line, where_column.saturating_sub(width));
                line.push_str(where_clause);
            }
            line
        })
        .collect()
}

fn push_spaces(output: &mut String, count: usize) {
    output.push_str(&" ".repeat(count));
}

struct IndexParts<'a> {
    head: &'a str,
    options: &'a str,
    name: &'a str,
    on: &'a str,
    where_clause: Option<&'a str>,
}

fn index_parts(statement: &str) -> Option<IndexParts<'_>> {
    let toks = significant(statement);
    if !word_at(&toks, 0, "create") {
        return None;
    }
    let index_token = toks.iter().position(|token| word(token, "index"))?;
    let on_token = depth_zero_word(&toks, "on", index_token + 1)?;
    let name = toks.get(on_token.checked_sub(1)?)?;
    if !is_ident(name) {
        return None;
    }
    let head_end = token_end(&toks[index_token]);
    let head = statement[..head_end].trim_end();
    let options = statement[head_end..name.start].trim();
    let where_token = depth_zero_word(&toks, "where", on_token + 1);
    let on_end = where_token.map_or(statement.len(), |index| toks[index].start);
    let on = statement[toks[on_token].start..on_end].trim_end();
    let where_clause = where_token.map(|index| statement[toks[index].start..].trim_start());
    Some(IndexParts { head, options, name: &statement[name.start..token_end(name)], on, where_clause })
}

fn depth_zero_word(toks: &[Token<'_>], expected: &str, start: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (index, token) in toks.iter().enumerate().skip(start) {
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

fn statement_end(text: &str, span: crate::span::Span) -> usize {
    text[span.end..].find(';').map_or(span.end, |offset| span.end + offset + 1)
}

fn significant(statement: &str) -> Vec<Token<'_>> {
    crate::sql::lexer::tokens(statement)
        .into_iter()
        .filter(|token| !matches!(token.kind, Kind::Whitespace | Kind::Comment))
        .collect()
}

fn token_end(token: &Token<'_>) -> usize {
    token.start + token.text.len()
}

fn word(token: &Token<'_>, expected: &str) -> bool {
    token.kind == Kind::Word && token.text.eq_ignore_ascii_case(expected)
}

fn word_at(toks: &[Token<'_>], index: usize, expected: &str) -> bool {
    toks.get(index).is_some_and(|token| word(token, expected))
}

fn is_ident(token: &Token<'_>) -> bool {
    matches!(token.kind, Kind::Word | Kind::QuotedIdent)
}

fn punct_at(toks: &[Token<'_>], index: usize, expected: &str) -> bool {
    toks.get(index).is_some_and(|token| token.kind == Kind::Punct && token.text == expected)
}

fn apply(text: &str, mut edits: Vec<(usize, usize, String)>) -> String {
    if edits.is_empty() {
        return text.to_owned();
    }
    edits.sort_by_key(|(start, _, _)| *start);
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
    fn groups_table_ddl_and_aligns_indexes() {
        let sql = "create table t (id bigint, primary key (id));\n\n\
alter sequence t_id_seq restart with 1;\n\n\
create index if not exists idx_t_b on t (b);\n\
create index if not exists idx_t_long on t (long);\n\n\
comment on table t is 't';\n\
comment on column t.id is 'id';\n";
        let expected = "create table t (\n    id bigint not null,\n    primary key (id)\n);\n\
comment on table  t    is 't';\n\
comment on column t.id is 'id';\n\
create index if not exists idx_t_b    on t (b);\n\
create index if not exists idx_t_long on t (long);\n\
alter sequence t_id_seq restart with 1;\n";
        let formatted = format(sql);
        assert_eq!(formatted, expected);
        assert_eq!(format(&formatted), formatted);
    }

    #[test]
    fn aligns_mixed_index_forms_and_where() {
        let sql = "create table t (id bigint, name text);\n\
create unique index if not exists uk_t on t (id) where id > 0;\n\
create index if not exists idx_t_name on t (name) where name is not null;\n";
        let expected = "create table t (id bigint, name text);\n\
create unique index if not exists uk_t       on t (id)   where id > 0;\n\
create index        if not exists idx_t_name on t (name) where name is not null;\n";
        assert_eq!(format(sql), expected);
    }

    #[test]
    fn orders_column_comments_by_table_columns() {
        let sql = "create table t (b int, a int);\n\
comment on column t.a is 'a';\n\
comment on column t.b is 'b';\n";
        let expected = "create table t (b int, a int);\n\
comment on column t.b is 'b';\n\
comment on column t.a is 'a';\n";
        assert_eq!(format(sql), expected);
    }

    #[test]
    fn does_not_group_across_another_table() {
        let sql = "create table a (id bigint);\n\
create table b (id bigint);\n\
create index if not exists idx_a on a (id);\n";
        let formatted = format(sql);
        assert!(formatted.contains("create table a (id bigint);\ncreate table b (id bigint);"), "{formatted}");
        assert!(formatted.contains("create table b (id bigint);\ncreate index if not exists idx_a on a (id);"), "{formatted}");
    }
}
