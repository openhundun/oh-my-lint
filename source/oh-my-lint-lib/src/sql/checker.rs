//! 断行检查：只产出违规与定位，不做任何文本修改。

/// 期望的版式。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    Break,
    Inline,
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Break => "break",
            Self::Inline => "inline",
        }
    }
}

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
}

const BREAK_KINDS: [&str; 6] = ["select", "from", "where", "having", "group by", "order by"];

/// 检查整段 SQL；根语句与任意深度子查询逐子句对照。
pub fn violations(sql: &str) -> Vec<Violation> {
    let mut found = Vec::new();
    for statement in crate::sql::splitter::statements(sql) {
        let statement_text = &sql[statement.start..statement.end];
        for block_span in crate::sql::splitter::all_statements(statement_text) {
            let block = &statement_text[block_span.start..block_span.end];
            let is_root = block_span.start == 0 && block_span.end == statement_text.len();
            if !is_root
                && !crate::sql::splitter::has_top_level_newline(block)
                && !crate::sql::decision::is_simple(block)
            {
                // 内联子查询整体交给 subquery 诊断；不要只改它内部的一个子句。
                continue;
            }
            let clauses = crate::sql::splitter::clause_spans(block);
            if clauses.first().map(|clause| clause.kind) != Some("select") {
                continue;
            }
            for clause in &clauses {
                if !BREAK_KINDS.contains(&clause.kind) {
                    continue;
                }
                let body = &block[clause.body_start..clause.body_end];
                let elements = crate::sql::splitter::split_elements(clause.kind, body);
                if elements.is_empty() {
                    continue;
                }
                let expected = if crate::sql::decision::needs_break(clause.kind, &elements) {
                    Mode::Break
                } else {
                    Mode::Inline
                };
                let actual = crate::sql::splitter::has_top_level_newline(body.trim());
                if (expected == Mode::Break) == actual {
                    continue;
                }
                let start = statement.start + block_span.start + clause.body_start;
                let end = statement.start + block_span.start + clause.body_end;
                found.push(Violation {
                    kind: clause.kind,
                    element_count: elements.len(),
                    expected,
                    span: crate::span::Span::new(start, end),
                    indent: line_indent(statement_text, block_span.start + clause.keyword_start),
                    next_indent: trailing_indent(body),
                    message: format!(
                        "{} n={} expected={}",
                        clause.kind,
                        elements.len(),
                        expected.as_str()
                    ),
                });
            }

            let has_set_operator = clauses
                .iter()
                .any(|clause| crate::sql::splitter::SET_OPERATORS.contains(&clause.kind));
            if !crate::sql::decision::is_simple(block) && !has_set_operator {
                let indent = block_indent(statement_text, statement.start + block_span.start);
                for clause in clauses.iter().skip(1) {
                    let absolute = statement.start + block_span.start + clause.keyword_start;
                    let relative = absolute - statement.start;
                    let before = &statement_text[..relative];
                    let line_start = before.rfind('\n').map_or(0, |index| index + 1);
                    let prefix = &before[line_start..];
                    if prefix
                        .chars()
                        .all(|character| character == ' ' || character == '\t')
                    {
                        continue;
                    }
                    let mut start = relative;
                    let bytes = statement_text.as_bytes();
                    while start > 0 && matches!(bytes[start - 1], b' ' | b'\t') {
                        start -= 1;
                    }
                    found.push(Violation {
                        kind: "clause-start",
                        element_count: 0,
                        expected: Mode::Break,
                        span: crate::span::Span::new(statement.start + start, absolute),
                        indent,
                        next_indent: None,
                        message: format!("{} should start a new line", clause.kind),
                    });
                }
            }

            for inner in crate::sql::splitter::all_statements(block)
                .into_iter()
                .skip(1)
            {
                let inner_text = &block[inner.start..inner.end];
                if inner_text.trim().is_empty()
                    || crate::sql::splitter::has_top_level_newline(inner_text.trim())
                    || crate::sql::decision::is_simple(inner_text)
                {
                    continue;
                }
                let start = statement.start + block_span.start + inner.start;
                let end = statement.start + block_span.start + inner.end;
                let count = crate::sql::splitter::clause_spans(inner_text).len();
                found.push(Violation {
                    kind: "subquery",
                    element_count: count,
                    expected: Mode::Break,
                    span: crate::span::Span::new(start, end),
                    indent: 0,
                    next_indent: None,
                    message: format!("subquery n={count} expected=break"),
                });
            }
        }
    }
    found
}

fn line_indent(text: &str, offset: usize) -> usize {
    let line_start = text[..offset].rfind('\n').map_or(0, |index| index + 1);
    text[line_start..offset]
        .chars()
        .take_while(|character| *character == ' ')
        .count()
}

fn block_indent(text: &str, block_start: usize) -> usize {
    let line_start = text[..block_start].rfind('\n').map_or(0, |index| index + 1);
    let prefix = &text[line_start..block_start];
    let indent = prefix
        .chars()
        .take_while(|character| *character == ' ')
        .count();
    if prefix
        .chars()
        .all(|character| character == ' ' || character == '\t')
    {
        indent
    } else if prefix.trim_end().ends_with('(') {
        indent + 4
    } else {
        indent
    }
}

fn trailing_indent(body: &str) -> Option<usize> {
    let newline = body.rfind('\n')?;
    let tail = &body[newline + 1..];
    if tail
        .chars()
        .all(|character| character == ' ' || character == '\t')
    {
        Some(tail.chars().count())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn flags_four_field_select() {
        let found = crate::sql::checker::violations("select a, b, c, d from t");
        assert!(found.iter().any(|violation| {
            violation.kind == "select" && violation.expected == crate::sql::checker::Mode::Break
        }));
    }

    #[test]
    fn allows_three_conditions() {
        assert!(
            crate::sql::checker::violations("select a from t where a = 1 and b = 2 and c = 3")
                .is_empty()
        );
    }

    #[test]
    fn flags_inline_subquery() {
        let found = crate::sql::checker::violations(
            "select a from t where x in (select y from u group by y having count(*) > 1 and max(z) > 0 and min(z) < 9 and sum(z) > 1)",
        );
        assert!(found.iter().any(|violation| violation.kind == "subquery"));
    }
}
