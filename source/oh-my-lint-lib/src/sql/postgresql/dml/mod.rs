//! DML 版式：`DELETE`（M1）与 `EXPLAIN`（M2）。

use sqlparser::ast::{SetExpr, Statement};

use crate::sql::decision;
use crate::sql::lexer::{self, Kind};
use crate::sql::parse::{self, Dialect};
use crate::sql::splitter::{self, Element};
use crate::sql::token;

mod delete;
mod explain;
mod insert;
pub(crate) mod on_conflict;
mod update;

/// 渲染 `DELETE`、`EXPLAIN`；无法渲染时返回 `None`。
pub(crate) fn layout(sql: &str, dialect: Dialect) -> Option<String> {
    lexer::with_dialect(dialect, || layout_inner(sql, dialect))
}

fn layout_inner(sql: &str, dialect: Dialect) -> Option<String> {
    let normalized = token::normalize(sql.trim());
    let normalized = normalized.trim_end_matches(';').trim_end();
    if normalized.is_empty() {
        return None;
    }
    let statements = parse::parse(normalized, dialect).ok()?;
    let output = match statements.as_slice() {
        [Statement::Delete(delete)] => delete::layout_delete(normalized, delete)?,
        [Statement::Update(_)] => update::layout_update(normalized, dialect)?,
        [Statement::Insert(_)] => insert::layout_insert(normalized, dialect)?,
        [Statement::Explain { statement, .. }] => explain::layout_explain(normalized, statement, dialect)?,
        [Statement::Query(query)] => match query.body.as_ref() {
            SetExpr::Delete(Statement::Delete(delete)) => delete::layout_delete(normalized, delete)?,
            SetExpr::Update(_) => update::layout_update(normalized, dialect)?,
            SetExpr::Insert(_) => insert::layout_insert(normalized, dialect)?,
            _ => return None,
        },
        _ => return None,
    };
    if parse::parse(&output, dialect).is_err() {
        return None;
    }
    Some(output)
}

/// `with` 框架单独排版后拼回 DML：借用 `select 1` 让 DQL 只处理 CTE 部分。
pub(super) fn format_cte_prefix(prefix: &str, dialect: Dialect) -> Option<String> {
    let synthetic = format!("{}\nselect 1", prefix.trim_end());
    let formatted = crate::sql::fix::format_with(&synthetic, dialect).ok()?;
    let trimmed = formatted.trim_end();
    let cut = trimmed.rfind("\nselect 1")?;
    let cte = trimmed[..cut].trim_end();
    (!cte.is_empty()).then(|| cte.to_owned())
}

/// `from`/`using` 的表列表；join 独行，首表留在关键字行。
pub(super) fn table_lines(body: &str, keyword: &str) -> Vec<String> {
    let starts = join_starts(body);
    if starts.is_empty() {
        return vec![format!("{keyword} {}", collapse(body))];
    }
    let first = collapse(&body[..starts[0]]);
    let mut lines = vec![if first.is_empty() { keyword.to_owned() } else { format!("{keyword} {first}") }];
    for (index, start) in starts.iter().enumerate() {
        let end = starts.get(index + 1).copied().unwrap_or(body.len());
        lines.push(collapse(&body[*start..end]));
    }
    lines
}

/// 一个深度 0 的单词。
pub(super) struct Word {
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) text: String,
}

/// `where`：与 DQL 共用条件块；内联不包，断行时整块加括号。
pub(super) fn where_lines(body: &str) -> Vec<String> {
    let condition = crate::sql::postgresql::dql::r#where::parse(body);
    let inner = condition.inner;
    let elements: Vec<Element> = condition
        .elements
        .iter()
        .map(|element| Element { text: collapse(&element.text), connector: element.connector })
        .collect();
    if elements.len() <= 1 || is_current_of(inner) {
        return vec![format!("where {}", collapse(inner))];
    }
    match crate::sql::postgresql::dql::r#where::mode(&elements) {
        crate::sql::postgresql::dql::Mode::Inline => vec![format!("where {}", crate::sql::postgresql::dql::r#where::inline_text(&elements))],
        crate::sql::postgresql::dql::Mode::Break => {
            let mut lines = vec!["where (".to_owned()];
            for line in crate::sql::postgresql::dql::r#where::block_lines(&elements) {
                lines.push(format!("    {line}"));
            }
            lines.push(")".to_owned());
            lines
        }
    }
}

fn is_current_of(body: &str) -> bool {
    let mut words = lexer::tokens(body).into_iter().filter(|token| token.kind == Kind::Word).map(|token| token.text.to_ascii_lowercase());
    words.next().as_deref() == Some("current") && words.next().as_deref() == Some("of")
}

/// `returning` 按 R7；别名必须写 `as`。
pub(super) fn returning_lines(body: &str) -> Vec<String> {
    let elements = returning_elements(body);
    if elements.is_empty() {
        return vec![format!("returning {}", collapse(body))];
    }
    let broken = elements.len() >= 4
        || elements.iter().any(|element| decision::is_complex(&element.text, "select"))
        || (elements.len() == 3 && elements.iter().all(|element| decision::has_alias(&element.text)));
    if !broken {
        let joined = elements.iter().map(|element| element.text.as_str()).collect::<Vec<_>>().join(", ");
        return vec![format!("returning {joined}")];
    }
    let mut lines = vec!["returning".to_owned()];
    for (index, element) in elements.iter().enumerate() {
        let comma = if index + 1 < elements.len() { "," } else { "" };
        lines.push(format!("    {}{comma}", element.text));
    }
    lines
}

pub(super) fn returning_elements(body: &str) -> Vec<Element> {
    splitter::split_elements("select", body)
        .into_iter()
        .map(|element| Element { text: alias_as(&element.text), connector: element.connector })
        .collect()
}

/// 补上 `returning` 的隐式别名；已有 `as` 的保持原样。
fn alias_as(text: &str) -> String {
    let tokens = lexer::tokens(text);
    let significant: Vec<&lexer::Token<'_>> = tokens.iter().filter(|token| !matches!(token.kind, Kind::Whitespace | Kind::Comment)).collect();
    if significant.len() < 2 {
        return collapse(text);
    }
    let last = significant[significant.len() - 1];
    if !matches!(last.kind, Kind::Word | Kind::QuotedIdent) {
        return collapse(text);
    }
    let previous = significant[significant.len() - 2];
    if previous.kind == Kind::Word && previous.text.eq_ignore_ascii_case("as") {
        return collapse(text);
    }
    let bare_pair = significant.len() == 2 && matches!(significant[0].kind, Kind::Word | Kind::QuotedIdent);
    let ends_expression = matches!(previous.kind, Kind::String | Kind::Number) || (previous.kind == Kind::Punct && previous.text == ")");
    if !(bare_pair || ends_expression) {
        return collapse(text);
    }
    if last.kind == Kind::Word && is_alias_stopword(last.text) {
        return collapse(text);
    }
    let prefix = collapse(&text[..last.start]);
    format!("{prefix} as {}", &text[last.start..last.start + last.text.len()])
}

fn is_alias_stopword(word: &str) -> bool {
    matches!(
        word.to_ascii_lowercase().as_str(),
        "null"
            | "true"
            | "false"
            | "end"
            | "when"
            | "then"
            | "else"
            | "and"
            | "or"
            | "not"
            | "is"
            | "in"
            | "between"
            | "like"
            | "ilike"
            | "asc"
            | "desc"
            | "distinct"
            | "all"
            | "any"
            | "some"
            | "case"
            | "exists"
    )
}

pub(super) fn top_words(text: &str) -> Vec<Word> {
    let mut depth = 0i32;
    let mut words = Vec::new();
    for token in lexer::tokens(text) {
        match token.kind {
            Kind::Punct if token.text == "(" => depth += 1,
            Kind::Punct if token.text == ")" => depth -= 1,
            Kind::Word if depth == 0 => words.push(Word { start: token.start, end: token.start + token.text.len(), text: token.text.to_ascii_lowercase() }),
            _ => {}
        }
    }
    words
}

/// 深度 0 的 join 起点，连同 `left`/`full` 等修饰词。
fn join_starts(text: &str) -> Vec<usize> {
    let tokens = lexer::tokens(text);
    let mut depth = 0i32;
    let mut starts = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        match token.kind {
            Kind::Punct if token.text == "(" => {
                depth += 1;
                continue;
            }
            Kind::Punct if token.text == ")" => {
                depth -= 1;
                continue;
            }
            Kind::Word if depth == 0 && token.text.eq_ignore_ascii_case("join") => {
                let mut start = token.start;
                let mut probe = index;
                while probe > 0 {
                    probe -= 1;
                    let previous = &tokens[probe];
                    if previous.kind == Kind::Whitespace {
                        continue;
                    }
                    if previous.kind == Kind::Word && splitter::is_join_modifier(previous.text) {
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
    starts
}

/// 把任意空白折叠成一个空格；字符串、引号标识符逐字节保留。
pub(super) fn collapse(text: &str) -> String {
    let mut output = String::new();
    let mut pending = false;
    for token in lexer::tokens(text) {
        match token.kind {
            Kind::Whitespace | Kind::Comment => {
                if !output.is_empty() {
                    pending = true;
                }
            }
            _ => {
                if pending {
                    output.push(' ');
                    pending = false;
                }
                output.push_str(token.text);
            }
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(sql: &str) -> String {
        let output = layout(sql, Dialect::Postgres).expect("layout");
        assert_eq!(layout(&output, Dialect::Postgres), Some(output.clone()));
        output
    }

    #[test]
    fn keeps_single_line_delete() {
        assert_eq!(render("delete from films where kind != 'Musical'"), "delete from films where kind != 'Musical'");
    }

    #[test]
    fn breaks_non_simple_delete_but_keeps_where_inline() {
        assert_eq!(
            render("delete from films f using producers p where f.producer_id = p.id and p.name = 'foo'"),
            "delete from films f\nusing producers p\nwhere f.producer_id = p.id and p.name = 'foo'"
        );
    }

    #[test]
    fn breaks_four_where_conditions_with_anchor() {
        let sql = "delete from films f using producers p, actors a where 1 = 1 and f.producer_id = p.id and p.name = 'foo' and f.actor_id = a.id and a.name = 'joe cool'";
        assert_eq!(
            render(sql),
            "delete from films f\n\
             using producers p, actors a\n\
             where (\n\
             \u{20}   1 = 1\n\
             \u{20}   and f.producer_id = p.id\n\
             \u{20}   and p.name = 'foo'\n\
             \u{20}   and f.actor_id = a.id\n\
             \u{20}   and a.name = 'joe cool'\n\
             )"
        );
    }

    #[test]
    fn or_conditions_use_zero_anchor() {
        let sql = "delete from t where a = 1 or b = 2 or c = 3 or d = 4";
        assert_eq!(render(sql), "delete from t\nwhere (\n    1 = 0\n    or a = 1\n    or b = 2\n    or c = 3\n    or d = 4\n)");
    }

    #[test]
    fn puts_using_join_on_its_own_line() {
        assert_eq!(
            render("delete from foo using baz left join bar on baz.bar_id = bar.id where foo.id = bar.foo_id and baz.id = 'c'"),
            "delete from foo\n\
             using baz\n\
             left join bar on baz.bar_id = bar.id\n\
             where foo.id = bar.foo_id and baz.id = 'c'"
        );
    }

    #[test]
    fn returning_alias_gets_as() {
        assert_eq!(
            render("delete from tasks where status = 'DONE' returning actor_id a_id"),
            "delete from tasks where status = 'DONE' returning actor_id as a_id"
        );
    }

    #[test]
    fn breaks_three_aliased_returning_items() {
        assert_eq!(
            render("delete from tasks where status = 'DONE' returning actor_id a_id, producer_id p_id, x y"),
            "delete from tasks\n\
             where status = 'DONE'\n\
             returning\n\
             \u{20}   actor_id as a_id,\n\
             \u{20}   producer_id as p_id,\n\
             \u{20}   x as y"
        );
    }

    #[test]
    fn inlines_simple_cte() {
        assert_eq!(render("with test as (select foo from bar) delete from films"), "with test as (select foo from bar) delete from films");
    }

    #[test]
    fn refuses_complex_cte() {
        let sql = "with recursive t(n) as (values (1) union all select n + 1 from t where n < 100) delete from films";
        assert_eq!(layout(sql, Dialect::Postgres), None);
    }

    #[test]
    fn formats_insert_select_query() {
        assert_eq!(
            render("insert into t (a, b, c, d) select a, b, c, d from s"),
            "insert into t (a, b, c, d)\n\
             select\n\
             \u{20}   a,\n\
             \u{20}   b,\n\
             \u{20}   c,\n\
             \u{20}   d\n\
             from s"
        );
    }

    #[test]
    fn splits_explain_header_and_formats_query() {
        assert_eq!(
            render("explain (analyze true, format json) select a, b, c, d from t"),
            "explain (analyze true, format json)\n\
             select\n\
             \u{20}   a,\n\
             \u{20}   b,\n\
             \u{20}   c,\n\
             \u{20}   d\n\
             from t"
        );
    }

    #[test]
    fn normalizes_boolean_option_values() {
        assert_eq!(
            render("explain (analyze on, verbose off, costs 1, generic_plan 0, format xml) select 1"),
            "explain (analyze true, verbose false, costs true, generic_plan false, format xml)\nselect 1"
        );
    }

    #[test]
    fn keeps_bare_options_and_enums() {
        assert_eq!(render("explain (analyze, serialize binary, memory) select 1"), "explain (analyze, serialize binary, memory)\nselect 1");
    }

    #[test]
    fn explains_delete_with_delete_layout() {
        assert_eq!(
            render("explain (analyze true) delete from t where a = 1 and b = 2"),
            "explain (analyze true)\ndelete from t\nwhere a = 1 and b = 2"
        );
    }

    #[test]
    fn explains_with_delete() {
        assert_eq!(
            render("explain with test as (select foo from bar) delete from films"),
            "explain\nwith test as (select foo from bar) delete from films"
        );
    }

    #[test]
    fn formats_with_query_after_explain() {
        assert_eq!(
            render("explain (analyze true) with x as (select a from t) select a from x"),
            "explain (analyze true)\n\
             with\n\
             x as (select a from t)\n\
             select a\n\
             from x"
        );
    }

    #[test]
    fn keeps_a_simple_update_on_one_line() {
        assert_eq!(render("update t set a = 1 where b = 2"), "update t set a = 1 where b = 2");
    }

    #[test]
    fn breaks_update_clauses_but_keeps_three_where_conditions_inline() {
        assert_eq!(
            render("update sys_permission set resource_path = '/x' where permission_code = 'p' and deleted = 0 and resource_path = '/y'"),
            "update sys_permission\n\
             set resource_path = '/x'\n\
             where permission_code = 'p' and deleted = 0 and resource_path = '/y'"
        );
    }

    #[test]
    fn breaks_four_update_assignments() {
        assert_eq!(
            render("update t set a = 1, b = 2, c = 3, d = 4 where id = 1"),
            "update t\n\
             set\n\
             \u{20}   a = 1,\n\
             \u{20}   b = 2,\n\
             \u{20}   c = 3,\n\
             \u{20}   d = 4\n\
             where id = 1"
        );
    }

    #[test]
    fn formats_with_update_and_cte() {
        assert_eq!(
            render("with ranked as (select id from t where status = 'RUNNING') update t set status = 'FAILED' where id in (select id from ranked)"),
            "with\n\
             ranked as (select id from t where status = 'RUNNING')\n\
             update t\n\
             set status = 'FAILED'\n\
             where id in (select id from ranked)"
        );
    }
}
