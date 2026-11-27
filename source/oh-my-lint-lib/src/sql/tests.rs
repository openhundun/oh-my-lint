//! SQL 规则的对外行为测试：诊断、修复、幂等与语料形态。

#[test]
fn three_aliased_fields_break() {
    let sql = "select a as x, b as y, c as z from t";
    assert!(!crate::sql::check(sql).is_empty());
    let fixed = crate::sql::format(sql).expect("format");
    assert!(crate::sql::check(&fixed).is_empty(), "{fixed}");
}

#[test]
fn window_function_stays_inline() {
    let sql = "select a, rank() over(order by b) as r from t;";
    assert!(crate::sql::check(sql).is_empty(), "{:?}", crate::sql::check(sql));
    assert_eq!(crate::sql::format(sql).expect("format"), sql);
}

#[test]
fn derived_table_breaks_from() {
    let sql = "select x from (select a from t) x, u";
    assert!(!crate::sql::check(sql).is_empty());
    let fixed = crate::sql::format(sql).expect("format");
    assert!(crate::sql::check(&fixed).is_empty(), "{fixed}");
}

#[test]
fn wraps_break_on_condition() {
    let sql = "select a from t join u on t.id = u.id and t.kind = u.kind and t.deleted = 0 and u.deleted = 0;";
    let fixed = crate::sql::format(sql).expect("format");
    assert_eq!(
        fixed,
        "select a\nfrom t\njoin u on (\n    1 = 1\n    and t.id = u.id\n    and t.kind = u.kind\n    and t.deleted = 0\n    and u.deleted = 0\n);"
    );
    assert!(crate::sql::check(&fixed).is_empty(), "{fixed}");
    assert_eq!(crate::sql::format(&fixed).expect("second"), fixed);
}

#[test]
fn inline_on_and_where_are_not_wrapped() {
    let on = "select a from t join u on t.id = u.id and t.deleted = 0;";
    let fixed = crate::sql::format(on).expect("format");
    assert_eq!(fixed, "select a\nfrom t\njoin u on t.id = u.id and t.deleted = 0;");

    let where_sql = "select a from t where a = 1 and b = 2 and c = 3;";
    let fixed = crate::sql::format(where_sql).expect("format");
    assert_eq!(fixed, "select a\nfrom t\nwhere a = 1 and b = 2 and c = 3;");
}

#[test]
fn single_subquery_condition_stays_unwrapped() {
    let sql = "select a from t where exists (select 1 from u where a = 1 and b = 2 and c = 3 and d = 4);";
    let fixed = crate::sql::format(sql).expect("format");
    assert!(fixed.starts_with("select a\nfrom t\nwhere exists ("), "{fixed}");
    assert!(crate::sql::check(&fixed).is_empty(), "{fixed}");
}

#[test]
fn unwraps_inline_condition_bodies() {
    let where_sql = "select a\nfrom t\nwhere (\n    1 = 1\n    and a = 1\n    and b = 2\n);";
    assert_eq!(crate::sql::format(where_sql).expect("format"), "select a\nfrom t\nwhere a = 1 and b = 2;");

    let on_sql = "select a from t join u on (t.id = u.id and t.deleted = 0);";
    assert_eq!(crate::sql::format(on_sql).expect("format"), "select a\nfrom t\njoin u on t.id = u.id and t.deleted = 0;");
}

#[test]
fn wraps_each_break_on_condition_in_multi_join() {
    let sql =
        "select a from t join u on t.id = u.id and t.kind = u.kind and t.deleted = 0 and u.deleted = 0 join v on v.id = u.id and v.kind = u.kind and v.deleted = 0 and v.status = 1 where t.a = 1;";
    let fixed = crate::sql::format(sql).expect("format");
    assert_eq!(
        fixed,
        "select a\n\
         from t\n\
         join u on (\n\
         \u{20}   1 = 1\n\
         \u{20}   and t.id = u.id\n\
         \u{20}   and t.kind = u.kind\n\
         \u{20}   and t.deleted = 0\n\
         \u{20}   and u.deleted = 0\n\
         )\n\
         join v on (\n\
         \u{20}   1 = 1\n\
         \u{20}   and v.id = u.id\n\
         \u{20}   and v.kind = u.kind\n\
         \u{20}   and v.deleted = 0\n\
         \u{20}   and v.status = 1\n\
         )\n\
         where t.a = 1;"
    );
    assert!(crate::sql::check(&fixed).is_empty(), "{fixed}");
    assert_eq!(crate::sql::format(&fixed).expect("second"), fixed);
}

#[test]
fn ctas_keeps_break_where_block() {
    let sql = "create temp table t as (select id from u where a = 1 and b = 2 and c = 3 and d = 4);";
    let fixed = crate::sql::format(sql).expect("format");
    assert_eq!(
        fixed,
        "create temp table t as (\n    select id\n    from u\n    where (\n        1 = 1\n        and a = 1\n        and b = 2\n        and c = 3\n        and d = 4\n    )\n);"
    );
    assert!(crate::sql::check(&fixed).is_empty(), "{fixed}");
}

#[test]
fn wraps_break_where_and_having_before_next_clauses() {
    let sql = "select a, count(*) from t where a = 1 and b = 2 and c = 3 and d = 4 group by a having count(*) > 1 and sum(b) > 2 and min(c) > 3 and max(d) > 4 order by a;";
    let fixed = crate::sql::format(sql).expect("format");
    assert!(
        fixed.contains("where (\n    1 = 1\n    and a = 1\n    and b = 2\n    and c = 3\n    and d = 4\n)\ngroup by a\n"),
        "{fixed}"
    );
    assert!(
        fixed.contains("having (\n    1 = 1\n    and count(*) > 1\n    and sum(b) > 2\n    and min(c) > 3\n    and max(d) > 4\n)\norder by a;"),
        "{fixed}"
    );
    assert!(crate::sql::check(&fixed).is_empty(), "{fixed}");
    assert_eq!(crate::sql::format(&fixed).expect("second"), fixed);
}

#[test]
fn single_subquery_on_stays_unwrapped() {
    let sql = "select a from t join u on exists (select 1 from v where a = 1 and b = 2 and c = 3 and d = 4);";
    let fixed = crate::sql::format(sql).expect("format");
    assert!(fixed.contains("join u on exists ("), "{fixed}");
    assert!(crate::sql::check(&fixed).is_empty(), "{fixed}");
}

#[test]
fn on_condition_stops_before_cross_join_comma() {
    let sql = "select a from t join u on t.id = u.id and t.kind = u.kind and t.deleted = 0 and u.deleted = 0, v;";
    let fixed = crate::sql::format(sql).expect("format");
    assert_eq!(
        fixed,
        "select a\nfrom t\njoin u on (\n    1 = 1\n    and t.id = u.id\n    and t.kind = u.kind\n    and t.deleted = 0\n    and u.deleted = 0\n)\ncross join v;"
    );
    assert!(crate::sql::check(&fixed).is_empty(), "{fixed}");
}

#[test]
fn simple_three_clause_subquery_stays_inline() {
    let sql = "select a from t where b < (\n    select 0.2 * avg(c)\n    from u\n    where u.id = t.id\n);";
    let fixed = crate::sql::format(sql).expect("format");
    assert_eq!(fixed, "select a\nfrom t\nwhere b < (select 0.2 * avg(c) from u where u.id = t.id);");
    assert!(crate::sql::check(&fixed).is_empty(), "{fixed}");
}

#[test]
fn window_case_stays_inline() {
    let sql = "select rank() over(partition by (case when grouping(s_state) = 0 then s_state end) order by sum(x) desc) as r from t;";
    let fixed = crate::sql::format(sql).expect("format");
    assert_eq!(
        fixed,
        "select rank() over(partition by (case when grouping(s_state) = 0 then s_state end) order by sum(x) desc) as r from t;"
    );
    assert!(crate::sql::check(&fixed).is_empty(), "{fixed}");

    let expanded = "select rank() over(partition by (case\n    when grouping(s_state) = 0\n    then s_state\nend) order by sum(x) desc) as r from t;";
    assert_eq!(crate::sql::format(expanded).expect("format"), fixed);
}

#[test]
fn inline_where_keeps_simple_query_inline() {
    let sql = "select a from t where ((a = 1)) order by a;";
    let fixed = crate::sql::format(sql).expect("format");
    assert_eq!(fixed, "select a from t where a = 1 order by a;");
    assert!(crate::sql::check(&fixed).is_empty(), "{fixed}");
}

#[test]
fn wraps_break_on_or_chain() {
    let sql = "select a from t join u on t.id = u.id or t.kind = u.kind or t.deleted = 0 or u.deleted = 0;";
    let fixed = crate::sql::format(sql).expect("format");
    assert_eq!(
        fixed,
        "select a\nfrom t\njoin u on (\n    1 = 0\n    or t.id = u.id\n    or t.kind = u.kind\n    or t.deleted = 0\n    or u.deleted = 0\n);"
    );
    assert!(crate::sql::check(&fixed).is_empty(), "{fixed}");
}

#[test]
fn sentinel_shape_matches_corpus() {
    let sql = "select a\nfrom t\nwhere (\n    1 = 1\n    and a = 1\n    and b = 2\n    and c = 3\n    and d = 4\n);";
    assert!(crate::sql::check(sql).is_empty(), "{:?}", crate::sql::check(sql));
    assert_eq!(crate::sql::format(sql).expect("format"), sql);
}

/// 阶段 3 语料：DDL/DML 夹具的 `check` 必须干净。
///
/// 夹具已经过 formatter 处理；仍不断言 `format == fixture`，因为
/// 不支持的语句会按 R34 保留原始字节，只断言检查通过。
#[test]
fn ddl_dml_corpus_is_clean() {
    const FIXTURES: [(&str, &str); 7] = [
        ("alter_table.sql", include_str!("../../../../public/sqlfluff/postgres/alter_table.sql")),
        ("create_index.sql", include_str!("../../../../public/sqlfluff/postgres/create_index.sql")),
        ("create_table.sql", include_str!("../../../../public/sqlfluff/postgres/create_table.sql")),
        ("create_table_as.sql", include_str!("../../../../public/sqlfluff/postgres/create_table_as.sql")),
        (
            "create_table_column_storage_compression.sql",
            include_str!("../../../../public/sqlfluff/postgres/create_table_column_storage_compression.sql"),
        ),
        ("delete.sql", include_str!("../../../../public/sqlfluff/postgres/delete.sql")),
        ("explain.sql", include_str!("../../../../public/sqlfluff/postgres/explain.sql")),
    ];
    for (name, sql) in FIXTURES {
        let findings = crate::sql::check(sql);
        assert!(findings.is_empty(), "{name}: {findings:?}");
    }
}

#[test]
fn inline_subquery_is_expanded() {
    let sql = "select a from t where x in (select y from u group by y having count(*) > 1 and max(z) > 0 and min(z) < 9 and sum(z) > 1)";
    assert!(crate::sql::check(sql).iter().any(|finding| finding.rule == "sql-break"));
    let fixed = crate::sql::format(sql).expect("format");
    assert!(crate::sql::check(&fixed).is_empty(), "{fixed}");
}

const BROKEN: &str = "select a from t where (x between (select 1 from u)) and (select 2 from u)";

fn block(text: &str) -> crate::model::Block {
    crate::model::Block { span: crate::span::Span::default(), text: text.to_owned(), language: crate::model::Language::Sql }
}

#[test]
fn skips_unparseable_statement() {
    let findings = crate::sql::check(BROKEN);
    assert!(findings.is_empty(), "{findings:?}");
    assert_eq!(crate::sql::format(BROKEN).expect("format"), BROKEN);
}

#[test]
fn dialect_controls_statement_handling() {
    let sql = "select a from t distribute by a";
    for dialect in [crate::sql::Dialect::Postgres, crate::sql::Dialect::Hive] {
        let findings = crate::sql::check_with(sql, dialect);
        assert!(findings.iter().all(|finding| !finding.message.contains("does not parse")), "{dialect:?}: {findings:?}");
    }
    assert_eq!(crate::sql::format_with(sql, crate::sql::Dialect::Postgres).expect("format"), sql);
}

#[test]
fn mixed_file_keeps_unparseable_statement() {
    let prefix = "select a as x, b as y, c as z from t;\n";
    let sql = format!("{prefix}{BROKEN};\nselect a from u;");
    let findings = crate::sql::check(&sql);
    assert!(!findings.is_empty(), "{findings:?}");
    assert!(findings.iter().all(|finding| !finding.message.contains("does not parse")), "{findings:?}");
    let bad = crate::span::Span::new(prefix.len(), prefix.len() + BROKEN.len());
    assert!(findings.iter().all(|finding| !finding.span.intersects(bad)), "{findings:?}");
    let formatted = crate::sql::format(&sql).expect("format");
    assert!(formatted.contains(BROKEN), "{formatted}");
    assert!(!formatted.contains("select a as x, b as y, c as z"), "{formatted}");
    assert!(formatted.ends_with("select a from u;"), "{formatted}");
    assert_eq!(crate::sql::format(&formatted).expect("format"), formatted);
}

#[test]
fn values_derived_table_is_not_a_subquery() {
    let sql = "select *\nfrom (values (1), (2)) a (x),\n    (values (3)) b (y);";
    let findings = crate::sql::check(sql);
    assert!(findings.is_empty(), "{findings:?}");
}

#[test]
fn update_with_values_derived_is_clean() {
    let sql = "update sys_permission p\n\
               set resource_path = spec.resource_path, http_method = 'POST'\n\
               from (\n\
               \u{20}   values\n\
               \u{20}   ('dlp:read:21', '/api/a'),\n\
               \u{20}   ('dlp:read:22', '/api/b')\n\
               ) spec (permission_code, resource_path)\n\
               where p.permission_code = spec.permission_code and p.deleted = 0;";
    assert!(crate::sql::check(sql).is_empty(), "{:?}", crate::sql::check(sql));
    assert_eq!(crate::sql::format(sql).expect("format"), sql);
}

#[test]
fn unsupported_layout_still_normalizes_tokens() {
    let sql = "ALTER FOREIGN TABLE t ADD COLUMN c INT;";
    let fixed = crate::sql::format(sql).expect("format");
    assert_eq!(fixed, "alter foreign table t add column c int;");
    assert!(crate::sql::check(&fixed).is_empty());
    assert_eq!(crate::sql::format(&fixed).expect("format"), fixed);
}

#[test]
fn options_keyword_keeps_space() {
    let sql = "ALTER FOREIGN TABLE t OPTIONS (ADD a 'b');";
    let fixed = crate::sql::format(sql).expect("format");
    assert_eq!(fixed, "alter foreign table t options (add a 'b');");
}

#[test]
fn other_statements_are_token_normalized() {
    let sql = "INSERT INTO t (a) VALUES (1)";
    assert!(!crate::sql::check(sql).is_empty());
    let fixed = crate::sql::format(sql).expect("format");
    assert_eq!(fixed, "insert into t (a) values (1);");
    assert!(crate::sql::check(&fixed).is_empty());
}

#[test]
fn final_semicolon_tracks_last_statement() {
    let all_processed = "select a from t;\nselect b from u;";
    assert_eq!(crate::sql::format(all_processed).expect("format"), all_processed);
    let last_skipped = "select a from t;\ninsert into t (a) values (1);";
    assert_eq!(crate::sql::format(last_skipped).expect("format"), last_skipped);
    let missing_last = "select a from t;\nselect b from u";
    assert_eq!(crate::sql::format(missing_last).expect("format"), "select a from t;\nselect b from u;");
}

#[test]
fn comments_are_removed_from_parseable_statements() {
    let sql = "-- head\nSELECT A FROM t;";
    let fixed = crate::sql::format(sql).expect("format");
    assert_eq!(fixed, "select a from t;");
    assert!(crate::sql::check(&fixed).is_empty());
}

#[test]
fn comments_before_unparseable_statements_are_removed() {
    let sql = "-- drop\nselect a from t where (x between (select 1 from u)) and (select 2 from u);";
    let fixed = crate::sql::format(sql).expect("format");
    assert_eq!(fixed, "select a from t where (x between (select 1 from u)) and (select 2 from u);");
}

#[test]
fn comments_inside_unparseable_statements_are_kept() {
    let sql = "select a -- keep\nfrom t where (x between (select 1 from u)) and (select 2 from u);";
    let fixed = crate::sql::format(sql).expect("format");
    assert_eq!(fixed, "select a -- keep\nfrom t where (x between (select 1 from u)) and (select 2 from u);");
}

#[test]
fn missing_final_semicolon_is_reported_and_fixed() {
    let sql = "select a from t";
    let findings = crate::sql::check(sql);
    assert!(findings.iter().any(|finding| finding.message.contains("semicolon")), "{findings:?}");
    let fixed = crate::sql::format(sql).expect("format");
    assert_eq!(fixed, "select a from t;");
    assert!(crate::sql::check(&fixed).is_empty(), "{fixed}");
}

#[test]
fn alter_table_name_starting_with_select_is_formatted() {
    let sql = "ALTER TABLE selected_table DROP COLUMN IF EXISTS datasource_id;";
    let fixed = crate::sql::format(sql).expect("format");
    assert_eq!(fixed, "alter table selected_table drop column if exists datasource_id;");
    assert!(crate::sql::check(&fixed).is_empty(), "{fixed}");
}

#[test]
fn fix_mode_accepts_unparseable_block() {
    use crate::model::Rule;

    let rule = crate::sql::SqlRule::default();
    let broken = block(BROKEN);
    assert!(rule.can_fix(&broken).is_ok());
    assert!(rule.fix(&broken).is_none());
    assert!(rule.can_fix(&block("select a from t -- note")).is_ok());

    let mixed = block(&format!("select a as x, b as y, c as z from t;\n{BROKEN};"));
    let fix = rule.fix(&mixed).expect("fix");
    let replacement = &fix.edits[0].replacement;
    assert!(replacement.contains(BROKEN), "{replacement}");
    assert!(replacement.ends_with(&format!("{BROKEN};")), "{replacement}");
}
