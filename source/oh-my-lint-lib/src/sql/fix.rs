//! 修复：只重写违规子句的正文，块内其余字节原样保留。

/// 把一段 SQL 修成房规版式；合规输入原样返回（语料 no-op 由构造保证）。
pub fn format(sql: &str) -> crate::error::Result<String> {
    if sql.contains("--") || sql.contains("/*") {
        return Err(crate::error::Error::Rejected(
            "comments are not supported in fix mode".to_owned(),
        ));
    }
    let initial = crate::sql::checker::violations(sql);
    if initial.iter().any(|violation| violation.kind == "subquery") {
        return Err(crate::error::Error::Rejected(
            "inline subquery expansion is not supported yet".to_owned(),
        ));
    }
    let mut current = sql.to_owned();
    for _ in 0..256 {
        let found = crate::sql::checker::violations(&current);
        let Some(violation) = found
            .iter()
            .filter(|violation| violation.kind != "subquery")
            .min_by_key(|violation| violation.span.len())
        else {
            break;
        };
        let replacement = render(&current, violation);
        if replacement == current[violation.span.start..violation.span.end] {
            break;
        }
        current.replace_range(violation.span.start..violation.span.end, &replacement);
    }
    Ok(current)
}

fn render(text: &str, violation: &crate::sql::checker::Violation) -> String {
    if violation.kind == "clause-start" {
        return format!("\n{}", " ".repeat(violation.indent));
    }
    let body = &text[violation.span.start..violation.span.end];
    let elements = crate::sql::splitter::split_elements(violation.kind, body);
    let has_next = violation.span.end < text.len();
    let base = violation.indent + 4;
    match violation.expected {
        crate::sql::checker::Mode::Break => {
            let mut lines: Vec<String> = Vec::new();
            if matches!(violation.kind, "where" | "having") {
                lines.push("1 = 1".to_owned());
                for element in &elements {
                    lines.push(format!(
                        "{} {}",
                        crate::sql::decision::connector_word(element.connector),
                        element.text
                    ));
                }
            } else {
                for (index, element) in elements.iter().enumerate() {
                    let comma = if index + 1 < elements.len() { "," } else { "" };
                    lines.push(format!("{}{comma}", element.text));
                }
            }
            let padding = " ".repeat(base);
            let content = lines
                .iter()
                .map(|line| format!("{padding}{line}"))
                .collect::<Vec<_>>()
                .join("\n");
            let suffix = if has_next {
                let indent = violation.next_indent.unwrap_or(violation.indent);
                format!("\n{}", " ".repeat(indent))
            } else {
                String::new()
            };
            format!("\n{content}{suffix}")
        }
        crate::sql::checker::Mode::Inline => {
            let mut joined = String::new();
            for (index, element) in elements.iter().enumerate() {
                if index == 0 {
                    joined.push_str(&element.text);
                    continue;
                }
                if matches!(violation.kind, "where" | "having") {
                    joined.push(' ');
                    joined.push_str(crate::sql::decision::connector_word(element.connector));
                    joined.push(' ');
                } else {
                    joined.push_str(", ");
                }
                joined.push_str(&element.text);
            }
            let suffix = if has_next { " " } else { "" };
            format!(" {joined}{suffix}")
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn breaks_four_fields_and_is_idempotent() {
        let sql = "select a, b, c, d from t";
        let fixed = crate::sql::fix::format(sql).expect("format");
        assert!(fixed.contains("select\n    a,"));
        assert!(crate::sql::check(&fixed).is_empty(), "{fixed}");
        assert_eq!(crate::sql::fix::format(&fixed).expect("second"), fixed);
    }

    #[test]
    fn inlines_three_conditions() {
        let sql = "select a from t where\n    1 = 1\n    and a = 1\n    and b = 2\n    and c = 3\n";
        let fixed = crate::sql::fix::format(sql).expect("format");
        assert_eq!(
            fixed.trim(),
            "select a from t where a = 1 and b = 2 and c = 3"
        );
        assert!(crate::sql::check(&fixed).is_empty(), "{fixed}");
    }

    #[test]
    fn leaves_compliant_sql_untouched() {
        let sql = "select a, b\nfrom t\nwhere 1 = 1\n    and a = 1\n    and b = 2\n    and c = 3\n    and d = 4";
        assert_eq!(crate::sql::fix::format(sql).expect("format"), sql);
    }

    #[test]
    fn rejects_comments() {
        assert!(crate::sql::fix::format("select a -- x\nfrom t").is_err());
    }
}
