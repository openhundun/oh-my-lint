//! SQL 规则的对外行为测试：诊断、修复、幂等与语料形态。

#[test]
fn three_aliased_fields_break() {
    let sql = "select a as x, b as y, c as z from t";
    assert!(!crate::sql::check(sql).is_empty());
    let fixed = crate::sql::format(sql).expect("format");
    assert!(crate::sql::check(&fixed).is_empty(), "{fixed}");
}

#[test]
fn window_function_breaks_select() {
    let sql = "select a, rank() over(order by b) as r from t";
    assert!(!crate::sql::check(sql).is_empty());
    let fixed = crate::sql::format(sql).expect("format");
    assert!(crate::sql::check(&fixed).is_empty(), "{fixed}");
}

#[test]
fn derived_table_breaks_from() {
    let sql = "select x from (select a from t) x, u";
    assert!(!crate::sql::check(sql).is_empty());
    let fixed = crate::sql::format(sql).expect("format");
    assert!(crate::sql::check(&fixed).is_empty(), "{fixed}");
}

#[test]
fn sentinel_shape_matches_corpus() {
    let sql =
        "select a\nfrom t\nwhere 1 = 1\n    and a = 1\n    and b = 2\n    and c = 3\n    and d = 4";
    assert!(
        crate::sql::check(sql).is_empty(),
        "{:?}",
        crate::sql::check(sql)
    );
    assert_eq!(crate::sql::format(sql).expect("format"), sql);
}

/// 阶段 3 语料：DDL/DML 夹具的 `check` 必须干净。
///
/// 夹具刻意保留末尾分号（R26 只在修复时去掉），因此不断言
/// `format == fixture`，只断言检查通过。
#[test]
fn ddl_dml_corpus_is_clean() {
    const FIXTURES: [(&str, &str); 7] = [
        (
            "alter_table.sql",
            include_str!("../../../../public/sqlfluff/postgres/alter_table.sql"),
        ),
        (
            "create_index.sql",
            include_str!("../../../../public/sqlfluff/postgres/create_index.sql"),
        ),
        (
            "create_table.sql",
            include_str!("../../../../public/sqlfluff/postgres/create_table.sql"),
        ),
        (
            "create_table_as.sql",
            include_str!("../../../../public/sqlfluff/postgres/create_table_as.sql"),
        ),
        (
            "create_table_column_storage_compression.sql",
            include_str!(
                "../../../../public/sqlfluff/postgres/create_table_column_storage_compression.sql"
            ),
        ),
        (
            "delete.sql",
            include_str!("../../../../public/sqlfluff/postgres/delete.sql"),
        ),
        (
            "explain.sql",
            include_str!("../../../../public/sqlfluff/postgres/explain.sql"),
        ),
    ];
    for (name, sql) in FIXTURES {
        let findings = crate::sql::check(sql);
        assert!(findings.is_empty(), "{name}: {findings:?}");
    }
}

#[test]
fn inline_subquery_is_expanded() {
    let sql = "select a from t where x in (select y from u group by y having count(*) > 1 and max(z) > 0 and min(z) < 9 and sum(z) > 1)";
    assert!(
        crate::sql::check(sql)
            .iter()
            .any(|finding| finding.rule == "sql-break")
    );
    let fixed = crate::sql::format(sql).expect("format");
    assert!(crate::sql::check(&fixed).is_empty(), "{fixed}");
}

const BROKEN: &str = "select a from t where (x between (select 1 from u)) and (select 2 from u)";

fn block(text: &str) -> crate::model::Block {
    crate::model::Block {
        span: crate::span::Span::default(),
        text: text.to_owned(),
        language: crate::model::Language::Sql,
    }
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
        assert!(
            findings
                .iter()
                .all(|finding| !finding.message.contains("does not parse")),
            "{dialect:?}: {findings:?}"
        );
    }
    assert_eq!(
        crate::sql::format_with(sql, crate::sql::Dialect::Postgres).expect("format"),
        sql
    );
}

#[test]
fn mixed_file_keeps_unparseable_statement() {
    let prefix = "select a as x, b as y, c as z from t;\n";
    let sql = format!("{prefix}{BROKEN};\nselect a from u;");
    let findings = crate::sql::check(&sql);
    assert!(!findings.is_empty(), "{findings:?}");
    assert!(
        findings
            .iter()
            .all(|finding| !finding.message.contains("does not parse")),
        "{findings:?}"
    );
    let bad = crate::span::Span::new(prefix.len(), prefix.len() + BROKEN.len());
    assert!(
        findings.iter().all(|finding| !finding.span.intersects(bad)),
        "{findings:?}"
    );
    let formatted = crate::sql::format(&sql).expect("format");
    assert!(formatted.contains(BROKEN), "{formatted}");
    assert!(
        !formatted.contains("select a as x, b as y, c as z"),
        "{formatted}"
    );
    assert!(formatted.ends_with("select a from u"), "{formatted}");
    assert_eq!(crate::sql::format(&formatted).expect("format"), formatted);
}

#[test]
fn final_semicolon_tracks_last_statement() {
    let all_processed = "select a from t;\nselect b from u;";
    assert_eq!(
        crate::sql::format(all_processed).expect("format"),
        "select a from t;\nselect b from u"
    );
    let last_skipped = "select a from t;\ninsert into t (a) values (1);";
    assert_eq!(
        crate::sql::format(last_skipped).expect("format"),
        last_skipped
    );
}

#[test]
fn fix_mode_accepts_unparseable_block() {
    use crate::model::Rule;

    let rule = crate::sql::SqlRule::default();
    let broken = block(BROKEN);
    assert!(rule.can_fix(&broken).is_ok());
    assert!(rule.fix(&broken).is_none());
    assert!(rule.can_fix(&block("select a from t -- note")).is_err());

    let mixed = block(&format!("select a as x, b as y, c as z from t;\n{BROKEN};"));
    let fix = rule.fix(&mixed).expect("fix");
    let replacement = &fix.edits[0].replacement;
    assert!(replacement.contains(BROKEN), "{replacement}");
    assert!(
        replacement.ends_with(&format!("{BROKEN};")),
        "{replacement}"
    );
}
