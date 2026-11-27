//! `DELETE`：`with`、`delete from`、`using`、`where`、`returning` 的子句版式。

use sqlparser::ast::{Delete, FromTable};

use crate::sql::lexer::{self, Kind};
use crate::sql::postgresql::dml::{collapse, format_cte_prefix, returning_lines, table_lines, top_words, where_lines};
use crate::sql::{decision, splitter};

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

pub(super) fn layout_delete(text: &str, delete: &Delete) -> Option<String> {
    if !delete_supported(delete) {
        return None;
    }
    let parts = delete_parts(text)?;
    let simple = delete_simple(text, &parts);
    if let Some(with) = parts.iter().find(|part| part.kind == DeletePart::With)
        && (!cte_bodies_simple(with.body(text)) || !simple)
    {
        let words = top_words(text);
        let delete_word = words.iter().find(|word| word.text == "delete")?;
        let cte = format_cte_prefix(&text[..delete_word.start], crate::sql::lexer::current_dialect())?;
        let main = &text[delete_word.start..];
        let formatted = layout_delete(main, delete)?;
        return Some(format!("{cte}\n{formatted}"));
    }
    let collected: Vec<Vec<String>> = parts.iter().map(|part| render_delete_part(text, *part)).collect();
    if simple {
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
                if crate::sql::postgresql::dql::r#where::parse(body).elements.len() > 1 {
                    return false;
                }
            }
            DeletePart::Returning => {
                let elements = super::returning_elements(body);
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
