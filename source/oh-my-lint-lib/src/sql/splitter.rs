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
    let masked = crate::sql::lexer::mask(text);
    let low = masked.to_ascii_lowercase();
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
    for token in crate::sql::lexer::tokens(body) {
        match token.kind {
            crate::sql::lexer::Kind::Punct if token.text == "(" => depth += 1,
            crate::sql::lexer::Kind::Punct if token.text == ")" => depth -= 1,
            crate::sql::lexer::Kind::Whitespace if depth == 0 && token.text.contains('\n') => {
                return true;
            }
            _ => {}
        }
    }
    false
}

/// 布尔组：括号内正文的字节区间。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Group {
    pub start: usize,
    pub end: usize,
}

/// 找出全部布尔组（含嵌套）；跳过子查询、函数、限定名与 `case` 括号。
pub fn boolean_group_spans(text: &str) -> Vec<Group> {
    let masked = crate::sql::lexer::mask(text);
    let bytes = masked.as_bytes();
    let mut stack: Vec<usize> = Vec::new();
    let mut pairs: Vec<(usize, usize)> = Vec::new();
    for (index, byte) in bytes.iter().enumerate() {
        match *byte {
            b'(' => stack.push(index),
            b')' => {
                if let Some(open) = stack.pop() {
                    pairs.push((open, index));
                }
            }
            _ => {}
        }
    }
    let mut groups = Vec::new();
    for (open, close) in pairs {
        let inner = &masked[open + 1..close];
        let trimmed = inner.trim_start();
        if starts_with_word(trimmed, "select")
            || starts_with_word(trimmed, "with")
            || starts_with_word(trimmed, "case")
        {
            continue;
        }
        if !is_group_prefix(&masked, open) {
            continue;
        }
        if split_elements("where", &text[open + 1..close]).len() >= 2 {
            groups.push(Group {
                start: open + 1,
                end: close,
            });
        }
    }
    groups.sort_by_key(|group| group.start);
    groups
}

/// 括号前是否处在“条件组”位置：紧邻控制关键字、逗号、运算符或行首。
fn is_group_prefix(masked: &str, open: usize) -> bool {
    let bytes = masked.as_bytes();
    let mut index = open;
    while index > 0 && bytes[index - 1].is_ascii_whitespace() {
        index -= 1;
    }
    let Some(last) = index.checked_sub(1) else {
        return true;
    };
    match bytes[last] {
        byte if byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$' => {
            let mut start = last;
            while start > 0 {
                let previous = bytes[start - 1];
                if previous.is_ascii_alphanumeric() || previous == b'_' || previous == b'$' {
                    start -= 1;
                } else {
                    break;
                }
            }
            matches!(
                masked[start..last + 1].to_ascii_lowercase().as_str(),
                "and" | "or" | "not" | "where" | "having" | "on" | "when" | "then" | "else"
            )
        }
        b'.' | b']' | b')' => false,
        _ => true,
    }
}

/// 文本去掉前导空白后是否以某个词开头（按词边界判断）。
fn starts_with_word(text: &str, word: &str) -> bool {
    let Some(rest) = text.strip_prefix(word) else {
        return false;
    };
    rest.chars().next().is_none_or(|character| {
        !(character.is_alphanumeric() || character == '_' || character == '$')
    })
}
pub fn all_statements(text: &str) -> Vec<crate::span::Span> {
    let mut blocks = vec![crate::span::Span::new(0, text.len())];
    let masked = crate::sql::lexer::mask(text);
    let mut starts: Vec<usize> = Vec::new();
    for (index, byte) in masked.bytes().enumerate() {
        if byte == b'(' {
            starts.push(index);
        } else if byte == b')' {
            let Some(start) = starts.pop() else {
                continue;
            };
            if starts_with_statement(&masked[start + 1..index]) {
                let inner = &text[start + 1..index];
                let leading = inner.len() - inner.trim_start().len();
                blocks.push(crate::span::Span::new(start + 1 + leading, index));
            }
        }
    }
    blocks
}

/// 按深度 0 的分号或「空行 + select/with」切出多条语句。
pub fn statements(text: &str) -> Vec<crate::span::Span> {
    let masked = crate::sql::lexer::mask(text);
    let bytes = masked.as_bytes();
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
            b'\n' if depth == 0 && STATEMENT_BREAK.is_match(&masked[index..]) => {
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
    let masked = crate::sql::lexer::mask(body);
    let mut depth: i32 = 0;
    let mut start = 0;
    for (index, byte) in masked.bytes().enumerate() {
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
    let masked = crate::sql::lexer::mask(body);
    let low = masked.to_ascii_lowercase();
    let bytes = masked.as_bytes();
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

/// 条件列表的区间：深度 0 的 and/or 切分、合并 `between … and`、去掉首锚点。
pub fn condition_spans(body: &str) -> Vec<crate::span::Span> {
    let masked = crate::sql::lexer::mask(body);
    let low = masked.to_ascii_lowercase();
    let bytes = masked.as_bytes();
    let mut pieces: Vec<(usize, usize)> = Vec::new();
    let mut depth = 0;
    let mut start = 0;
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'(' => depth += 1,
            b')' => depth -= 1,
            _ => {}
        }
        if depth == 0 {
            let mut matched = false;
            for (word, len) in [("and", 3usize), ("or", 2usize)] {
                let boundary = (index == 0 || !is_word(bytes[index - 1]))
                    && (index + len >= bytes.len() || !is_word(bytes[index + len]));
                if boundary && low.get(index..).is_some_and(|rest| rest.starts_with(word)) {
                    pieces.push((start, index));
                    index += len;
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
    pieces.push((start, body.len()));
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (piece_start, piece_end) in pieces {
        if low[piece_start..piece_end].trim().is_empty() {
            continue;
        }
        if let Some(last) = merged.last_mut() {
            let last_low = &low[last.0..last.1];
            if last_low.matches("between").count() > last_low.matches(" and ").count() {
                last.1 = piece_end;
                continue;
            }
        }
        merged.push((piece_start, piece_end));
    }
    if merged
        .first()
        .is_some_and(|first| matches!(low[first.0..first.1].trim(), "1 = 1" | "1 = 0"))
    {
        merged.remove(0);
    }
    merged
        .into_iter()
        .filter_map(|(start, end)| {
            let piece = &body[start..end];
            let leading = piece.len() - piece.trim_start().len();
            let trailing = piece.trim_end().len();
            if trailing <= leading {
                return None;
            }
            Some(crate::span::Span::new(start + leading, start + trailing))
        })
        .collect()
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
    starts_with_word(&trimmed, "select") || starts_with_word(&trimmed, "with")
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
