//! `join ... on` 条件：与 `where` / `having` 共用条件块规则。

use crate::span::Span;
use crate::sql::postgresql::dql::checker::Violation;
use crate::sql::postgresql::dql::join;
use crate::sql::postgresql::dql::r#where;

/// 一个深度 0 的 `on` 关键字及正文区间。
#[derive(Clone, Copy, Debug)]
pub(crate) struct OnClause {
    pub(crate) keyword_start: usize,
    pub(crate) body_start: usize,
    pub(crate) body_end: usize,
}

/// 按 join 分段，找出每段第一个深度 0 的 `on`。
pub(crate) fn clauses(body: &str) -> Vec<OnClause> {
    let starts = join::join_starts(body);
    if starts.is_empty() {
        return Vec::new();
    }
    let tokens = crate::sql::lexer::tokens(body);
    let mut boundaries = starts.clone();
    let mut depth = 0i32;
    for token in &tokens {
        match token.kind {
            crate::sql::lexer::Kind::Punct if token.text == "(" => depth += 1,
            crate::sql::lexer::Kind::Punct if token.text == ")" => depth -= 1,
            crate::sql::lexer::Kind::Punct if token.text == "," && depth == 0 => boundaries.push(token.start),
            _ => {}
        }
    }
    boundaries.sort_unstable();
    let mut depth = 0i32;
    let mut seen = vec![false; starts.len()];
    let mut found = Vec::new();
    for token in &tokens {
        match token.kind {
            crate::sql::lexer::Kind::Punct if token.text == "(" => depth += 1,
            crate::sql::lexer::Kind::Punct if token.text == ")" => depth -= 1,
            crate::sql::lexer::Kind::Word if depth == 0 && token.text.eq_ignore_ascii_case("on") => {
                let Some(index) = starts.iter().rposition(|start| *start < token.start) else {
                    continue;
                };
                if seen[index] {
                    continue;
                }
                seen[index] = true;
                let body_end = boundaries.iter().copied().find(|boundary| *boundary > token.start).unwrap_or(body.len());
                found.push(OnClause { keyword_start: token.start, body_start: token.start + token.text.len(), body_end });
            }
            _ => {}
        }
    }
    found
}

pub(super) fn check(statement_start: usize, body_base: usize, from_indent: usize, body: &str, found: &mut Vec<Violation>) {
    let absolute_base = statement_start + body_base;
    for clause in clauses(body) {
        let raw = &body[clause.body_start..clause.body_end];
        let condition = r#where::parse(raw);
        if condition.elements.is_empty() {
            continue;
        }
        let keyword_span = Span::new(absolute_base + clause.keyword_start, absolute_base + clause.body_start);
        found.extend(r#where::ConditionLayout::plan("on", raw, condition, keyword_span, from_indent, Some(from_indent)));
    }
}
