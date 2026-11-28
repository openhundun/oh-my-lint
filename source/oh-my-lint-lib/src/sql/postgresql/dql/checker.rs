//! 断行检查：只产出违规与定位，不做任何文本修改。

use crate::sql::postgresql::dql::util::{block_indent, line_indent, starts_with_word, trailing_indent};
use crate::sql::postgresql::dql::r#where;

/// 期望的版式。
pub(crate) use crate::sql::postgresql::dql::Mode;

/// 一条断行违规；`span` 是待替换 / 待检查的正文区间。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Violation {
    pub kind: &'static str,
    pub element_count: usize,
    pub expected: Mode,
    pub span: crate::span::Span,
    pub indent: usize,
    pub next_indent: Option<usize>,
    pub message: String,
    pub(crate) condition: Option<r#where::ConditionLayout>,
}

const BREAK_KINDS: [&str; 6] = ["select", "from", "where", "having", "group by", "order by"];

/// 按指定方言检查；结构优先来自 AST，解析失败时回退到文本扫描。
pub fn violations_with(sql: &str, dialect: crate::sql::parse::Dialect) -> Vec<Violation> {
    crate::sql::lexer::with_dialect(dialect, || violations_inner(sql, dialect))
}

fn violations_inner(sql: &str, dialect: crate::sql::parse::Dialect) -> Vec<Violation> {
    let structure = crate::sql::astmap::structure(sql, dialect);
    let mut found = Vec::new();
    for statement in crate::sql::splitter::statements(sql) {
        let statement_text = &sql[statement.start..statement.end];
        let blocks = structure
            .as_ref()
            .and_then(|structure| structure.blocks(statement, statement_text))
            .unwrap_or_else(|| crate::sql::splitter::all_statements(statement_text));
        for (index, block_span) in blocks.into_iter().enumerate() {
            let block = &statement_text[block_span.start..block_span.end];
            let is_root = block_span.start == 0 && block_span.end == statement_text.len();
            let is_query = !is_root && super::subquery::is_query_block(block);
            if is_query && !crate::sql::decision::is_simple(block) {
                if !crate::sql::splitter::has_top_level_newline(block) {
                    let count = crate::sql::splitter::clause_spans(block).len();
                    found.push(Violation {
                        kind: "subquery",
                        element_count: count,
                        expected: Mode::Break,
                        span: crate::span::Span::new(statement.start + block_span.start, statement.start + block_span.end),
                        indent: line_indent(statement_text, block_span.start),
                        next_indent: None,
                        message: format!("subquery n={count} expected=break"),
                        condition: None,
                    });
                    continue;
                }
                if let Some((open, close)) = super::subquery::query_parens(statement_text, block_span) {
                    let indent = line_indent(statement_text, open);
                    let actual = &statement_text[open + 1..close];
                    let padding = " ".repeat(indent + 4);
                    let closing = format!("\n{}", " ".repeat(indent));
                    let starts = actual.starts_with(&format!("\n{padding}"));
                    let ends = actual.ends_with(&closing);
                    if !starts || !ends {
                        found.push(Violation {
                            kind: "query-layout",
                            element_count: 0,
                            expected: Mode::Break,
                            span: crate::span::Span::new(statement.start + open + 1, statement.start + close),
                            indent,
                            next_indent: None,
                            message: "subquery body should align with its parentheses".to_owned(),
                            condition: None,
                        });
                        continue;
                    }
                }
            } else if is_query && crate::sql::splitter::has_top_level_newline(block) {
                let span = super::subquery::query_parens(statement_text, block_span)
                    .map_or(crate::span::Span::new(statement.start + block_span.start, statement.start + block_span.end), |(open, close)| {
                        crate::span::Span::new(statement.start + open + 1, statement.start + close)
                    });
                found.push(Violation {
                    kind: "subquery",
                    element_count: 0,
                    expected: Mode::Inline,
                    span,
                    indent: line_indent(statement_text, block_span.start),
                    next_indent: None,
                    message: "simple subquery should be inline".to_owned(),
                    condition: None,
                });
            }
            let _ = index;
            check_block(statement.start, statement_text, block_span, &mut found);
        }
    }
    found
}

/// `from` 正文是否包含 `(values ...)` 派生表；行构造器不参与条件组/函数实参检查。
fn contains_values_derived(text: &str) -> bool {
    let tokens = crate::sql::lexer::tokens(text);
    for (index, token) in tokens.iter().enumerate() {
        if token.kind != crate::sql::lexer::Kind::Punct || token.text != "(" {
            continue;
        }
        let next = tokens[index + 1..].iter().find(|next| next.kind != crate::sql::lexer::Kind::Whitespace);
        if next.is_some_and(|next| next.kind == crate::sql::lexer::Kind::Word && next.text.eq_ignore_ascii_case("values")) {
            return true;
        }
    }
    false
}

fn check_block(statement_start: usize, statement_text: &str, block: crate::span::Span, found: &mut Vec<Violation>) {
    let text = &statement_text[block.start..block.end];
    if starts_with_word(text, "with") {
        super::with::check(statement_start, statement_text, block, found);
    }
    let clauses = crate::sql::splitter::clause_spans(text);
    if clauses.iter().any(|clause| crate::sql::splitter::SET_OPERATORS.contains(&clause.kind)) {
        super::set_op::check(statement_start, statement_text, block, &clauses, found);
        return;
    }
    check_clauses(statement_start, statement_text, block.start, block.end, found);
}

/// 子句级检查：断行期望、元素内的布尔组与 `case`、`from` 的 join 版式。
pub(super) fn check_clauses(statement_start: usize, statement_text: &str, base: usize, end: usize, found: &mut Vec<Violation>) {
    let text = &statement_text[base..end];
    let clauses = crate::sql::splitter::clause_spans(text);
    if clauses.first().map(|clause| clause.kind) != Some("select") {
        return;
    }
    let block_indent = block_indent(statement_text, base);
    let misplaced: Vec<&crate::sql::splitter::Clause> = clauses
        .iter()
        .skip(1)
        .filter(|clause| {
            let relative = base + clause.keyword_start;
            let line_start = statement_text[..relative].rfind('\n').map_or(0, |index| index + 1);
            !statement_text[line_start..relative].chars().all(|character| character == ' ' || character == '\t')
        })
        .collect();
    if !misplaced.is_empty() && !crate::sql::decision::is_simple(text) {
        for clause in misplaced {
            let relative = base + clause.keyword_start;
            let mut start = relative;
            let bytes = statement_text.as_bytes();
            while start > 0 && matches!(bytes[start - 1], b' ' | b'\t') {
                start -= 1;
            }
            found.push(Violation {
                kind: "clause-start",
                element_count: 0,
                expected: Mode::Break,
                span: crate::span::Span::new(statement_start + start, statement_start + relative),
                indent: block_indent,
                next_indent: None,
                message: format!("{} should start a new line", clause.kind),
                condition: None,
            });
        }
    }
    for clause in &clauses {
        if !BREAK_KINDS.contains(&clause.kind) {
            continue;
        }
        let body = &text[clause.body_start..clause.body_end];
        let has_join;
        if matches!(clause.kind, "where" | "having") {
            let condition = r#where::parse(body);
            if condition.elements.is_empty() {
                continue;
            }
            has_join = false;
            let keyword_start = base + clause.keyword_start;
            let keyword_span = crate::span::Span::new(statement_start + keyword_start, statement_start + base + clause.body_start);
            let indent = line_indent(statement_text, keyword_start);
            found.extend(r#where::ConditionLayout::plan(clause.kind, body, condition, keyword_span, indent, None));
        } else {
            let elements = crate::sql::splitter::split_elements(clause.kind, body);
            if elements.is_empty() {
                continue;
            }
            has_join = clause.kind == "from" && crate::sql::decision::has_join(body);
            if !has_join {
                let expected = if crate::sql::decision::needs_break(clause.kind, &elements) { Mode::Break } else { Mode::Inline };
                let actual = if expected == Mode::Break {
                    body.starts_with('\n')
                } else {
                    crate::sql::splitter::has_top_level_newline(body.trim_end())
                };
                if (expected == Mode::Break) != actual {
                    found.push(Violation {
                        kind: clause.kind,
                        element_count: elements.len(),
                        expected,
                        span: crate::span::Span::new(statement_start + base + clause.body_start, statement_start + base + clause.body_end),
                        indent: line_indent(statement_text, base + clause.keyword_start),
                        next_indent: trailing_indent(body),
                        message: format!("{} n={} expected={}", clause.kind, elements.len(), expected.as_str()),
                        condition: None,
                    });
                }
            }
        }
        let values_derived = clause.kind == "from" && !has_join && contains_values_derived(body);
        if !values_derived {
            super::group::check(statement_start, statement_text, base + clause.body_start, body, found);
            super::case::check(statement_start, statement_text, base + clause.body_start, body, found);
            super::function::check(statement_start, statement_text, base + clause.body_start, body, found);
        }
        if clause.kind == "from" {
            let from_indent = line_indent(statement_text, base + clause.keyword_start);
            super::join::check(statement_start, statement_text, base, clause, body, found);
            super::on::check(statement_start, base + clause.body_start, from_indent, body, found);
        }
        if clause.kind == "group by" {
            super::rollup::check(statement_start, statement_text, base + clause.body_start, body, found);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::sql::checker::Violation;
    use crate::sql::parse::Dialect;

    fn violations(sql: &str) -> Vec<Violation> {
        crate::sql::checker::violations_with(sql, Dialect::Postgres)
    }

    #[test]
    fn flags_four_field_select() {
        let found = violations("select a, b, c, d from t");
        assert!(found.iter().any(|violation| { violation.kind == "select" && violation.expected == crate::sql::checker::Mode::Break }));
    }

    #[test]
    fn allows_three_conditions_inline_body() {
        assert!(violations("select a\nfrom t\nwhere a = 1 and b = 2 and c = 3").iter().all(|violation| violation.kind != "where"));
    }

    #[test]
    fn flags_inline_subquery() {
        let found = violations("select a from t where x in (select y from u group by y having count(*) > 1 and max(z) > 0 and min(z) < 9 and sum(z) > 1)");
        assert!(found.iter().any(|violation| violation.kind == "subquery"));
    }

    #[test]
    fn handles_later_statement_after_utf8_comment() {
        let sql = "-- 用户同步修复\nbegin;\nselect a, b, c, d from t;\ncommit;";
        let found = violations(sql);
        assert!(found.iter().any(|violation| { violation.kind == "select" && violation.expected == crate::sql::checker::Mode::Break }));
    }

    #[test]
    fn flags_breaking_group_inline() {
        let found = violations("select a\nfrom t\nwhere 1 = 1\n    and (x = 1 or y = 2 or z = 3 or w = 4)");
        assert!(found.iter().any(|violation| violation.kind == "group"));
    }

    #[test]
    fn allows_inline_simple_group() {
        let found = violations("select a\nfrom t\nwhere 1 = 1\n    and (x = 1 or y = 2 or z = 3)\n    and b = 2\n    and c = 3");
        assert!(found.iter().all(|violation| violation.kind != "group"));
    }
}
