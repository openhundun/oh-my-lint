//! `UPDATE`：`set` 按 select 元素规则，其他子句超过 1 个元素即整句分行。

use crate::sql::parse::{self, Dialect};
use crate::sql::postgresql::dml::{collapse, format_cte_prefix, returning_lines, table_lines, top_words, where_lines};
use crate::sql::{decision, splitter};

/// `UPDATE`：`set` 按 select 元素规则，其他子句超过 1 个元素即整句分行。
pub(super) fn layout_update(text: &str, dialect: Dialect) -> Option<String> {
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
    let where_inline = where_body.is_none_or(|body| crate::sql::postgresql::dql::r#where::parse(body).elements.len() <= 1);
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
