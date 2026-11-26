//! 修复：只重写违规区间，块内其余字节原样保留。

/// 目前能正确渲染的违规类型。
const SUPPORTED_FIXES: [&str; 19] = [
    "select",
    "from",
    "where",
    "having",
    "group by",
    "order by",
    "clause-start",
    "subquery",
    "group",
    "case",
    "cross-join",
    "from-join",
    "set-op",
    "set-op-branch",
    "set-op-parens",
    "rollup",
    "with",
    "with-close",
    "with-query",
];

/// 把一段 SQL 修成房规版式；测试用 Postgres 快捷入口。
#[cfg(test)]
pub fn format(sql: &str) -> crate::error::Result<String> {
    format_with(sql, crate::sql::parse::Dialect::Postgres)
}

/// 按指定方言修复；每轮重算结构。
pub fn format_with(sql: &str, dialect: crate::sql::parse::Dialect) -> crate::error::Result<String> {
    crate::sql::lexer::with_dialect(dialect, || format_with_inner(sql, dialect))
}

fn format_with_inner(sql: &str, dialect: crate::sql::parse::Dialect) -> crate::error::Result<String> {
    if crate::sql::lexer::has_comment(sql) {
        return Err(crate::error::Error::Rejected("comments are not supported in fix mode".to_owned()));
    }
    let initial = crate::sql::checker::violations_with(sql, dialect);
    if let Some(violation) = initial.iter().find(|violation| !SUPPORTED_FIXES.contains(&violation.kind)) {
        return Err(crate::error::Error::Rejected(format!("{} layout is not supported yet", violation.kind)));
    }
    let mut current = sql.to_owned();
    for _ in 0..256 {
        let found = crate::sql::checker::violations_with(&current, dialect);
        let Some(violation) = found.iter().min_by_key(|violation| (violation.span.start, std::cmp::Reverse(violation.span.len()))) else {
            break;
        };
        let Some(replacement) = render(&current, violation) else {
            return Err(crate::error::Error::Rejected(format!("{} layout cannot be rendered in this context", violation.kind)));
        };
        if replacement == current[violation.span.start..violation.span.end] {
            break;
        }
        current.replace_range(violation.span.start..violation.span.end, &replacement);
    }
    Ok(current)
}

fn render(text: &str, violation: &crate::sql::checker::Violation) -> Option<String> {
    if violation.kind == "set-op" {
        let operator = text[violation.span.start..violation.span.end].trim();
        return Some(format!("\n{}{operator}", " ".repeat(violation.indent)));
    }
    if violation.kind == "set-op-branch" {
        return Some(format!("\n{}", " ".repeat(violation.indent)));
    }
    if violation.kind == "set-op-parens" {
        let branch = text[violation.span.start..violation.span.end].trim();
        let masked = crate::sql::lexer::mask(branch);
        let mut depth = 0;
        let mut close = None;
        for (index, character) in masked.char_indices() {
            if character == '(' {
                depth += 1;
            } else if character == ')' {
                depth -= 1;
                if depth == 0 {
                    close = Some(index);
                    break;
                }
            }
        }
        if close != Some(branch.len() - 1) {
            return None;
        }
        return Some(branch[1..branch.len() - 1].trim().to_owned());
    }
    if violation.kind == "rollup" {
        let inner = &text[violation.span.start..violation.span.end];
        let args = crate::sql::splitter::split_elements("group by", inner);
        if args.is_empty() {
            return None;
        }
        let texts: Vec<String> = args.into_iter().map(|argument| argument.text).collect();
        return Some(match violation.expected {
            crate::sql::checker::Mode::Break => {
                let padding = " ".repeat(violation.indent + 4);
                let pad = " ".repeat(violation.indent);
                let joined = texts.join(&format!(",\n{padding}"));
                format!("\n{padding}{joined}\n{pad}")
            }
            crate::sql::checker::Mode::Inline => texts.join(", "),
        });
    }
    if violation.kind == "with" {
        return Some(format!("with\n{}", " ".repeat(violation.indent)));
    }
    if violation.kind == "with-close" {
        let pad = " ".repeat(violation.indent);
        let original = &text[violation.span.start..violation.span.end];
        return Some(if original.starts_with(' ') || original.starts_with('\t') {
            format!("{pad})")
        } else {
            format!("\n{pad})")
        });
    }
    if violation.kind == "with-query" {
        let keyword = text[violation.span.start..violation.span.end].trim();
        return Some(format!("\n{}{keyword}", " ".repeat(violation.indent)));
    }
    if violation.kind == "clause-start" {
        return Some(format!("\n{}", " ".repeat(violation.indent)));
    }
    if violation.kind == "group" {
        return render_group(text, violation);
    }
    if violation.kind == "case" {
        return render_case(text, violation);
    }
    if violation.kind == "subquery" {
        return render_subquery(text, violation);
    }
    if violation.kind == "cross-join" {
        return render_cross_join(text, violation);
    }
    if violation.kind == "from-join" {
        return render_from_join(text, violation);
    }
    let body = &text[violation.span.start..violation.span.end];
    let elements = crate::sql::splitter::split_elements(violation.kind, body);
    let has_next = violation.span.end < text.len();
    let base = violation.indent + 4;
    match violation.expected {
        crate::sql::checker::Mode::Break => {
            let mut lines: Vec<String> = Vec::new();
            if matches!(violation.kind, "where" | "having") {
                let oracle = elements.get(1).or_else(|| elements.first());
                let anchor = if oracle.is_some_and(|element| element.connector == crate::sql::splitter::Connector::Or) {
                    "1 = 0"
                } else {
                    "1 = 1"
                };
                lines.push(anchor.to_owned());
                for element in &elements {
                    lines.push(format!("{} {}", crate::sql::decision::connector_word(element.connector), element.text));
                }
            } else {
                for (index, element) in elements.iter().enumerate() {
                    let comma = if index + 1 < elements.len() { "," } else { "" };
                    lines.push(format!("{}{comma}", element.text));
                }
            }
            let padding = " ".repeat(base);
            let content = lines.iter().map(|line| format!("{padding}{line}")).collect::<Vec<_>>().join("\n");
            let suffix = if has_next {
                let indent = violation.next_indent.unwrap_or(violation.indent);
                format!("\n{}", " ".repeat(indent))
            } else {
                String::new()
            };
            if matches!(violation.kind, "where" | "having") {
                let anchor = &lines[0];
                Some(format!(" {anchor}\n{content}{suffix}", content = content.split_once('\n').map(|(_, rest)| rest).unwrap_or("")))
            } else {
                Some(format!("\n{content}{suffix}"))
            }
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
            Some(format!(" {joined}{suffix}"))
        }
    }
}

/// 布尔组：断行时补锚点与连接词，内联时丢锚点。
fn render_group(text: &str, violation: &crate::sql::checker::Violation) -> Option<String> {
    let inner = &text[violation.span.start..violation.span.end];
    let elements = crate::sql::splitter::split_elements("where", inner);
    if elements.is_empty() {
        return None;
    }
    if elements.len() == 1 {
        let body = elements[0].text.trim();
        return Some(match violation.expected {
            crate::sql::checker::Mode::Break => {
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
            crate::sql::checker::Mode::Inline => body.to_owned(),
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
        crate::sql::checker::Mode::Break => {
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
        crate::sql::checker::Mode::Inline => {
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

/// 独立 `case`：展开时每个分支关键字独占一行。
fn render_case(text: &str, violation: &crate::sql::checker::Violation) -> Option<String> {
    let case_text = &text[violation.span.start..violation.span.end];
    let parts = case_parts(case_text)?;
    match violation.expected {
        crate::sql::checker::Mode::Break => {
            let padding = " ".repeat(violation.indent + 4);
            let pad = " ".repeat(violation.indent);
            let mut lines = vec!["case".to_owned()];
            for (keyword, content) in parts {
                lines.push(format!("{padding}{keyword} {content}"));
            }
            lines.push(format!("{pad}end"));
            Some(lines.join("\n"))
        }
        crate::sql::checker::Mode::Inline => {
            let mut joined = String::from("case");
            for (keyword, content) in parts {
                joined.push(' ');
                joined.push_str(keyword);
                joined.push(' ');
                joined.push_str(&content);
            }
            joined.push_str(" end");
            Some(joined)
        }
    }
}

/// 把 `case … end` 拆成 `(when|then|else, 正文)` 序列；嵌套 `case` 不拆。
fn case_parts(text: &str) -> Option<Vec<(&'static str, String)>> {
    let tokens = crate::sql::lexer::tokens(text);
    let mut parts: Vec<(&'static str, String)> = Vec::new();
    let mut current: Option<(&'static str, usize)> = None;
    let mut depth = 0;
    for token in &tokens {
        if token.kind != crate::sql::lexer::Kind::Word {
            continue;
        }
        let lower = token.text.to_ascii_lowercase();
        if lower == "case" {
            depth += 1;
            continue;
        }
        if lower == "end" {
            depth -= 1;
            if depth == 0 {
                if let Some((keyword, start)) = current.take() {
                    parts.push((keyword, text[start..token.start].trim().to_owned()));
                }
                return Some(parts);
            }
            continue;
        }
        if depth == 1 && matches!(lower.as_str(), "when" | "then" | "else") {
            if let Some((keyword, start)) = current.take() {
                parts.push((keyword, text[start..token.start].trim().to_owned()));
            }
            let keyword = match lower.as_str() {
                "when" => "when",
                "then" => "then",
                _ => "else",
            };
            current = Some((keyword, token.start + token.text.len()));
        }
    }
    None
}

/// 内联子查询体展开：每个子句独占一行，闭括号回到元素缩进。
fn render_subquery(text: &str, violation: &crate::sql::checker::Violation) -> Option<String> {
    let block = &text[violation.span.start..violation.span.end];
    if starts_with_word(block, "with") {
        return None;
    }
    let clauses = crate::sql::splitter::clause_spans(block);
    if clauses.is_empty() || clauses.iter().any(|clause| crate::sql::splitter::SET_OPERATORS.contains(&clause.kind)) {
        return None;
    }
    let padding = " ".repeat(violation.indent + 4);
    let pad = " ".repeat(violation.indent);
    let mut lines = Vec::new();
    for clause in &clauses {
        let body = block[clause.body_start..clause.body_end].trim();
        if body.is_empty() {
            continue;
        }
        lines.push(format!("{padding}{} {body}", clause.kind));
    }
    if lines.is_empty() {
        return None;
    }
    Some(format!("\n{}\n{pad}", lines.join("\n")))
}

/// `from` 的 join 版式：首表留在 `from` 行，join 行回到 `from` 缩进。
fn render_from_join(text: &str, violation: &crate::sql::checker::Violation) -> Option<String> {
    let body = &text[violation.span.start..violation.span.end];
    if body.trim_start_matches([' ', '\t']).starts_with('\n') {
        return render_from_body(body, violation.indent);
    }
    let clause = body.trim();
    let pad = " ".repeat(violation.indent);
    let at_line_start = violation.span.start == 0 || text[..violation.span.start].ends_with('\n');
    Some(format!("{}{pad}{clause}", if at_line_start { "" } else { "\n" }))
}

/// 首表原在下一行时，重排整个 `from` 正文。
fn render_from_body(body: &str, indent: usize) -> Option<String> {
    let tokens = crate::sql::lexer::tokens(body);
    let mut depth = 0i32;
    let mut starts = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        match token.kind {
            crate::sql::lexer::Kind::Punct if token.text == "(" => depth += 1,
            crate::sql::lexer::Kind::Punct if token.text == ")" => depth -= 1,
            crate::sql::lexer::Kind::Word if depth == 0 && token.text.eq_ignore_ascii_case("join") => {
                let mut start = token.start;
                let mut probe = index;
                while probe > 0 {
                    probe -= 1;
                    let previous = &tokens[probe];
                    if previous.kind == crate::sql::lexer::Kind::Whitespace {
                        continue;
                    }
                    if previous.kind == crate::sql::lexer::Kind::Word && crate::sql::splitter::is_join_modifier(previous.text) {
                        start = previous.start;
                        continue;
                    }
                    break;
                }
                starts.push(start);
            }
            _ => {}
        }
    }
    if starts.is_empty() {
        return None;
    }
    let first = body[..starts[0]].trim();
    let mut rendered = String::new();
    if !first.is_empty() {
        rendered.push(' ');
        rendered.push_str(first);
    }
    for (index, &start) in starts.iter().enumerate() {
        let end = starts.get(index + 1).copied().unwrap_or(body.len());
        rendered.push('\n');
        rendered.push_str(&" ".repeat(indent));
        rendered.push_str(body[start..end].trim());
    }
    let trailing = body.trim_end().len();
    rendered.push_str(&body[trailing..]);
    Some(rendered)
}

/// 显式 join 后的逗号表改写为 `cross join`。
fn render_cross_join(_text: &str, violation: &crate::sql::checker::Violation) -> Option<String> {
    let pad = " ".repeat(violation.indent);
    Some(format!("\n{pad}cross join"))
}

/// 文本去掉前导空白后是否以某个词开头（按词边界判断）。
fn starts_with_word(text: &str, word: &str) -> bool {
    let Some(rest) = text.trim_start().strip_prefix(word) else {
        return false;
    };
    rest.chars().next().is_none_or(|character| !(character.is_alphanumeric() || character == '_' || character == '$'))
}

#[cfg(test)]
mod tests {
    /// `fix::format` 只渲染语句，不补块级分号；检查前补上 R26 的终止符。
    fn check_complete(sql: &str) -> bool {
        let text = if sql.trim_end().ends_with(';') { sql.to_owned() } else { format!("{sql};") };
        crate::sql::check(&text).is_empty()
    }

    #[test]
    fn breaks_four_fields_and_is_idempotent() {
        let sql = "select a, b, c, d from t";
        let fixed = crate::sql::fix::format(sql).expect("format");
        assert!(fixed.contains("select\n    a,"));
        assert!(check_complete(&fixed), "{fixed}");
        assert_eq!(crate::sql::fix::format(&fixed).expect("second"), fixed);
    }

    #[test]
    fn inlines_three_conditions() {
        let sql = "select a from t where\n    1 = 1\n    and a = 1\n    and b = 2\n    and c = 3\n";
        let fixed = crate::sql::format(sql).expect("format");
        assert_eq!(fixed.trim(), "select a\nfrom t\nwhere a = 1 and b = 2 and c = 3;");
        assert!(check_complete(&fixed), "{fixed}");
    }

    #[test]
    fn breaks_wrapped_single_condition_with_subqueries() {
        let sql = "select a from t where a = 1 and (x between (select 1 from u where p = 1 and q = 2) and (select 2 from u))";
        let fixed = crate::sql::format(sql).expect("format");
        assert_eq!(
            fixed,
            "select a\nfrom t\nwhere 1 = 1\n    and a = 1\n    and (\n        x between (\n            select 1\n            from u\n            where p = 1 and q = 2\n        ) and (select 2 from u)\n    );"
        );
        assert_eq!(crate::sql::format(&fixed).expect("second"), fixed);
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

    #[test]
    fn breaks_inline_join() {
        let sql = "select a from t left join u on t.id = u.id";
        let fixed = crate::sql::format(sql).expect("format");
        assert_eq!(fixed, "select a\nfrom t\nleft join u on t.id = u.id;");
        assert_eq!(crate::sql::format(&fixed).expect("second"), fixed);
    }

    #[test]
    fn breaks_join_after_from_newline() {
        let sql = "select a\nfrom\n    t left join u on t.id = u.id";
        let fixed = crate::sql::format(sql).expect("format");
        assert_eq!(fixed, "select a\nfrom t\nleft join u on t.id = u.id;");
    }

    #[test]
    fn reindents_join() {
        let sql = "select a\nfrom t\n        left join u on t.id = u.id";
        let fixed = crate::sql::format(sql).expect("format");
        assert_eq!(fixed, "select a\nfrom t\nleft join u on t.id = u.id;");
    }

    #[test]
    fn expands_breaking_group() {
        let sql = "select a\nfrom t\nwhere 1 = 1\n    and (x = 1 or y = 2 or z = 3 or w = 4)";
        let fixed = crate::sql::fix::format(sql).expect("format");
        assert!(fixed.contains("where (\n    1 = 0\n"), "{fixed}");
        assert!(!fixed.contains("1 = 1"), "{fixed}");
        assert!(check_complete(&fixed), "{fixed}");
    }

    #[test]
    fn inlines_simple_group() {
        let sql = "select a\nfrom t\nwhere 1 = 1\n    and (\n        1 = 0\n        or x = 1\n        or y = 2\n    )\n    and b = 2\n    and c = 3";
        let fixed = crate::sql::fix::format(sql).expect("format");
        assert!(fixed.contains("and (x = 1 or y = 2)"), "{fixed}");
        assert!(check_complete(&fixed), "{fixed}");
    }

    #[test]
    fn expands_branch_complex_case() {
        let sql = "select (case when x > 0 then abs(y) else null end) as v\nfrom t";
        let fixed = crate::sql::fix::format(sql).expect("format");
        assert!(fixed.contains("(case\n"), "{fixed}");
        assert!(fixed.contains("\nend) as v"), "{fixed}");
        assert!(check_complete(&fixed), "{fixed}");
    }

    #[test]
    fn expands_inline_subquery() {
        let sql = "select a from t where x in (select y from u where a = 1 and b = 2)";
        let fixed = crate::sql::fix::format(sql).expect("format");
        assert!(fixed.contains("x in (\n    select y\n"), "{fixed}");
        assert!(check_complete(&fixed), "{fixed}");
        assert_eq!(crate::sql::fix::format(&fixed).expect("second"), fixed);
    }

    #[test]
    fn breaks_set_operation_lines() {
        let sql = "select a from t1 union all select b from t2";
        let fixed = crate::sql::fix::format(sql).expect("format");
        assert!(fixed.contains("select a from t1\nunion all\nselect b from t2"), "{fixed}");
        assert!(check_complete(&fixed), "{fixed}");
        assert_eq!(crate::sql::fix::format(&fixed).expect("second"), fixed);
    }

    #[test]
    fn strips_set_operation_branch_parens() {
        let sql = "select a from t1 except (select a from t2)";
        let fixed = crate::sql::fix::format(sql).expect("format");
        assert!(fixed.contains("select a from t1\nexcept\nselect a from t2"), "{fixed}");
        assert!(check_complete(&fixed), "{fixed}");
    }

    #[test]
    fn expands_rollup_arguments() {
        let sql = "select a from t group by rollup(a, b, c, d)";
        let fixed = crate::sql::fix::format(sql).expect("format");
        assert!(fixed.contains("rollup(\n    a,\n    b,\n    c,\n    d\n)"), "{fixed}");
        assert!(check_complete(&fixed), "{fixed}");
    }

    #[test]
    fn lays_out_with_frame() {
        let sql = "with x as (select a from t) select a from x";
        let fixed = crate::sql::fix::format(sql).expect("format");
        assert!(fixed.contains("with\nx as (select a from t)\nselect a\nfrom x"), "{fixed}");
        assert!(check_complete(&fixed), "{fixed}");
        assert_eq!(crate::sql::fix::format(&fixed).expect("second"), fixed);
    }

    #[test]
    fn aligns_cte_closing_paren() {
        let sql = "with\nx as (\n    select a\n    from t\n    )\nselect a from x";
        let fixed = crate::sql::fix::format(sql).expect("format");
        assert!(fixed.contains("\n)\nselect a"), "{fixed}");
        assert!(check_complete(&fixed), "{fixed}");
    }

    #[test]
    fn rewrites_implicit_cross_join() {
        let sql = "select a\nfrom t1\nleft join t2 on t1.id = t2.id, t3";
        let fixed = crate::sql::fix::format(sql).expect("format");
        assert!(fixed.contains("cross join t3"), "{fixed}");
        assert!(check_complete(&fixed), "{fixed}");
    }
}
