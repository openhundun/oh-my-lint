//! `join ... on` 条件：与 `where` / `having` 共用条件块规则。

use crate::sql::postgresql::dql::checker::Violation;
use crate::sql::postgresql::dql::clause::Mode;
use crate::sql::postgresql::dql::join;
use crate::sql::postgresql::dql::r#where;

/// 一个深度 0 的 `on` 条件正文区间。
#[derive(Clone, Copy, Debug)]
struct OnClause {
    body_start: usize,
    body_end: usize,
}

/// 按 join 分段，找出每段第一个深度 0 的 `on`。
fn clauses(body: &str) -> Vec<OnClause> {
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
                found.push(OnClause { body_start: token.start + token.text.len(), body_end });
            }
            _ => {}
        }
    }
    found
}

pub(super) fn check(statement_start: usize, body_base: usize, from_indent: usize, body: &str, found: &mut Vec<Violation>) {
    for clause in clauses(body) {
        let raw = &body[clause.body_start..clause.body_end];
        let condition = r#where::parse(raw);
        if condition.elements.is_empty() {
            continue;
        }
        let expected = r#where::mode(&condition.elements);
        let actual = match expected {
            Mode::Inline => !condition.wrapped && !crate::sql::splitter::has_top_level_newline(raw.trim_end()),
            Mode::Break => condition.wrapped && crate::sql::splitter::has_top_level_newline(condition.inner) && r#where::block_layout_ok(raw, from_indent),
        };
        if actual {
            continue;
        }
        let message = match (expected, condition.wrapped) {
            (Mode::Break, false) => "on should be wrapped",
            (Mode::Break, true) => "on condition block is not aligned",
            (Mode::Inline, true) => "on should not be wrapped",
            (Mode::Inline, false) => "on should be inline",
        };
        found.push(Violation {
            kind: "on",
            element_count: condition.elements.len(),
            expected,
            span: crate::span::Span::new(statement_start + body_base + clause.body_start, statement_start + body_base + clause.body_end),
            indent: from_indent,
            next_indent: Some(from_indent),
            message: message.to_owned(),
        });
    }
}
