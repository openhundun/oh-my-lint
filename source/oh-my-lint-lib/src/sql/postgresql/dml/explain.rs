//! `EXPLAIN`：头部选项一行，被执行语句按 DQL/DML 渲染。

use sqlparser::ast::{SetExpr, Spanned as _, Statement};

use crate::sql::lexer;
use crate::sql::parse::Dialect;
use crate::sql::postgresql::dml::collapse;
use crate::sql::postgresql::dml::delete::layout_delete;
use crate::sql::token;

/// `EXPLAIN` 里只接受布尔值的选项；其余选项原样保留。
const BOOLEAN_OPTIONS: [&str; 11] = ["analyze", "analyse", "verbose", "costs", "settings", "buffers", "wal", "timing", "summary", "generic_plan", "memory"];

pub(super) fn layout_explain(text: &str, statement: &Statement, dialect: Dialect) -> Option<String> {
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
    let options = crate::sql::splitter::split_elements("select", inner);
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

fn starts_with_word(text: &str, word: &str) -> bool {
    let Some(rest) = text.trim_start().strip_prefix(word) else {
        return false;
    };
    rest.chars().next().is_none_or(|character| !(character.is_alphanumeric() || character == '_' || character == '$'))
}
