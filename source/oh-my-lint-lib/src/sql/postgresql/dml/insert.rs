//! `INSERT ... SELECT` 与 `INSERT ... VALUES` 的 DML 骨架；values 由 DQL values 模块收尾。

use crate::sql::lexer::{self, Kind};
use crate::sql::parse::{self, Dialect};
use crate::sql::postgresql::dml::{collapse, format_cte_prefix, top_words};
use crate::sql::token;

/// `INSERT ... SELECT`：保留 insert 头，把查询体交给 DQL 版式。
pub(super) fn layout_insert(text: &str, dialect: Dialect) -> Option<String> {
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
    let query_words = top_words(query);
    let conflict = query_words
        .iter()
        .enumerate()
        .find(|(index, word)| word.text == "on" && query_words.get(index + 1).is_some_and(|next| next.text == "conflict"))
        .map(|(index, _)| index);
    let (query, suffix) = conflict.map_or((query, ""), |index| {
        let start = query_words[index].start;
        (&query[..start], &query[start..])
    });
    let flat = flatten_query(query)?;
    let formatted = crate::sql::fix::format_with(&flat, dialect).ok()?;
    let body = if suffix.is_empty() {
        format!("{header}\n{formatted}")
    } else {
        format!("{header}\n{formatted}\n{}", suffix.trim())
    };
    let output = if insert.start > 0 {
        let cte = format_cte_prefix(&text[..insert.start], dialect)?;
        format!("{cte}\n{body}")
    } else {
        body
    };
    parse::parse(&output, dialect).ok()?;
    Some(output)
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
