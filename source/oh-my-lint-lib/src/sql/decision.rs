//! 元素形态与断行判定：把 `complex` / `simple` / `decide` 固化成函数。

const STATEMENT_KINDS: [&str; 7] = [
    "select", "from", "where", "group by", "having", "order by", "limit",
];

static CASE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"(?i)\bcase\b").expect("case"));
static SUBQUERY: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"(?i)\(\s*select\b").expect("subquery"));
static OVER: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"(?i)\bover\s*\(").expect("over"));
static ALIAS: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(r#"(?i)\bas\s+(\w+|"[^"]+"|`[^`]+`)\s*$"#).expect("alias")
});
static DERIVED: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(r"(?i)^\s*\(\s*(select|with)\b").expect("derived")
});
static AND_OR: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"(?i)\b(and|or)\b").expect("and or"));

/// 元素是否复杂：case、子查询、select/order by 里的窗口、或自身会断行的布尔组。
pub fn is_complex(element: &str, kind: &str) -> bool {
    if CASE.is_match(element) || SUBQUERY.is_match(element) {
        return true;
    }
    if matches!(kind, "select" | "order by") && OVER.is_match(element) {
        return true;
    }
    if matches!(kind, "where" | "having") {
        for group in boolean_groups(element) {
            let conditions = crate::sql::splitter::split_elements("where", &group);
            if !conditions.is_empty() && needs_break("where", &conditions) {
                return true;
            }
        }
    }
    false
}

/// 是否以 `as <标识符>` 结尾。
pub fn has_alias(element: &str) -> bool {
    ALIAS.is_match(element.trim())
}

/// 是否派生表：以 `(select` / `(with` 开头。
pub fn is_derived(element: &str) -> bool {
    DERIVED.is_match(element.trim())
}

/// 深度 0 是否含 `join`。
pub fn has_join(element: &str) -> bool {
    let low = element.to_ascii_lowercase();
    let bytes = low.as_bytes();
    let mut depth: i32 = 0;
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'(' => depth += 1,
            b')' => depth -= 1,
            _ => {}
        }
        if depth == 0
            && (index == 0 || !is_word(bytes[index - 1]))
            && low
                .get(index..)
                .is_some_and(|rest| rest.starts_with("join"))
            && (index + 4 >= bytes.len() || !is_word(bytes[index + 4]))
        {
            return true;
        }
        index += 1;
    }
    false
}

/// 深度 0 的布尔括号组正文；紧跟函数名 / 限定名后的括号（`over(...)`、`sum(...)`）不算组。
pub fn boolean_groups(element: &str) -> Vec<String> {
    let mut groups = Vec::new();
    let mut depth: i32 = 0;
    let mut starts: Vec<usize> = Vec::new();
    let bytes = element.as_bytes();
    for (index, byte) in bytes.iter().enumerate() {
        match *byte {
            b'(' => {
                let previous = index.checked_sub(1).map(|position| bytes[position]);
                if depth == 0
                    && previous.is_none_or(|byte| !is_word(byte) && byte != b'.' && byte != b']')
                {
                    starts.push(index);
                }
                depth += 1;
            }
            b')' => {
                depth -= 1;
                if depth == 0
                    && let Some(start) = starts.pop()
                {
                    let inner = &element[start + 1..index];
                    if AND_OR.is_match(inner) {
                        groups.push(inner.to_owned());
                    }
                }
            }
            _ => {}
        }
    }
    groups
}

/// 子句是否必须一元素一行。
pub fn needs_break(kind: &str, elements: &[crate::sql::splitter::Element]) -> bool {
    let count = elements.len();
    if kind == "from" {
        if count >= 4 || elements.iter().any(|element| has_join(&element.text)) {
            return true;
        }
        return count >= 2 && elements.iter().any(|element| is_derived(&element.text));
    }
    if count <= 1 {
        return false;
    }
    let complex = elements
        .iter()
        .any(|element| is_complex(&element.text, kind));
    if kind == "select" {
        return count >= 4
            || complex
            || (count == 3 && elements.iter().all(|element| has_alias(&element.text)));
    }
    count >= 4 || complex
}

/// 语句 / 子查询能否整行写完：子句 <=3、无集合运算、每个子句自身都不断行。
pub fn is_simple(text: &str) -> bool {
    let clauses = crate::sql::splitter::clause_spans(text);
    let kinds: Vec<&str> = clauses.iter().map(|clause| clause.kind).collect();
    if kinds.iter().filter(|kind| **kind == "select").count() != 1 {
        return false;
    }
    if kinds
        .iter()
        .any(|kind| crate::sql::splitter::SET_OPERATORS.contains(kind))
    {
        return false;
    }
    if kinds
        .iter()
        .filter(|kind| STATEMENT_KINDS.contains(kind))
        .count()
        > 3
    {
        return false;
    }
    for clause in &clauses {
        let body = &text[clause.body_start..clause.body_end];
        let elements = crate::sql::splitter::split_elements(clause.kind, body);
        if !elements.is_empty() && needs_break(clause.kind, &elements) {
            return false;
        }
    }
    true
}

/// 条件连接词转成文本；首元素为 `and`，保持 `where 1 = 1` 的语料形态。
pub fn connector_word(connector: crate::sql::splitter::Connector) -> &'static str {
    match connector {
        crate::sql::splitter::Connector::Or => "or",
        _ => "and",
    }
}

fn is_word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
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
        assert!(!crate::sql::decision::needs_break(
            "select",
            &elements(&["a", "b"])
        ));
        assert!(!crate::sql::decision::needs_break(
            "select",
            &elements(&["a", "b", "c"])
        ));
        assert!(crate::sql::decision::needs_break(
            "select",
            &elements(&["a as x", "b as y", "c as z"])
        ));
        assert!(crate::sql::decision::needs_break(
            "select",
            &elements(&["a", "b", "c", "d"])
        ));
    }

    #[test]
    fn where_thresholds() {
        assert!(!crate::sql::decision::needs_break(
            "where",
            &elements(&["a = 1", "b = 2", "c = 3"])
        ));
        assert!(crate::sql::decision::needs_break(
            "where",
            &elements(&["a = 1", "b = 2", "c = 3", "d = 4"])
        ));
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
        assert!(!crate::sql::decision::is_simple(
            "select a, b from t union all select c, d from u"
        ));
        assert!(!crate::sql::decision::is_simple("select a, b, c, d from t"));
    }

    #[test]
    fn sentinels_do_not_count() {
        let conditions =
            crate::sql::splitter::split_elements("where", " 1 = 1 and a = 1 and b = 2 and c = 3 ");
        assert_eq!(conditions.len(), 3);
        assert!(!crate::sql::decision::needs_break("where", &conditions));
    }
}
