//! 布尔组：组内超过 3 个条件或含复杂条件时逐条件断行。

use crate::sql::postgresql::dql::checker::Violation;
use crate::sql::postgresql::dql::clause::Mode;
use crate::sql::postgresql::dql::util::line_indent;

/// 布尔组：>3 个条件或含复杂条件则逐条件断行。
pub(super) fn check(statement_start: usize, statement_text: &str, body_base: usize, body: &str, found: &mut Vec<Violation>) {
    let nested: Vec<crate::span::Span> = crate::sql::splitter::all_statements(body).into_iter().skip(1).collect();
    for group in crate::sql::splitter::condition_group_spans(body) {
        if nested.iter().any(|block| block.start <= group.start && group.end <= block.end) {
            continue;
        }
        let inner = &body[group.start..group.end];
        let conditions = crate::sql::splitter::split_elements("where", inner);
        if conditions.is_empty() {
            continue;
        }
        let expected = if conditions.len() > 3 || conditions.iter().any(|condition| crate::sql::decision::is_complex(&condition.text, "where")) {
            Mode::Break
        } else {
            Mode::Inline
        };
        let actual = crate::sql::splitter::has_top_level_newline(inner);
        if (expected == Mode::Break) == actual {
            continue;
        }
        found.push(Violation {
            kind: "group",
            element_count: conditions.len(),
            expected,
            span: crate::span::Span::new(statement_start + body_base + group.start, statement_start + body_base + group.end),
            indent: line_indent(statement_text, body_base + group.start.saturating_sub(1)),
            next_indent: None,
            message: format!("group n={} expected={}", conditions.len(), expected.as_str()),
            condition: None,
        });
    }
}

/// 布尔组：断行时补锚点与连接词，内联时丢锚点。
pub(super) fn render(text: &str, violation: &Violation) -> Option<String> {
    let inner = &text[violation.span.start..violation.span.end];
    let elements = crate::sql::splitter::split_elements("where", inner);
    if elements.is_empty() {
        return None;
    }
    if elements.len() == 1 {
        let body = elements[0].text.trim();
        return Some(match violation.expected {
            Mode::Break => {
                let padding = " ".repeat(violation.indent + 4);
                let mut lines = body.lines();
                let first = lines.next().unwrap_or("").trim_end();
                let mut rendered = format!("\n{padding}{first}");
                for line in lines {
                    rendered.push('\n');
                    if !line.trim().is_empty() {
                        rendered.push_str("    ");
                        rendered.push_str(line.trim_end());
                    }
                }
                rendered.push('\n');
                rendered.push_str(&" ".repeat(violation.indent));
                rendered
            }
            Mode::Inline => body.to_owned(),
        });
    }
    let connective = elements
        .iter()
        .skip(1)
        .map(|element| element.connector)
        .next()
        .or_else(|| elements.first().map(|element| element.connector))
        .unwrap_or(crate::sql::splitter::Connector::And);
    match violation.expected {
        Mode::Break => {
            let pad = " ".repeat(violation.indent);
            let padding = " ".repeat(violation.indent + 4);
            let anchor = if connective == crate::sql::splitter::Connector::Or { "1 = 0" } else { "1 = 1" };
            let mut lines = vec![anchor.to_owned()];
            for (index, element) in elements.iter().enumerate() {
                let connector = if index == 0 { connective } else { element.connector };
                lines.push(format!("{} {}", crate::sql::decision::connector_word(connector), element.text));
            }
            let content = lines.iter().map(|line| format!("{padding}{line}")).collect::<Vec<_>>().join("\n");
            Some(format!("\n{content}\n{pad}"))
        }
        Mode::Inline => {
            let mut joined = String::new();
            for (index, element) in elements.iter().enumerate() {
                if index == 0 {
                    joined.push_str(&element.text);
                    continue;
                }
                joined.push(' ');
                joined.push_str(crate::sql::decision::connector_word(element.connector));
                joined.push(' ');
                joined.push_str(&element.text);
            }
            Some(joined)
        }
    }
}
