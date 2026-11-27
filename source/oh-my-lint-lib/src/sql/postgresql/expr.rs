//! 元素形态与断行判定：把 `complex` / `simple` / `decide` 固化成函数。

use crate::sql::core::keywords::is_function_name;

static ALIAS: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| regex::Regex::new(r#"(?i)\bas\s+(\w+|"[^"]+"|`[^`]+`)\s*$"#).expect("alias"));

/// 元素是否复杂：子查询、case、窗口函数、条件里的复杂函数、任意布尔组、超长 in 列表。
pub fn is_complex(element: &str, kind: &str) -> bool {
    has_word(element, "case")
        || has_subquery(element)
        || (!matches!(kind, "select" | "order by") && has_complex_function_call(element))
        || has_oversized_in_list(element)
        || (is_condition_context(kind) && has_boolean_group(element))
}

/// 条件上下文才把布尔组视为复杂；select / order by 的算术括号不参与。
fn is_condition_context(kind: &str) -> bool {
    matches!(kind, "where" | "having" | "on" | "")
}

/// 任意布尔组；嵌套组和外层组都算，父级据此退出简单判定。
fn has_boolean_group(text: &str) -> bool {
    !crate::sql::splitter::boolean_group_spans(text).is_empty()
}

/// 是否以 `as <标识符>` 结尾。
pub fn has_alias(element: &str) -> bool {
    ALIAS.is_match(element.trim())
}

/// 是否派生表：以 `(select` / `(with` 开头。
pub fn is_derived(element: &str) -> bool {
    let tokens = crate::sql::lexer::tokens(element);
    let mut probe = tokens.iter();
    let Some(open) = probe.next() else {
        return false;
    };
    if open.kind != crate::sql::lexer::Kind::Punct || open.text != "(" {
        return false;
    }
    let Some(word) = probe.find(|token| token.kind != crate::sql::lexer::Kind::Whitespace) else {
        return false;
    };
    word.kind == crate::sql::lexer::Kind::Word && (word.text.eq_ignore_ascii_case("select") || word.text.eq_ignore_ascii_case("with") || word.text.eq_ignore_ascii_case("values"))
}

/// 深度 0 是否含 `join`。
pub fn has_join(element: &str) -> bool {
    let mut depth: i32 = 0;
    for token in crate::sql::lexer::tokens(element) {
        match token.kind {
            crate::sql::lexer::Kind::Punct if token.text == "(" => depth += 1,
            crate::sql::lexer::Kind::Punct if token.text == ")" => depth -= 1,
            crate::sql::lexer::Kind::Word if depth == 0 && token.text.eq_ignore_ascii_case("join") => {
                return true;
            }
            _ => {}
        }
    }
    false
}

/// 子句是否必须一元素一行；实现由 DQL 子句模块提供。
pub fn needs_break(kind: &str, elements: &[crate::sql::splitter::Element]) -> bool {
    crate::sql::postgresql::dql::clause::needs_break(kind, elements)
}

/// 简单语句块：select ≤3、其他子句 ≤1、无 with/集合运算、无嵌套查询、无条件布尔组。
pub fn is_simple(text: &str) -> bool {
    if starts_with_word(text, "with") {
        return false;
    }
    if crate::sql::splitter::all_statements(text).into_iter().nth(1).is_some() {
        return false;
    }
    let clauses = crate::sql::splitter::clause_spans(text);
    if clauses.iter().any(|clause| crate::sql::splitter::SET_OPERATORS.contains(&clause.kind)) {
        return false;
    }
    if clauses.iter().filter(|clause| clause.kind == "select").count() != 1 {
        return false;
    }
    for clause in &clauses {
        let body = &text[clause.body_start..clause.body_end];
        if matches!(clause.kind, "where" | "having" | "from") && has_boolean_group(body) {
            return false;
        }
        let condition = matches!(clause.kind, "where" | "having").then(|| crate::sql::postgresql::dql::r#where::parse(body));
        let elements = condition
            .as_ref()
            .map_or_else(|| crate::sql::splitter::split_elements(clause.kind, body), |condition| condition.elements.clone());
        let limit = if clause.kind == "select" { 3 } else { 1 };
        if elements.len() > limit {
            return false;
        }
        if !elements.is_empty() && needs_break(clause.kind, &elements) {
            return false;
        }
    }
    true
}

/// 条件连接词转成文本；首元素为 `and`，保持 `where 1 = 1` 的语料形态。
pub fn connector_word(connector: crate::sql::splitter::Connector) -> &'static str {
    crate::sql::postgresql::dql::clause::connector_word(connector)
}

/// `case` 分支是否复杂：只按分支本身判定，不把 `case` 关键字算进去。
pub fn branches_complex(text: &str) -> bool {
    has_subquery(text) || has_complex_function_call(text) || has_oversized_in_list(text) || !crate::sql::splitter::boolean_group_spans(text).is_empty()
}

/// 文本是否含某个独立单词（字符串与注释被整体跳过）。
fn has_word(text: &str, word: &str) -> bool {
    crate::sql::lexer::tokens(text)
        .iter()
        .any(|token| token.kind == crate::sql::lexer::Kind::Word && token.text.eq_ignore_ascii_case(word))
}

/// 文本是否含子查询：`(` 之后（忽略空白）是 `select` / `with`。
pub(crate) fn has_subquery(text: &str) -> bool {
    let tokens = crate::sql::lexer::tokens(text);
    for (index, token) in tokens.iter().enumerate() {
        if token.kind != crate::sql::lexer::Kind::Punct || token.text != "(" {
            continue;
        }
        for next in &tokens[index + 1..] {
            if next.kind == crate::sql::lexer::Kind::Whitespace {
                continue;
            }
            if next.kind == crate::sql::lexer::Kind::Word && (next.text.eq_ignore_ascii_case("select") || next.text.eq_ignore_ascii_case("with")) {
                return true;
            }
            break;
        }
    }
    false
}

/// 文本是否含复杂函数调用：简单聚合与 `cast` 之外都算复杂。
fn has_complex_function_call(text: &str) -> bool {
    let tokens = crate::sql::lexer::tokens(text);
    let mut index = 0;
    while index < tokens.len() {
        let token = tokens[index];
        if token.kind != crate::sql::lexer::Kind::Punct || token.text != "(" {
            index += 1;
            continue;
        }
        let mut probe = index;
        while probe > 0 && tokens[probe - 1].kind == crate::sql::lexer::Kind::Whitespace {
            probe -= 1;
        }
        if probe == 0 {
            index += 1;
            continue;
        }
        let name = tokens[probe - 1];
        let is_name = match name.kind {
            crate::sql::lexer::Kind::Word => is_function_name(name.text),
            crate::sql::lexer::Kind::QuotedIdent => true,
            _ => false,
        };
        if !is_name {
            index += 1;
            continue;
        }
        let mut depth = 0;
        let mut close = None;
        for (offset, next) in tokens[index..].iter().enumerate() {
            if next.kind == crate::sql::lexer::Kind::Punct && next.text == "(" {
                depth += 1;
            } else if next.kind == crate::sql::lexer::Kind::Punct && next.text == ")" {
                depth -= 1;
                if depth == 0 {
                    close = Some(index + offset);
                    break;
                }
            }
        }
        let Some(close) = close else {
            index += 1;
            continue;
        };
        let call = tokens[probe - 1..=close].to_vec();
        if !simple_call(&call) {
            return true;
        }
        index = close + 1;
    }
    false
}

/// 简单调用：`count(*)`、单字段聚合、`cast(单字段 as 类型)`。
fn simple_call(call: &[crate::sql::lexer::Token<'_>]) -> bool {
    let call: Vec<crate::sql::lexer::Token<'_>> = call.iter().copied().filter(|token| token.kind != crate::sql::lexer::Kind::Whitespace).collect();
    if call.len() < 3 || call[1].text != "(" || call[call.len() - 1].text != ")" {
        return false;
    }
    let name = call[0].text.to_ascii_lowercase();
    let inner = &call[2..call.len() - 1];
    if name == "cast" {
        let mut depth = 0;
        for (index, token) in inner.iter().enumerate() {
            match token.kind {
                crate::sql::lexer::Kind::Punct if token.text == "(" => depth += 1,
                crate::sql::lexer::Kind::Punct if token.text == ")" => depth -= 1,
                crate::sql::lexer::Kind::Word if depth == 0 && token.text.eq_ignore_ascii_case("as") => {
                    return simple_operand(&inner[..index]) && !inner[index + 1..].is_empty();
                }
                _ => {}
            }
        }
        return false;
    }
    if matches!(name.as_str(), "current_date" | "current_timestamp") {
        return inner.is_empty();
    }
    if matches!(name.as_str(), "coalesce" | "nullif") {
        return split_arguments(inner).iter().all(|argument| !argument.is_empty() && simple_operand(argument));
    }
    if AGGREGATES.contains(&name.as_str()) {
        let body = if inner
            .first()
            .is_some_and(|token| token.kind == crate::sql::lexer::Kind::Word && token.text.eq_ignore_ascii_case("distinct"))
        {
            &inner[1..]
        } else {
            inner
        };
        return simple_operand(body);
    }
    false
}

/// 单列聚合：参数足够简单，按普通表达式处理。
const AGGREGATES: [&str; 11] = ["count", "sum", "avg", "min", "max", "stddev", "stddev_pop", "stddev_samp", "variance", "var_pop", "var_samp"];

/// 按深度 0 的逗号拆分参数。
fn split_arguments<'a>(tokens: &[crate::sql::lexer::Token<'a>]) -> Vec<Vec<crate::sql::lexer::Token<'a>>> {
    let mut arguments: Vec<Vec<crate::sql::lexer::Token<'a>>> = vec![Vec::new()];
    let mut depth = 0;
    for token in tokens {
        match token.kind {
            crate::sql::lexer::Kind::Punct if token.text == "(" => depth += 1,
            crate::sql::lexer::Kind::Punct if token.text == ")" => depth -= 1,
            crate::sql::lexer::Kind::Punct if token.text == "," && depth == 0 => {
                arguments.push(Vec::new());
                continue;
            }
            _ => {}
        }
        arguments.last_mut().expect("arguments is non-empty").push(*token);
    }
    arguments
}

/// 单个字段、`*`、字面量，或简单聚合/`cast`；允许前置一元正负号。
fn simple_operand(tokens: &[crate::sql::lexer::Token<'_>]) -> bool {
    let tokens: Vec<crate::sql::lexer::Token<'_>> = tokens.iter().copied().filter(|token| token.kind != crate::sql::lexer::Kind::Whitespace).collect();
    let mut index = 0;
    if tokens.first().is_some_and(|token| token.kind == crate::sql::lexer::Kind::Punct && matches!(token.text, "-" | "+")) {
        index = 1;
    }
    let slice = &tokens[index..];
    if slice.is_empty() {
        return false;
    }
    if slice.len() == 1 {
        return match slice[0].kind {
            crate::sql::lexer::Kind::Number | crate::sql::lexer::Kind::String | crate::sql::lexer::Kind::Word | crate::sql::lexer::Kind::QuotedIdent => true,
            crate::sql::lexer::Kind::Punct => slice[0].text == "*",
            _ => false,
        };
    }
    let field = slice.len() % 2 == 1
        && slice
            .iter()
            .step_by(2)
            .all(|token| matches!(token.kind, crate::sql::lexer::Kind::Word | crate::sql::lexer::Kind::QuotedIdent))
        && slice.iter().skip(1).step_by(2).all(|token| token.kind == crate::sql::lexer::Kind::Punct && token.text == ".");
    if field {
        return true;
    }
    matches!(slice[0].kind, crate::sql::lexer::Kind::Word | crate::sql::lexer::Kind::QuotedIdent) && slice.get(1).is_some_and(|token| token.text == "(") && simple_call(slice)
}

/// 函数实参内是否含子查询。
pub(crate) fn function_contains_subquery(text: &str) -> bool {
    let tokens = crate::sql::lexer::tokens(text);
    for (index, token) in tokens.iter().enumerate() {
        if token.kind != crate::sql::lexer::Kind::Punct || token.text != "(" {
            continue;
        }
        let mut probe = index;
        while probe > 0 && tokens[probe - 1].kind == crate::sql::lexer::Kind::Whitespace {
            probe -= 1;
        }
        if probe == 0 {
            continue;
        }
        let name = tokens[probe - 1];
        let is_name = match name.kind {
            crate::sql::lexer::Kind::Word => is_function_name(name.text),
            crate::sql::lexer::Kind::QuotedIdent => true,
            _ => false,
        };
        if !is_name || (name.kind == crate::sql::lexer::Kind::Word && name.text.eq_ignore_ascii_case("over")) {
            continue;
        }
        let mut depth = 0;
        let mut close = None;
        for next in &tokens[index..] {
            if next.kind == crate::sql::lexer::Kind::Punct && next.text == "(" {
                depth += 1;
            } else if next.kind == crate::sql::lexer::Kind::Punct && next.text == ")" {
                depth -= 1;
                if depth == 0 {
                    close = Some(next.start);
                    break;
                }
            }
        }
        let Some(close) = close else {
            continue;
        };
        if has_subquery(&text[token.start + 1..close]) {
            return true;
        }
    }
    false
}

/// `in (…)` 的常量列表是否超过 3 项。
fn has_oversized_in_list(text: &str) -> bool {
    let tokens = crate::sql::lexer::tokens(text);
    for (index, token) in tokens.iter().enumerate() {
        if token.kind != crate::sql::lexer::Kind::Word || !token.text.eq_ignore_ascii_case("in") {
            continue;
        }
        let mut probe = index + 1;
        while probe < tokens.len() && tokens[probe].kind == crate::sql::lexer::Kind::Whitespace {
            probe += 1;
        }
        if probe >= tokens.len() || tokens[probe].text != "(" {
            continue;
        }
        let mut depth = 0;
        let mut commas = 0;
        let mut items = 0;
        let mut is_subquery = false;
        for next in &tokens[probe..] {
            match next.kind {
                crate::sql::lexer::Kind::Punct if next.text == "(" => depth += 1,
                crate::sql::lexer::Kind::Punct if next.text == ")" => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                crate::sql::lexer::Kind::Punct if next.text == "," && depth == 1 => commas += 1,
                crate::sql::lexer::Kind::Word if depth == 1 && (next.text.eq_ignore_ascii_case("select") || next.text.eq_ignore_ascii_case("with")) => {
                    is_subquery = true;
                }
                crate::sql::lexer::Kind::Whitespace => {}
                _ if depth == 1 => items += 1,
                _ => {}
            }
        }
        if is_subquery || items == 0 {
            continue;
        }
        if commas + 1 > 3 {
            return true;
        }
    }
    false
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
    fn elements(texts: &[&str]) -> Vec<crate::sql::splitter::Element> {
        texts
            .iter()
            .enumerate()
            .map(|(index, text)| crate::sql::splitter::Element {
                text: (*text).to_owned(),
                connector: if index == 0 {
                    crate::sql::splitter::Connector::None
                } else {
                    crate::sql::splitter::Connector::Comma
                },
            })
            .collect()
    }

    #[test]
    fn select_thresholds() {
        assert!(!crate::sql::decision::needs_break("select", &elements(&["a", "b"])));
        assert!(!crate::sql::decision::needs_break("select", &elements(&["a", "b", "c"])));
        assert!(crate::sql::decision::needs_break("select", &elements(&["a as x", "b as y", "c as z"])));
        assert!(crate::sql::decision::needs_break("select", &elements(&["a", "b", "c", "d"])));
        assert!(!crate::sql::decision::needs_break("select", &elements(&["a", "sum(b)"])));
        assert!(!crate::sql::decision::needs_break("select", &elements(&["a", "count(*)"])));
        assert!(!crate::sql::decision::needs_break("select", &elements(&["a", "cast(b as int)"])));
        assert!(!crate::sql::decision::needs_break("select", &elements(&["a", "substring(b, 1, 2)"])));
        assert!(!crate::sql::decision::needs_break("select", &elements(&["a", "sum(b * c)"])));
        assert!(!crate::sql::decision::needs_break("select", &elements(&["a", "rank() over(order by b)"])));
    }

    #[test]
    fn where_thresholds() {
        assert!(!crate::sql::decision::needs_break("where", &elements(&["a = 1", "b = 2", "c = 3"])));
        assert!(crate::sql::decision::needs_break("where", &elements(&["a = 1", "b = 2", "c = 3", "d = 4"])));
        assert!(crate::sql::decision::needs_break("where", &elements(&["a = 1", "substring(b, 1, 2) = 'x'"])));
        assert!(crate::sql::decision::needs_break("where", &elements(&["a = 1", "b in (1, 2, 3, 4)"])));
        assert!(!crate::sql::decision::needs_break("where", &elements(&["a = 1", "b in (1, 2, 3)"])));
    }

    #[test]
    fn detects_aliases() {
        assert!(crate::sql::decision::has_alias("a as x"));
        assert!(crate::sql::decision::has_alias("a as \"x\""));
        assert!(!crate::sql::decision::has_alias("a x"));
    }

    #[test]
    fn simple_statement() {
        assert!(crate::sql::decision::is_simple("select a, b from t"));
        assert!(!crate::sql::decision::is_simple("select a, b from t union all select c, d from u"));
        assert!(!crate::sql::decision::is_simple("select a, b, c, d from t"));
        assert!(!crate::sql::decision::is_simple("select a from t where x = 1 and y = 2"));
        assert!(crate::sql::decision::is_simple("select count(*) from t where a = 1"));
        assert!(!crate::sql::decision::is_simple("select a from t1, t2"));
        assert!(!crate::sql::decision::is_simple("with x as (select a from t) select a from x"));
        assert!(!crate::sql::decision::is_simple("select a from t where (a = 1 or b = 2)"));
        assert!(!crate::sql::decision::is_simple("select a from t where x < (select y from u)"));
        assert!(crate::sql::decision::is_simple("select 0.2 * avg(l_quantity) from lineitem where l_partkey = p_partkey"));
    }

    #[test]
    fn boolean_groups_are_complex_in_conditions() {
        assert!(crate::sql::decision::is_complex("(a = 1 or b = 2)", "where"));
        assert!(crate::sql::decision::is_complex("(a = 1 and b = 2) or (c = 3 and d = 4)", "where"));
        assert!(!crate::sql::decision::is_complex("(a + b)", "select"));
    }

    #[test]
    fn sentinels_do_not_count() {
        let conditions = crate::sql::splitter::split_elements("where", " 1 = 1 and a = 1 and b = 2 and c = 3 ");
        assert_eq!(conditions.len(), 3);
        assert!(!crate::sql::decision::needs_break("where", &conditions));
    }

    #[test]
    fn whitelisted_functions_are_simple() {
        assert!(!crate::sql::decision::is_complex("coalesce(a, 0)", "where"));
        assert!(!crate::sql::decision::is_complex("nullif(a, 0)", "where"));
        assert!(!crate::sql::decision::is_complex("current_date()", "where"));
        assert!(!crate::sql::decision::is_complex("current_timestamp()", "where"));
        assert!(!crate::sql::decision::is_complex("stddev(a)", "where"));
        assert!(crate::sql::decision::is_complex("coalesce(substring(x, 1, 2), '')", "where"));
        assert!(crate::sql::decision::is_complex("greatest(a, b)", "where"));
        assert!(crate::sql::decision::is_complex("least(a, b)", "where"));
    }

    #[test]
    fn strings_do_not_fake_complexity() {
        assert!(!crate::sql::decision::is_complex("'a and b (c)' as x", "select"));
        assert!(!crate::sql::decision::is_complex("lower(a)", "select"));
        assert!(crate::sql::decision::is_complex("lower(a)", "where"));
        assert!(!crate::sql::decision::is_complex("count(*)", "select"));
        assert!(!crate::sql::decision::is_complex("cast(a as decimal(15, 4))", "select"));
        assert!(!crate::sql::decision::is_complex("sum(a * b)", "select"));
        assert!(crate::sql::decision::is_complex("sum(a * b)", "where"));
        assert!(crate::sql::decision::is_complex("concat(a, b)", "where"));
        assert!(!crate::sql::decision::is_complex("concat(a, b)", "select"));
        assert!(!crate::sql::decision::is_complex("rank() over(order by b)", "select"));
    }
}
