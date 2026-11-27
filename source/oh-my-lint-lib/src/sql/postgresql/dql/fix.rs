//! 修复：只重写违规区间，块内其余字节原样保留。

/// 目前能正确渲染的违规类型。
const SUPPORTED_FIXES: [&str; 22] = [
    "select",
    "from",
    "where",
    "having",
    "on",
    "group by",
    "order by",
    "clause-start",
    "subquery",
    "query-layout",
    "function-args",
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
        let Some(replacement) = render(&current, violation, dialect) else {
            return Err(crate::error::Error::Rejected(format!("{} layout cannot be rendered in this context", violation.kind)));
        };
        if replacement == current[violation.span.start..violation.span.end] {
            break;
        }
        current.replace_range(violation.span.start..violation.span.end, &replacement);
    }
    Ok(current)
}

fn render(text: &str, violation: &crate::sql::checker::Violation, dialect: crate::sql::parse::Dialect) -> Option<String> {
    if matches!(violation.kind, "set-op" | "set-op-branch" | "set-op-parens") {
        return super::set_op::render(text, violation);
    }
    if violation.kind == "rollup" {
        return super::rollup::render(text, violation);
    }
    if matches!(violation.kind, "with" | "with-close" | "with-query") {
        return super::with::render(text, violation);
    }
    if violation.kind == "clause-start" {
        return Some(format!("\n{}", " ".repeat(violation.indent)));
    }
    if violation.kind == "group" {
        return super::group::render(text, violation);
    }
    if violation.kind == "case" {
        return super::case::render(text, violation);
    }
    if violation.kind == "subquery" {
        return super::subquery::render(text, violation, dialect);
    }
    if violation.kind == "query-layout" {
        return super::subquery::format_nested(&text[violation.span.start..violation.span.end], violation.indent, dialect);
    }
    if violation.kind == "function-args" {
        return super::function::render(text, violation);
    }
    if violation.kind == "cross-join" {
        return super::join::render_cross_join(violation);
    }
    if violation.kind == "from-join" {
        return super::join::render_from_join(text, violation);
    }
    if matches!(violation.kind, "where" | "having") {
        return render_condition_body(text, violation);
    }
    if violation.kind == "on" {
        return render_condition_body(text, violation);
    }
    let body = &text[violation.span.start..violation.span.end];
    let elements = crate::sql::splitter::split_elements(violation.kind, body);
    let output = crate::sql::postgresql::dql::clause::render(violation.kind, &elements, violation.expected)?;
    let has_next = violation.span.end < text.len();
    match output {
        crate::sql::postgresql::dql::clause::ClauseOutput::Exline(lines) => {
            let padding = " ".repeat(violation.indent + 4);
            let suffix = if has_next {
                let indent = violation.next_indent.unwrap_or(violation.indent);
                format!("\n{}", " ".repeat(indent))
            } else {
                String::new()
            };
            let content = lines.iter().map(|line| format!("{padding}{line}")).collect::<Vec<_>>().join("\n");
            Some(format!("\n{content}{suffix}"))
        }
        crate::sql::postgresql::dql::clause::ClauseOutput::Inline(joined) => {
            let suffix = if has_next { " " } else { "" };
            Some(format!(" {joined}{suffix}"))
        }
        crate::sql::postgresql::dql::clause::ClauseOutput::Block(lines) => Some(render_block(&lines, violation.indent, has_next, violation.next_indent.unwrap_or(violation.indent))),
    }
}

/// `where` / `having` / `on`：解析条件正文，统一产出 inline 或括号块。
fn render_condition_body(text: &str, violation: &crate::sql::checker::Violation) -> Option<String> {
    let body = &text[violation.span.start..violation.span.end];
    let condition = crate::sql::postgresql::dql::r#where::parse(body);
    if condition.elements.is_empty() {
        return None;
    }
    let has_next = violation.span.end < text.len();
    let next_indent = violation.next_indent.unwrap_or(violation.indent);
    let next = text.get(violation.span.end..).and_then(|tail| tail.trim_start().chars().next());
    let next_is_comma = next == Some(',');
    let next_is_semicolon = next == Some(';');
    let multiline = !crate::sql::decision::is_simple(text);
    match crate::sql::postgresql::dql::r#where::mode(&condition.elements) {
        crate::sql::postgresql::dql::Mode::Inline => {
            let suffix = if !has_next || next_is_comma || next_is_semicolon {
                String::new()
            } else if multiline && next != Some(')') {
                format!("\n{}", " ".repeat(next_indent))
            } else {
                " ".to_owned()
            };
            Some(format!(" {}{suffix}", crate::sql::postgresql::dql::r#where::inline_text(&condition.elements)))
        }
        crate::sql::postgresql::dql::Mode::Break => Some(render_block(
            &crate::sql::postgresql::dql::r#where::block_lines(&condition.elements),
            violation.indent,
            has_next && !next_is_comma,
            next_indent,
        )),
    }
}

/// 条件块：` (` 与关键字同行，内容 +4，闭括号回关键字缩进。
fn render_block(lines: &[String], indent: usize, has_next: bool, next_indent: usize) -> String {
    let padding = " ".repeat(indent + 4);
    let content = lines.iter().map(|line| format!("{padding}{line}")).collect::<Vec<_>>().join("\n");
    let mut output = format!(" (\n{content}\n{})", " ".repeat(indent));
    if has_next {
        output.push('\n');
        output.push_str(&" ".repeat(next_indent));
    }
    output
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
            "select a\nfrom t\nwhere (\n    1 = 1\n    and a = 1\n    and (\n        x between (\n            select 1\n            from u\n            where p = 1 and q = 2\n        ) and (select 2 from u)\n    )\n);"
        );
        assert_eq!(crate::sql::format(&fixed).expect("second"), fixed);
    }

    #[test]
    fn wraps_break_where_and_stays_idempotent() {
        let sql = "select a, b\nfrom t\nwhere 1 = 1\n    and a = 1\n    and b = 2\n    and c = 3\n    and d = 4";
        let expected = "select a, b\nfrom t\nwhere (\n    1 = 1\n    and a = 1\n    and b = 2\n    and c = 3\n    and d = 4\n)";
        assert_eq!(crate::sql::fix::format(sql).expect("format"), expected);
        assert_eq!(crate::sql::fix::format(expected).expect("second"), expected);
    }

    #[test]
    fn keeps_canonical_break_where_untouched() {
        let sql = "select a, b\nfrom t\nwhere (\n    1 = 1\n    and a = 1\n    and b = 2\n    and c = 3\n    and d = 4\n)";
        assert_eq!(crate::sql::fix::format(sql).expect("format"), sql);
    }

    #[test]
    fn expands_function_args_with_a_subquery() {
        let sql = "select setval('seq', greatest(999999, (select coalesce(max(id), 999999) from t)))";
        let expected = "select\n\
                        \u{20}   setval(\n\
                        \u{20}       'seq',\n\
                        \u{20}       greatest(\n\
                        \u{20}           999999,\n\
                        \u{20}           (select coalesce(max(id), 999999) from t)\n\
                        \u{20}       )\n\
                        \u{20}   )";
        assert_eq!(crate::sql::fix::format(sql).expect("format"), expected);
        assert_eq!(crate::sql::fix::format(expected).expect("second"), expected);
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
    fn breaks_where_with_nested_group() {
        let sql = "select a\nfrom t\nwhere 1 = 1\n    and (\n        1 = 0\n        or x = 1\n        or y = 2\n    )\n    and b = 2\n    and c = 3";
        let fixed = crate::sql::fix::format(sql).expect("format");
        assert!(fixed.contains("where (\n    1 = 1\n    and (x = 1 or y = 2)\n    and b = 2\n    and c = 3\n)"), "{fixed}");
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
        let sql = "with\nx as (\n    select a\n    from t\n    where a = 1 and b = 2 and c = 3 and d = 4\n    )\nselect a from x";
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
