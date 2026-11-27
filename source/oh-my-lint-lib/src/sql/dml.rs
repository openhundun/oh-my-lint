//! DML 版式：`DELETE`（M1）与 `EXPLAIN`（M2）。

use sqlparser::ast::{Delete, FromTable, SetExpr, Spanned as _, Statement};

use crate::sql::decision;
use crate::sql::lexer::{self, Kind};
use crate::sql::parse::{self, Dialect};
use crate::sql::splitter::{self, Connector, Element};
use crate::sql::token;

/// `EXPLAIN` 里只接受布尔值的选项；其余选项原样保留。
const BOOLEAN_OPTIONS: [&str; 11] = ["analyze", "analyse", "verbose", "costs", "settings", "buffers", "wal", "timing", "summary", "generic_plan", "memory"];

/// 渲染 `DELETE`、`EXPLAIN`；无法渲染时返回 `None`。
pub(crate) fn layout(sql: &str, dialect: Dialect) -> Option<String> {
    lexer::with_dialect(dialect, || layout_inner(sql, dialect))
}

fn layout_inner(sql: &str, dialect: Dialect) -> Option<String> {
    let normalized = token::normalize(sql.trim());
    let normalized = normalized.trim_end_matches(';').trim_end();
    if normalized.is_empty() {
        return None;
    }
    let statements = parse::parse(normalized, dialect).ok()?;
    let output = match statements.as_slice() {
        [Statement::Delete(delete)] => layout_delete(normalized, delete)?,
        [Statement::Update(_)] => layout_update(normalized, dialect)?,
        [Statement::Insert(_)] => layout_insert(normalized, dialect)?,
        [Statement::Explain { statement, .. }] => layout_explain(normalized, statement, dialect)?,
        [Statement::Query(query)] => match query.body.as_ref() {
            SetExpr::Delete(Statement::Delete(delete)) => layout_delete(normalized, delete)?,
            SetExpr::Update(_) => layout_update(normalized, dialect)?,
            SetExpr::Insert(_) => layout_insert(normalized, dialect)?,
            _ => return None,
        },
        _ => return None,
    };
    if parse::parse(&output, dialect).is_err() {
        return None;
    }
    Some(output)
}

/// `INSERT ... SELECT`：保留 insert 头，把查询体交给 DQL 版式。
fn layout_insert(text: &str, dialect: Dialect) -> Option<String> {
    let words = top_words(text);
    let insert = words.iter().find(|word| word.text == "insert")?;
    let select = words
        .iter()
        .enumerate()
        .skip_while(|(_, word)| word.start <= insert.start)
        .find(|(_, word)| word.text == "select" || word.text == "with")
        .map(|(index, _)| index)?;
    let header = collapse(&text[insert.start..words[select].start]);
    let query = &text[words[select].start..];
    if query.trim().is_empty() {
        return None;
    }
    let flat = flatten_query(query)?;
    let formatted = crate::sql::fix::format_with(&flat, dialect).ok()?;
    let body = format!("{header}\n{formatted}");
    let output = if insert.start > 0 {
        let cte = format_cte_prefix(&text[..insert.start], dialect)?;
        format!("{cte}\n{body}")
    } else {
        body
    };
    parse::parse(&output, dialect).ok()?;
    Some(output)
}

/// `with` 框架单独排版后拼回 DML：借用 `select 1` 让 DQL 只处理 CTE 部分。
fn format_cte_prefix(prefix: &str, dialect: Dialect) -> Option<String> {
    let synthetic = format!("{}\nselect 1", prefix.trim_end());
    let formatted = crate::sql::fix::format_with(&synthetic, dialect).ok()?;
    let trimmed = formatted.trim_end();
    let cut = trimmed.rfind("\nselect 1")?;
    let cte = trimmed[..cut].trim_end();
    (!cte.is_empty()).then(|| cte.to_owned())
}

/// `UPDATE`：`set` 按 select 元素规则，其他子句超过 1 个元素即整句分行。
fn layout_update(text: &str, dialect: Dialect) -> Option<String> {
    let words = top_words(text);
    let update = words.iter().position(|word| word.text == "update")?;
    let set = words.iter().enumerate().skip(update + 1).find(|(_, word)| word.text == "set").map(|(index, _)| index)?;
    let from = words.iter().enumerate().skip(set + 1).find(|(_, word)| word.text == "from").map(|(index, _)| index);
    let after_from = from.map_or(set + 1, |index| index + 1);
    let where_clause = words.iter().enumerate().skip(after_from).find(|(_, word)| word.text == "where").map(|(index, _)| index);
    let after_where = where_clause.map_or(after_from, |index| index + 1);
    let returning = words.iter().enumerate().skip(after_where).find(|(_, word)| word.text == "returning").map(|(index, _)| index);

    let update_head = collapse(&text[words[update].start..words[set].start]);
    let set_end = [from, where_clause, returning].into_iter().flatten().map(|index| words[index].start).min().unwrap_or(text.len());
    let set_body = &text[words[set].end..set_end];
    let from_body = from.map(|index| {
        let end = [where_clause, returning].into_iter().flatten().map(|next| words[next].start).min().unwrap_or(text.len());
        &text[words[index].end..end]
    });
    let where_body = where_clause.map(|index| {
        let end = returning.map_or(text.len(), |next| words[next].start);
        &text[words[index].end..end]
    });
    let returning_body = returning.map(|index| &text[words[index].end..]);

    let cte = if words[update].start > 0 {
        Some(format_cte_prefix(&text[..words[update].start], dialect)?)
    } else {
        None
    };
    let set_elements = splitter::split_elements("select", set_body);
    let set_inline = set_elements.len() <= 3 && !set_elements.iter().any(|element| decision::is_complex(&element.text, "select"));
    let from_inline = from_body.is_none_or(|body| splitter::split_elements("from", body).len() <= 1 && !decision::has_join(body));
    let where_elements = where_body.map(|body| splitter::split_elements("where", body));
    let where_inline = where_elements.as_ref().is_none_or(|elements| elements.len() <= 1);
    let returning_lines = returning_body.map(returning_lines);
    let returning_inline = returning_lines.as_ref().is_none_or(|lines| lines.len() == 1);

    let set_lines = update_set_lines(set_body);
    let from_lines = from_body.map(|body| table_lines(body, "from"));
    let where_lines = where_body.map(where_lines);
    let mut lines = Vec::new();
    if let Some(cte) = &cte {
        lines.extend(cte.lines().map(ToOwned::to_owned));
    }
    lines.push(update_head.clone());
    lines.extend(set_lines.clone());
    if let Some(from_lines) = &from_lines {
        lines.extend(from_lines.clone());
    }
    if let Some(where_lines) = &where_lines {
        lines.extend(where_lines.clone());
    }
    if let Some(returning_lines) = &returning_lines {
        lines.extend(returning_lines.clone());
    }
    let simple = cte.is_none() && set_inline && from_inline && where_inline && returning_inline;
    let output = if simple {
        let mut parts = vec![update_head];
        parts.extend(set_lines);
        if let Some(from_lines) = from_lines {
            parts.extend(from_lines);
        }
        if let Some(where_lines) = where_lines {
            parts.extend(where_lines);
        }
        if let Some(returning_lines) = returning_lines {
            parts.extend(returning_lines);
        }
        parts.join(" ")
    } else {
        lines.join("\n")
    };
    parse::parse(&output, dialect).ok()?;
    Some(output)
}

/// `set` 赋值：≤3 个简单表达式同行，否则 `set` 独占一行、每项一行。
fn update_set_lines(body: &str) -> Vec<String> {
    let elements = splitter::split_elements("select", body);
    if elements.is_empty() {
        return vec!["set".to_owned()];
    }
    let broken = elements.len() >= 4 || elements.iter().any(|element| decision::is_complex(&element.text, "select"));
    if !broken {
        let joined = elements.iter().map(|element| collapse(&element.text)).collect::<Vec<_>>().join(", ");
        return vec![format!("set {joined}")];
    }
    let mut lines = vec!["set".to_owned()];
    for (index, element) in elements.iter().enumerate() {
        let comma = if index + 1 < elements.len() { "," } else { "" };
        lines.push(format!("    {}{comma}", collapse(&element.text)));
    }
    lines
}

/// `from`/`using` 的表列表；join 独行，首表留在关键字行。
fn table_lines(body: &str, keyword: &str) -> Vec<String> {
    let starts = join_starts(body);
    if starts.is_empty() {
        return vec![format!("{keyword} {}", collapse(body))];
    }
    let first = collapse(&body[..starts[0]]);
    let mut lines = vec![if first.is_empty() { keyword.to_owned() } else { format!("{keyword} {first}") }];
    for (index, start) in starts.iter().enumerate() {
        let end = starts.get(index + 1).copied().unwrap_or(body.len());
        lines.push(collapse(&body[*start..end]));
    }
    lines
}

/// 压掉查询里的换行，让 DQL 版式从元素列表重新渲染；字符串与注释原样。
fn flatten_query(query: &str) -> Option<String> {
    if lexer::has_comment(query) {
        return None;
    }
    let mut flat = String::with_capacity(query.len());
    for token in lexer::tokens(query) {
        if token.kind == Kind::Whitespace {
            if !flat.ends_with(' ') {
                flat.push(' ');
            }
        } else {
            flat.push_str(token.text);
        }
    }
    let flat = flat.trim();
    (!flat.is_empty()).then(|| token::normalize(flat))
}

/// `DELETE` 的顶层子句，顺序固定。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DeletePart {
    With,
    DeleteFrom,
    Using,
    Where,
    Returning,
}

/// 一个子句的字节区间：关键字起点、正文起点、正文终点（下一个子句起点）。
#[derive(Clone, Copy, Debug)]
struct Part {
    kind: DeletePart,
    keyword: usize,
    body: usize,
    end: usize,
}

impl Part {
    fn body(self, text: &str) -> &str {
        &text[self.body..self.end]
    }
}

/// 一个深度 0 的单词。
struct Word {
    start: usize,
    end: usize,
    text: String,
}

fn layout_delete(text: &str, delete: &Delete) -> Option<String> {
    if !delete_supported(delete) {
        return None;
    }
    let parts = delete_parts(text)?;
    if let Some(with) = parts.iter().find(|part| part.kind == DeletePart::With)
        && !cte_bodies_simple(with.body(text))
    {
        let words = top_words(text);
        let delete_word = words.iter().find(|word| word.text == "delete")?;
        let cte = format_cte_prefix(&text[..delete_word.start], crate::sql::lexer::current_dialect())?;
        let main = &text[delete_word.start..];
        let formatted = layout_delete(main, delete)?;
        return Some(format!("{cte}\n{formatted}"));
    }
    let collected: Vec<Vec<String>> = parts.iter().map(|part| render_delete_part(text, *part)).collect();
    if delete_simple(text, &parts) {
        let mut pieces = Vec::new();
        for lines in collected {
            pieces.extend(lines);
        }
        return Some(pieces.join(" "));
    }
    let mut lines = Vec::new();
    for part_lines in collected {
        lines.extend(part_lines);
    }
    Some(lines.join("\n"))
}

/// 只渲染 M1 列出的形态；其他方言扩展（多表、order by、limit 等）交给上层跳过。
fn delete_supported(delete: &Delete) -> bool {
    if !delete.tables.is_empty()
        || !delete.optimizer_hints.is_empty()
        || delete.on_cluster.is_some()
        || delete.output.is_some()
        || !delete.order_by.is_empty()
        || delete.limit.is_some()
        || delete.settings.is_some()
    {
        return false;
    }
    let FromTable::WithFromKeyword(from) = &delete.from else {
        return false;
    };
    !from.is_empty() && from.iter().all(|table| table.joins.is_empty())
}

/// 切出 `with`、`delete from`、`using`、`where`、`returning`。
fn delete_parts(text: &str) -> Option<Vec<Part>> {
    let words = top_words(text);
    let delete_index = words.iter().position(|word| word.text == "delete")?;
    let from_word = words.get(delete_index + 1)?;
    if from_word.text != "from" {
        return None;
    }
    let using_index = words.iter().enumerate().skip(delete_index + 2).find(|(_, word)| word.text == "using").map(|(index, _)| index);
    let after_using = using_index.map_or(delete_index + 2, |index| index + 1);
    let where_index = words.iter().enumerate().skip(after_using).find(|(_, word)| word.text == "where").map(|(index, _)| index);
    let after_where = where_index.map_or(after_using, |index| index + 1);
    let returning_index = words.iter().enumerate().skip(after_where).find(|(_, word)| word.text == "returning").map(|(index, _)| index);

    let mut parts: Vec<Part> = Vec::new();
    if words.first().is_some_and(|word| word.start == 0 && word.text == "with") {
        parts.push(Part { kind: DeletePart::With, keyword: 0, body: words[0].end, end: 0 });
    }
    parts.push(Part { kind: DeletePart::DeleteFrom, keyword: words[delete_index].start, body: from_word.end, end: 0 });
    for (index, kind) in [(using_index, DeletePart::Using), (where_index, DeletePart::Where), (returning_index, DeletePart::Returning)] {
        if let Some(index) = index {
            parts.push(Part { kind, keyword: words[index].start, body: words[index].end, end: 0 });
        }
    }
    for index in 0..parts.len() {
        parts[index].end = parts.get(index + 1).map_or(text.len(), |next| next.keyword);
    }
    Some(parts)
}

/// 简单判定同 R11：`returning` ≤3 且不触发 R7，其余子句元素 ≤1，`using` 无 join。
///
/// CTE 在调用前已确认各体简单。
fn delete_simple(text: &str, parts: &[Part]) -> bool {
    for part in parts {
        let body = part.body(text);
        match part.kind {
            DeletePart::With => {}
            DeletePart::DeleteFrom => {
                if splitter::split_elements("from", body).len() > 1 || decision::has_join(body) {
                    return false;
                }
            }
            DeletePart::Using => {
                if splitter::split_elements("from", body).len() > 1 || decision::has_join(body) {
                    return false;
                }
            }
            DeletePart::Where => {
                if splitter::split_elements("where", body).len() > 1 {
                    return false;
                }
            }
            DeletePart::Returning => {
                let elements = returning_elements(body);
                if elements.len() > 3 || decision::needs_break("select", &elements) {
                    return false;
                }
            }
        }
    }
    true
}

fn render_delete_part(text: &str, part: Part) -> Vec<String> {
    let body = part.body(text);
    match part.kind {
        DeletePart::With => vec![format!("with {}", collapse(body))],
        DeletePart::DeleteFrom => vec![format!("delete from {}", collapse(body))],
        DeletePart::Using => using_lines(body),
        DeletePart::Where => where_lines(body),
        DeletePart::Returning => returning_lines(body),
    }
}

/// `using` 按 R6/R23：join 随 `using` 缩进独占一行，首表留同行。
fn using_lines(body: &str) -> Vec<String> {
    table_lines(body, "using")
}

/// `where` 按 R8/R19：≥4 个条件或复杂时逐条件断行，否则整行并丢弃锚点。
fn where_lines(body: &str) -> Vec<String> {
    let elements = splitter::split_elements("where", body);
    if elements.len() <= 1 || is_current_of(body) {
        return vec![format!("where {}", collapse(body))];
    }
    let broken = elements.len() >= 4 || elements.iter().any(|element| decision::is_complex(&element.text, "where"));
    if !broken {
        return vec![format!("where {}", join_conditions(&elements))];
    }
    let mut lines = vec![format!("where {}", anchor(&elements))];
    for element in &elements {
        let connector = if element.connector == Connector::None { Connector::And } else { element.connector };
        lines.push(format!("    {} {}", decision::connector_word(connector), collapse(&element.text)));
    }
    lines
}

fn is_current_of(body: &str) -> bool {
    let mut words = lexer::tokens(body).into_iter().filter(|token| token.kind == Kind::Word).map(|token| token.text.to_ascii_lowercase());
    words.next().as_deref() == Some("current") && words.next().as_deref() == Some("of")
}

fn anchor(elements: &[Element]) -> &'static str {
    let oracle = elements.get(1).or_else(|| elements.first());
    if oracle.is_some_and(|element| element.connector == Connector::Or) { "1 = 0" } else { "1 = 1" }
}

fn join_conditions(elements: &[Element]) -> String {
    let mut joined = String::new();
    for (index, element) in elements.iter().enumerate() {
        if index > 0 {
            joined.push(' ');
            joined.push_str(decision::connector_word(element.connector));
            joined.push(' ');
        }
        joined.push_str(&collapse(&element.text));
    }
    joined
}

/// `returning` 按 R7；别名必须写 `as`。
fn returning_lines(body: &str) -> Vec<String> {
    let elements = returning_elements(body);
    if elements.is_empty() {
        return vec![format!("returning {}", collapse(body))];
    }
    let broken = elements.len() >= 4
        || elements.iter().any(|element| decision::is_complex(&element.text, "select"))
        || (elements.len() == 3 && elements.iter().all(|element| decision::has_alias(&element.text)));
    if !broken {
        let joined = elements.iter().map(|element| element.text.as_str()).collect::<Vec<_>>().join(", ");
        return vec![format!("returning {joined}")];
    }
    let mut lines = vec!["returning".to_owned()];
    for (index, element) in elements.iter().enumerate() {
        let comma = if index + 1 < elements.len() { "," } else { "" };
        lines.push(format!("    {}{comma}", element.text));
    }
    lines
}

fn returning_elements(body: &str) -> Vec<Element> {
    splitter::split_elements("select", body)
        .into_iter()
        .map(|element| Element { text: alias_as(&element.text), connector: element.connector })
        .collect()
}

/// 补上 `returning` 的隐式别名；已有 `as` 的保持原样。
fn alias_as(text: &str) -> String {
    let tokens = lexer::tokens(text);
    let significant: Vec<&lexer::Token<'_>> = tokens.iter().filter(|token| !matches!(token.kind, Kind::Whitespace | Kind::Comment)).collect();
    if significant.len() < 2 {
        return collapse(text);
    }
    let last = significant[significant.len() - 1];
    if !matches!(last.kind, Kind::Word | Kind::QuotedIdent) {
        return collapse(text);
    }
    let previous = significant[significant.len() - 2];
    if previous.kind == Kind::Word && previous.text.eq_ignore_ascii_case("as") {
        return collapse(text);
    }
    let bare_pair = significant.len() == 2 && matches!(significant[0].kind, Kind::Word | Kind::QuotedIdent);
    let ends_expression = matches!(previous.kind, Kind::String | Kind::Number) || (previous.kind == Kind::Punct && previous.text == ")");
    if !(bare_pair || ends_expression) {
        return collapse(text);
    }
    if last.kind == Kind::Word && is_alias_stopword(last.text) {
        return collapse(text);
    }
    let prefix = collapse(&text[..last.start]);
    format!("{prefix} as {}", &text[last.start..last.start + last.text.len()])
}

fn is_alias_stopword(word: &str) -> bool {
    matches!(
        word.to_ascii_lowercase().as_str(),
        "null"
            | "true"
            | "false"
            | "end"
            | "when"
            | "then"
            | "else"
            | "and"
            | "or"
            | "not"
            | "is"
            | "in"
            | "between"
            | "like"
            | "ilike"
            | "asc"
            | "desc"
            | "distinct"
            | "all"
            | "any"
            | "some"
            | "case"
            | "exists"
    )
}

/// CTE 正文是否都简单；`as materialized` 等未支持的写法返回 `false`。
fn cte_bodies_simple(body: &str) -> bool {
    let tokens = lexer::tokens(body);
    let mut depth = 0i32;
    let mut bodies = 0usize;
    let mut index = 0usize;
    while index < tokens.len() {
        let token = &tokens[index];
        match token.kind {
            Kind::Punct if token.text == "(" => {
                depth += 1;
            }
            Kind::Punct if token.text == ")" => {
                depth -= 1;
            }
            Kind::Word if depth == 0 && token.text.eq_ignore_ascii_case("as") => {
                let open = (index + 1..tokens.len()).find(|probe| tokens[*probe].kind != Kind::Whitespace);
                let Some(open) = open else {
                    return false;
                };
                if tokens[open].kind != Kind::Punct || tokens[open].text != "(" {
                    return false;
                }
                let Some(close) = matching_close(&tokens, open) else {
                    return false;
                };
                let inner = &body[tokens[open].start + 1..tokens[close].start];
                if !decision::is_simple(inner) {
                    return false;
                }
                bodies += 1;
                index = close;
            }
            _ => {}
        }
        index += 1;
    }
    bodies > 0
}

fn top_words(text: &str) -> Vec<Word> {
    let mut depth = 0i32;
    let mut words = Vec::new();
    for token in lexer::tokens(text) {
        match token.kind {
            Kind::Punct if token.text == "(" => depth += 1,
            Kind::Punct if token.text == ")" => depth -= 1,
            Kind::Word if depth == 0 => words.push(Word { start: token.start, end: token.start + token.text.len(), text: token.text.to_ascii_lowercase() }),
            _ => {}
        }
    }
    words
}

fn matching_close(tokens: &[lexer::Token<'_>], open: usize) -> Option<usize> {
    let mut depth = 0i32;
    for (index, token) in tokens.iter().enumerate().skip(open) {
        if token.kind != Kind::Punct {
            continue;
        }
        match token.text {
            "(" => depth += 1,
            ")" => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

/// 深度 0 的 join 起点，连同 `left`/`full` 等修饰词。
fn join_starts(text: &str) -> Vec<usize> {
    let tokens = lexer::tokens(text);
    let mut depth = 0i32;
    let mut starts = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        match token.kind {
            Kind::Punct if token.text == "(" => {
                depth += 1;
                continue;
            }
            Kind::Punct if token.text == ")" => {
                depth -= 1;
                continue;
            }
            Kind::Word if depth == 0 && token.text.eq_ignore_ascii_case("join") => {
                let mut start = token.start;
                let mut probe = index;
                while probe > 0 {
                    probe -= 1;
                    let previous = &tokens[probe];
                    if previous.kind == Kind::Whitespace {
                        continue;
                    }
                    if previous.kind == Kind::Word && splitter::is_join_modifier(previous.text) {
                        start = previous.start;
                        continue;
                    }
                    break;
                }
                starts.push(start);
            }
            _ => {}
        }
    }
    starts
}

fn layout_explain(text: &str, statement: &Statement, dialect: Dialect) -> Option<String> {
    let index = lexer::LineIndex::new(text);
    let span = index.span(text, statement.span())?;
    let header = explain_header(&text[..span.start])?;
    let query_text = text[span.start..].trim();
    if query_text.is_empty() {
        return None;
    }
    let rendered = match statement {
        Statement::Query(query) => match query.body.as_ref() {
            SetExpr::Delete(Statement::Delete(delete)) => {
                let normalized = token::normalize(query_text);
                layout_delete(&normalized, delete).unwrap_or(normalized)
            }
            _ if starts_with_word(query_text, "select") || starts_with_word(query_text, "with") => {
                let normalized = token::normalize(query_text);
                match crate::sql::fix::format_with(&normalized, dialect) {
                    Ok(formatted) => formatted,
                    Err(_) => normalized,
                }
            }
            _ => token::normalize(query_text),
        },
        Statement::Delete(delete) => {
            let normalized = token::normalize(query_text);
            layout_delete(&normalized, delete).unwrap_or(normalized)
        }
        _ => token::normalize(query_text),
    };
    Some(format!("{header}\n{rendered}"))
}

/// `explain` 头一行：选项以 `, ` 连接，布尔值统一 `true`/`false`。
fn explain_header(header: &str) -> Option<String> {
    let collapsed = collapse(header);
    if !collapsed.starts_with("explain") {
        return None;
    }
    let collapsed = if let Some(rest) = collapsed.strip_prefix("explain(") {
        format!("explain ({rest}")
    } else {
        collapsed
    };
    let Some(open) = collapsed.find('(') else {
        return Some(collapsed);
    };
    if !collapsed.ends_with(')') {
        return Some(collapsed);
    }
    let inner = &collapsed[open + 1..collapsed.len() - 1];
    let options = splitter::split_elements("select", inner);
    if options.is_empty() {
        return Some(collapsed);
    }
    let rendered: Vec<String> = options.iter().map(|option| canonical_option(&option.text)).collect();
    Some(format!("{} ({})", collapsed[..open].trim_end(), rendered.join(", ")))
}

fn canonical_option(option: &str) -> String {
    let collapsed = collapse(option);
    let Some(index) = collapsed.find(|character: char| character.is_whitespace()) else {
        return collapsed;
    };
    let (name, rest) = collapsed.split_at(index);
    let value = rest.trim();
    if !BOOLEAN_OPTIONS.iter().any(|candidate| candidate.eq_ignore_ascii_case(name)) {
        return collapsed;
    }
    let normalized = match value.to_ascii_lowercase().as_str() {
        "on" | "true" | "1" => "true",
        "off" | "false" | "0" => "false",
        _ => return collapsed,
    };
    format!("{name} {normalized}")
}

/// 把任意空白折叠成一个空格；字符串、引号标识符逐字节保留。
fn collapse(text: &str) -> String {
    let mut output = String::new();
    let mut pending = false;
    for token in lexer::tokens(text) {
        match token.kind {
            Kind::Whitespace | Kind::Comment => {
                if !output.is_empty() {
                    pending = true;
                }
            }
            _ => {
                if pending {
                    output.push(' ');
                    pending = false;
                }
                output.push_str(token.text);
            }
        }
    }
    output
}

fn starts_with_word(text: &str, word: &str) -> bool {
    let Some(rest) = text.trim_start().strip_prefix(word) else {
        return false;
    };
    rest.chars().next().is_none_or(|character| !(character.is_alphanumeric() || character == '_' || character == '$'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(sql: &str) -> String {
        let output = layout(sql, Dialect::Postgres).expect("layout");
        assert_eq!(layout(&output, Dialect::Postgres), Some(output.clone()));
        output
    }

    #[test]
    fn keeps_single_line_delete() {
        assert_eq!(render("delete from films where kind != 'Musical'"), "delete from films where kind != 'Musical'");
    }

    #[test]
    fn breaks_non_simple_delete_but_keeps_where_inline() {
        assert_eq!(
            render("delete from films f using producers p where f.producer_id = p.id and p.name = 'foo'"),
            "delete from films f\nusing producers p\nwhere f.producer_id = p.id and p.name = 'foo'"
        );
    }

    #[test]
    fn breaks_four_where_conditions_with_anchor() {
        let sql = "delete from films f using producers p, actors a where 1 = 1 and f.producer_id = p.id and p.name = 'foo' and f.actor_id = a.id and a.name = 'joe cool'";
        assert_eq!(
            render(sql),
            "delete from films f\n\
             using producers p, actors a\n\
             where 1 = 1\n\
             \u{20}   and f.producer_id = p.id\n\
             \u{20}   and p.name = 'foo'\n\
             \u{20}   and f.actor_id = a.id\n\
             \u{20}   and a.name = 'joe cool'"
        );
    }

    #[test]
    fn or_conditions_use_zero_anchor() {
        let sql = "delete from t where a = 1 or b = 2 or c = 3 or d = 4";
        assert_eq!(render(sql), "delete from t\nwhere 1 = 0\n    and a = 1\n    or b = 2\n    or c = 3\n    or d = 4");
    }

    #[test]
    fn puts_using_join_on_its_own_line() {
        assert_eq!(
            render("delete from foo using baz left join bar on baz.bar_id = bar.id where foo.id = bar.foo_id and baz.id = 'c'"),
            "delete from foo\n\
             using baz\n\
             left join bar on baz.bar_id = bar.id\n\
             where foo.id = bar.foo_id and baz.id = 'c'"
        );
    }

    #[test]
    fn returning_alias_gets_as() {
        assert_eq!(
            render("delete from tasks where status = 'DONE' returning actor_id a_id"),
            "delete from tasks where status = 'DONE' returning actor_id as a_id"
        );
    }

    #[test]
    fn breaks_three_aliased_returning_items() {
        assert_eq!(
            render("delete from tasks where status = 'DONE' returning actor_id a_id, producer_id p_id, x y"),
            "delete from tasks\n\
             where status = 'DONE'\n\
             returning\n\
             \u{20}   actor_id as a_id,\n\
             \u{20}   producer_id as p_id,\n\
             \u{20}   x as y"
        );
    }

    #[test]
    fn inlines_simple_cte() {
        assert_eq!(render("with test as (select foo from bar) delete from films"), "with test as (select foo from bar) delete from films");
    }

    #[test]
    fn refuses_complex_cte() {
        let sql = "with recursive t(n) as (values (1) union all select n + 1 from t where n < 100) delete from films";
        assert_eq!(layout(sql, Dialect::Postgres), None);
    }

    #[test]
    fn formats_insert_select_query() {
        assert_eq!(
            render("insert into t (a, b, c, d) select a, b, c, d from s"),
            "insert into t (a, b, c, d)\n\
             select\n\
             \u{20}   a,\n\
             \u{20}   b,\n\
             \u{20}   c,\n\
             \u{20}   d\n\
             from s"
        );
    }

    #[test]
    fn splits_explain_header_and_formats_query() {
        assert_eq!(
            render("explain (analyze true, format json) select a, b, c, d from t"),
            "explain (analyze true, format json)\n\
             select\n\
             \u{20}   a,\n\
             \u{20}   b,\n\
             \u{20}   c,\n\
             \u{20}   d\n\
             from t"
        );
    }

    #[test]
    fn normalizes_boolean_option_values() {
        assert_eq!(
            render("explain (analyze on, verbose off, costs 1, generic_plan 0, format xml) select 1"),
            "explain (analyze true, verbose false, costs true, generic_plan false, format xml)\nselect 1"
        );
    }

    #[test]
    fn keeps_bare_options_and_enums() {
        assert_eq!(render("explain (analyze, serialize binary, memory) select 1"), "explain (analyze, serialize binary, memory)\nselect 1");
    }

    #[test]
    fn explains_delete_with_delete_layout() {
        assert_eq!(
            render("explain (analyze true) delete from t where a = 1 and b = 2"),
            "explain (analyze true)\ndelete from t\nwhere a = 1 and b = 2"
        );
    }

    #[test]
    fn explains_with_delete() {
        assert_eq!(
            render("explain with test as (select foo from bar) delete from films"),
            "explain\nwith test as (select foo from bar) delete from films"
        );
    }

    #[test]
    fn formats_with_query_after_explain() {
        assert_eq!(
            render("explain (analyze true) with x as (select a from t) select a from x"),
            "explain (analyze true)\n\
             with\n\
             x as (select a from t)\n\
             select a\n\
             from x"
        );
    }

    #[test]
    fn keeps_a_simple_update_on_one_line() {
        assert_eq!(render("update t set a = 1 where b = 2"), "update t set a = 1 where b = 2");
    }

    #[test]
    fn breaks_update_clauses_but_keeps_three_where_conditions_inline() {
        assert_eq!(
            render("update sys_permission set resource_path = '/x' where permission_code = 'p' and deleted = 0 and resource_path = '/y'"),
            "update sys_permission\n\
             set resource_path = '/x'\n\
             where permission_code = 'p' and deleted = 0 and resource_path = '/y'"
        );
    }

    #[test]
    fn breaks_four_update_assignments() {
        assert_eq!(
            render("update t set a = 1, b = 2, c = 3, d = 4 where id = 1"),
            "update t\n\
             set\n\
             \u{20}   a = 1,\n\
             \u{20}   b = 2,\n\
             \u{20}   c = 3,\n\
             \u{20}   d = 4\n\
             where id = 1"
        );
    }

    #[test]
    fn formats_with_update_and_cte() {
        assert_eq!(
            render("with ranked as (select id from t where status = 'RUNNING') update t set status = 'FAILED' where id in (select id from ranked)"),
            "with\n\
             ranked as (\n\
             \u{20}   select id\n\
             \u{20}   from t\n\
             \u{20}   where status = 'RUNNING'\n\
             )\n\
             update t\n\
             set status = 'FAILED'\n\
             where id in (select id from ranked)"
        );
    }
}
