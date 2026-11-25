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
