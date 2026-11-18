//! 深度感知切分：括号内的关键字、逗号、and/or 都不算切点。

/// 参与子句级判定的关键字；顺序决定同一位置上的最长匹配（`union all` 先于 `union`）。
pub const CLAUSE_KEYWORDS: [&str; 11] = [
    "select",
    "from",
    "where",
    "group by",
    "having",
    "order by",
    "limit",
    "union all",
    "union",
    "intersect",
    "except",
];

/// 集合运算；出现即不能整条内联。
pub const SET_OPERATORS: [&str; 4] = ["union all", "union", "intersect", "except"];

/// 断行标记；不计入元素数。
pub const SENTINELS: [&str; 2] = ["1 = 1", "1 = 0"];

/// 语句边界：空行后跟 select / with。
static STATEMENT_BREAK: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(r"(?m)\n\s*\n\s*(?:select|with)\b").expect("valid statement break")
});

/// 一个子句：关键字起点、正文半开区间。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Clause {
    pub kind: &'static str,
    pub keyword_start: usize,
    pub body_start: usize,
    pub body_end: usize,
}

/// 元素与它前面的连接词；首元素为 `None`。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Connector {
    None,
    Comma,
    And,
    Or,
}

/// 一个子句元素。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Element {
    pub text: String,
    pub connector: Connector,
}

/// 切出深度 0 的子句；正文包含关键字后的空白，直到下一个子句或文本结束。
pub fn clause_spans(text: &str) -> Vec<Clause> {
    let low = text.to_ascii_lowercase();
    let bytes = low.as_bytes();
    let mut marks: Vec<(usize, &'static str)> = Vec::new();
    let mut depth: i32 = 0;
    let mut index = 0;

    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'(' {
            depth += 1;
            index += 1;
            continue;
        }
        if byte == b')' {
            depth -= 1;
            index += 1;
            continue;
        }
        if depth == 0 && (index == 0 || !is_word(bytes[index - 1])) {
            let matched = CLAUSE_KEYWORDS
                .iter()
                .copied()
                .find(|kind| low.get(index..).is_some_and(|rest| rest.starts_with(kind)));
            if let Some(kind) = matched {
                marks.push((index, kind));
                index += kind.len();
                continue;
            }
        }
        index += 1;
    }

    let mut clauses = Vec::new();
    for (order, (position, kind)) in marks.iter().enumerate() {
        let body_start = position + kind.len();
        let body_end = marks.get(order + 1).map_or(text.len(), |(next, _)| *next);
        clauses.push(Clause {
            kind,
            keyword_start: *position,
            body_start,
            body_end,
        });
    }
    clauses
}

/// 切出子句元素：where / having 按布尔连接词，其余按深度 0 逗号；去掉开头的断行哨兵。
pub fn split_elements(kind: &str, body: &str) -> Vec<Element> {
    if kind == "where" || kind == "having" {
        let mut items = split_conditions(body);
        if items
            .first()
            .is_some_and(|element| SENTINELS.contains(&element.text.as_str()))
        {
            items.remove(0);
        }
        items
    } else {
        split_depth(body)
    }
}

/// 正文在括号外是否换行——即该子句是否已经逐元素展开。
pub fn has_top_level_newline(body: &str) -> bool {
    let mut depth: i32 = 0;
    for byte in body.bytes() {
        match byte {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b'\n' if depth == 0 => return true,
            _ => {}
        }
    }
    false
}

/// 根语句 + 任意深度的括号内子语句；根在前。
pub fn all_statements(text: &str) -> Vec<crate::span::Span> {
    let mut blocks = vec![crate::span::Span::new(0, text.len())];
    let mut starts: Vec<usize> = Vec::new();
    for (index, byte) in text.bytes().enumerate() {
        if byte == b'(' {
            starts.push(index);
        } else if byte == b')' {
            let Some(start) = starts.pop() else {
                continue;
            };
            let inner = &text[start + 1..index];
            if starts_with_statement(inner) {
                let leading = inner.len() - inner.trim_start().len();
                blocks.push(crate::span::Span::new(start + 1 + leading, index));
            }
        }
    }
    blocks
}

/// 按深度 0 的分号或「空行 + select/with」切出多条语句。
pub fn statements(text: &str) -> Vec<crate::span::Span> {
    let bytes = text.as_bytes();
    let mut bounds: Vec<(usize, usize)> = Vec::new();
    let mut depth: i32 = 0;
    let mut start = 0;
    let mut index = 0;

    while index < bytes.len() {
        match bytes[index] {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b';' if depth == 0 => {
                bounds.push((start, index));
                start = index + 1;
            }
            b'\n' if depth == 0 && STATEMENT_BREAK.is_match(&text[index..]) => {
                bounds.push((start, index));
                start = index + 1;
            }
            _ => {}
        }
        index += 1;
    }
    bounds.push((start, text.len()));

    bounds
        .into_iter()
        .filter_map(|(begin, end)| {
            let chunk = &text[begin..end];
            let trimmed = chunk.trim();
            if trimmed.is_empty() {
                return None;
            }
            let leading = chunk.len() - chunk.trim_start().len();
            Some(crate::span::Span::new(
                begin + leading,
                begin + leading + trimmed.len(),
            ))
        })
        .collect()
}

fn split_depth(body: &str) -> Vec<Element> {
    let mut elements: Vec<Element> = Vec::new();
    let mut depth: i32 = 0;
    let mut start = 0;
    for (index, byte) in body.bytes().enumerate() {
        match byte {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b',' if depth == 0 => {
                push_element(&mut elements, &body[start..index], Connector::Comma);
                start = index + 1;
            }
            _ => {}
        }
    }
    push_element(&mut elements, &body[start..], Connector::Comma);
    elements
}

fn push_element(elements: &mut Vec<Element>, piece: &str, connector: Connector) {
    let text = piece.trim();
    if text.is_empty() {
        return;
    }
    let connector = if elements.is_empty() {
        Connector::None
    } else {
        connector
    };
    elements.push(Element {
        text: text.to_owned(),
        connector,
    });
}

fn split_conditions(body: &str) -> Vec<Element> {
    let low = body.to_ascii_lowercase();
    let bytes = body.as_bytes();
    let mut pieces: Vec<Element> = Vec::new();
    let mut depth: i32 = 0;
    let mut start = 0;
    let mut index = 0;
    let mut pending = Connector::None;

    while index < bytes.len() {
        match bytes[index] {
            b'(' => depth += 1,
            b')' => depth -= 1,
            _ => {}
        }
        if depth == 0 {
            let mut matched = false;
            for (word, connector) in [("and", Connector::And), ("or", Connector::Or)] {
                let boundary = (index == 0 || !is_word(bytes[index - 1]))
                    && (index + word.len() >= bytes.len() || !is_word(bytes[index + word.len()]));
                if boundary && low.get(index..).is_some_and(|rest| rest.starts_with(word)) {
                    push_element(&mut pieces, &body[start..index], pending);
                    pending = connector;
                    index += word.len();
                    start = index;
                    matched = true;
                    break;
                }
            }
            if matched {
                continue;
            }
        }
        index += 1;
    }
    push_element(&mut pieces, &body[start..], pending);
    merge_between(pieces)
}

fn merge_between(pieces: Vec<Element>) -> Vec<Element> {
    let mut merged: Vec<Element> = Vec::new();
    for element in pieces {
        if let Some(last) = merged.last_mut() {
            let lower = last.text.to_ascii_lowercase();
            if lower.matches("between").count() > lower.matches(" and ").count() {
                last.text = format!("{} and {}", last.text, element.text);
                continue;
            }
        }
        merged.push(element);
    }
    merged
}

fn starts_with_statement(text: &str) -> bool {
    let trimmed = text.trim_start().to_ascii_lowercase();
    trimmed.starts_with("select") || trimmed.starts_with("with")
}

fn is_word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

#[cfg(test)]
mod tests {
    #[test]
    fn splits_clause_bodies() {
        let clauses = crate::sql::splitter::clause_spans("select a from t where x = 1");
        assert_eq!(clauses.len(), 3);
        assert_eq!(clauses[0].kind, "select");
        assert_eq!(
            &"select a from t where x = 1"[clauses[0].body_start..clauses[0].body_end],
            " a "
        );
    }

    #[test]
    fn ignores_keywords_inside_parentheses() {
        let clauses = crate::sql::splitter::clause_spans("select a from (select b from t) x");
        assert_eq!(clauses.len(), 2);
        assert_eq!(
            clauses
                .iter()
                .filter(|clause| clause.kind == "select")
                .count(),
            1
        );
    }

    #[test]
    fn splits_conditions_and_keeps_connectors() {
        let elements = crate::sql::splitter::split_elements("where", " 1 = 1 and a = 1 or b = 2 ");
        assert_eq!(elements.len(), 2);
        assert_eq!(elements[0].text, "a = 1");
        assert_eq!(elements[1].connector, crate::sql::splitter::Connector::Or);
    }

    #[test]
    fn merges_between_and() {
        let elements = crate::sql::splitter::split_elements("where", "x between 1 and 3 and y = 1");
        assert_eq!(elements.len(), 2);
        assert_eq!(elements[0].text, "x between 1 and 3");
    }

    #[test]
    fn detects_top_level_newlines() {
        assert!(crate::sql::splitter::has_top_level_newline(" a,\n b "));
        assert!(!crate::sql::splitter::has_top_level_newline(" (a,\n b) "));
    }

    #[test]
    fn finds_nested_statements() {
        let blocks = crate::sql::splitter::all_statements("select a from (select b from t) x");
        assert_eq!(blocks.len(), 2);
        assert_eq!(
            &"select a from (select b from t) x"[blocks[1].start..blocks[1].end],
            "select b from t"
        );
    }

    #[test]
    fn splits_statements_on_semicolon() {
        let spans = crate::sql::splitter::statements("select 1; select 2");
        assert_eq!(spans.len(), 2);
    }
}
