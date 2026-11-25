//! token 规范化：目前只做 `between` 条件的括号。
//!
//! 规则：条件位置（where / on / having / 布尔组 / case 的 when）一律写成
//! `(x between a and b)`，`select` 里的值表达式不加。括号对判定层没有副作用。

/// 一条 token 规范化问题；`span` 是待加括号的条件区间。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Issue {
    pub span: crate::span::Span,
    pub message: String,
}

/// 需要加括号的条件区间。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct Wrap {
    start: usize,
    end: usize,
}

/// 报告需要加括号的 `between` 条件。
pub fn issues(sql: &str) -> Vec<Issue> {
    collect(sql)
        .into_iter()
        .map(|wrap| Issue {
            span: crate::span::Span::new(wrap.start, wrap.end),
            message: "between condition should be parenthesized".to_owned(),
        })
        .collect()
}

/// 给所有 `between` 条件加上括号。
pub fn normalize(sql: &str) -> String {
    let wraps = collect(sql);
    if wraps.is_empty() {
        return sql.to_owned();
    }
    let mut events: Vec<(usize, u8)> = Vec::new();
    for wrap in wraps {
        events.push((wrap.start, 1));
        events.push((wrap.end, 0));
    }
    events.sort();
    let mut output = String::with_capacity(sql.len() + events.len());
    let mut events = events.into_iter().peekable();
    for (index, character) in sql.char_indices() {
        while let Some((position, kind)) = events.peek().copied() {
            if position != index {
                break;
            }
            output.push(if kind == 0 { ')' } else { '(' });
            events.next();
        }
        output.push(character);
    }
    for (position, kind) in events {
        debug_assert_eq!(position, sql.len());
        output.push(if kind == 0 { ')' } else { '(' });
    }
    output
}

/// 收集所有需要加括号的条件区间。
fn collect(sql: &str) -> Vec<Wrap> {
    let mut wraps = Vec::new();
    for statement in crate::sql::splitter::statements(sql) {
        let statement_text = &sql[statement.start..statement.end];
        for block in crate::sql::splitter::all_statements(statement_text) {
            let block_text = &statement_text[block.start..block.end];
            let base = statement.start + block.start;
            for clause in crate::sql::splitter::clause_spans(block_text) {
                if !matches!(clause.kind, "where" | "having" | "from") {
                    continue;
                }
                let body = &block_text[clause.body_start..clause.body_end];
                let body_base = base + clause.body_start;
                if clause.kind == "from" {
                    collect_on(body, body_base, &mut wraps);
                } else {
                    collect_list(body, body_base, &mut wraps);
                }
            }
            collect_case_whens(block_text, base, &mut wraps);
        }
    }
    wraps.sort();
    wraps.dedup();
    wraps
}

/// 条件列表：逐个条件处理，嵌套布尔组递归。
fn collect_list(body: &str, base: usize, wraps: &mut Vec<Wrap>) {
    for condition in crate::sql::splitter::condition_spans(body) {
        let text = &body[condition.start..condition.end];
        for group in crate::sql::splitter::boolean_group_spans(text) {
            collect_list(
                &text[group.start..group.end],
                base + condition.start + group.start,
                wraps,
            );
        }
        if has_top_level_between(text) && !is_wrapped(text) {
            wraps.push(Wrap {
                start: base + condition.start,
                end: base + condition.end,
            });
        }
    }
}

/// `from` 里每个 `on` 的条件列表。
fn collect_on(body: &str, base: usize, wraps: &mut Vec<Wrap>) {
    let tokens = crate::sql::lexer::tokens(body);
    let mut index = 0;
    while index < tokens.len() {
        let token = tokens[index];
        if token.kind != crate::sql::lexer::Kind::Word || !token.text.eq_ignore_ascii_case("on") {
            index += 1;
            continue;
        }
        let start = token.start + token.text.len();
        let mut end = body.len();
        let mut depth = 0;
        for next in &tokens[index + 1..] {
            match next.kind {
                crate::sql::lexer::Kind::Punct if next.text == "(" => depth += 1,
                crate::sql::lexer::Kind::Punct if next.text == ")" => depth -= 1,
                crate::sql::lexer::Kind::Punct if next.text == "," && depth == 0 => {
                    end = next.start;
                    break;
                }
                crate::sql::lexer::Kind::Word
                    if depth == 0
                        && matches!(
                            next.text.to_ascii_lowercase().as_str(),
                            "join" | "left" | "right" | "full" | "inner" | "cross" | "natural"
                        ) =>
                {
                    end = next.start;
                    break;
                }
                _ => {}
            }
        }
        if end > start {
            collect_list(&body[start..end], base + start, wraps);
        }
        index += 1;
    }
}

/// `case … end` 里每个 `when` 的条件列表。
fn collect_case_whens(block: &str, base: usize, wraps: &mut Vec<Wrap>) {
    let tokens = crate::sql::lexer::tokens(block);
    for (index, token) in tokens.iter().enumerate() {
        if token.kind != crate::sql::lexer::Kind::Word || !token.text.eq_ignore_ascii_case("case") {
            continue;
        }
        let mut nesting = 1;
        let mut end = None;
        for next in &tokens[index + 1..] {
            if next.kind == crate::sql::lexer::Kind::Word && next.text.eq_ignore_ascii_case("case")
            {
                nesting += 1;
            } else if next.kind == crate::sql::lexer::Kind::Word
                && next.text.eq_ignore_ascii_case("end")
            {
                nesting -= 1;
                if nesting == 0 {
                    end = Some(next.start + next.text.len());
                    break;
                }
            }
        }
        let Some(end) = end else {
            continue;
        };
        let mut nesting = 0;
        let mut when_start = None;
        for next in &tokens[index..] {
            if next.start >= end {
                break;
            }
            if next.kind != crate::sql::lexer::Kind::Word {
                continue;
            }
            let lower = next.text.to_ascii_lowercase();
            if lower == "case" {
                nesting += 1;
            } else if lower == "end" {
                nesting -= 1;
            } else if nesting == 1 && lower == "when" {
                when_start = Some(next.start + next.text.len());
            } else if nesting == 1
                && lower == "then"
                && let Some(start) = when_start.take()
                && next.start > start
            {
                collect_list(&block[start..next.start], base + start, wraps);
            }
        }
    }
}

/// 条件里是否有深度 0 的 `between`。
fn has_top_level_between(condition: &str) -> bool {
    let masked = crate::sql::lexer::mask(condition);
    let bytes = masked.as_bytes();
    let lower = masked.to_ascii_lowercase();
    let mut depth = 0;
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'(' => depth += 1,
            b')' => depth -= 1,
            _ => {}
        }
        if depth == 0
            && (index == 0 || !is_word(bytes[index - 1]))
            && lower
                .get(index..)
                .is_some_and(|rest| rest.starts_with("between"))
        {
            return true;
        }
        index += 1;
    }
    false
}

/// 条件是否已经被一对完整括号包住。
fn is_wrapped(condition: &str) -> bool {
    let trimmed = condition.trim();
    if !(trimmed.starts_with('(') && trimmed.ends_with(')')) {
        return false;
    }
    let masked = crate::sql::lexer::mask(trimmed);
    let bytes = masked.as_bytes();
    let mut depth = 0;
    for (index, byte) in bytes.iter().enumerate() {
        match *byte {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 && index + 1 != bytes.len() {
                    return false;
                }
            }
            _ => {}
        }
    }
    depth == 0
}

fn is_word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

#[cfg(test)]
mod tests {
    fn normalized(sql: &str) -> String {
        crate::sql::token::normalize(sql)
    }

    #[test]
    fn wraps_between_condition() {
        assert_eq!(
            normalized("select a from t where x between 1 and 2"),
            "select a from t where (x between 1 and 2)"
        );
    }

    #[test]
    fn leaves_wrapped_condition_alone() {
        let sql = "select a from t where (x between 1 and 2)";
        assert_eq!(normalized(sql), sql);
    }

    #[test]
    fn wraps_only_the_between_condition_inline() {
        assert_eq!(
            normalized("select a from t where a = 1 and b between 2 and 3"),
            "select a from t where a = 1 and (b between 2 and 3)"
        );
    }

    #[test]
    fn wraps_inside_boolean_group() {
        assert_eq!(
            normalized("select a from t where (a = 1 or b between 2 and 3)"),
            "select a from t where (a = 1 or (b between 2 and 3))"
        );
    }

    #[test]
    fn wraps_case_when_condition() {
        assert_eq!(
            normalized("select (case when x between 1 and 2 then 1 else 0 end) as f from t"),
            "select (case when (x between 1 and 2) then 1 else 0 end) as f from t"
        );
    }

    #[test]
    fn wraps_on_condition() {
        assert_eq!(
            normalized("select a from t left join u on a between 1 and 2 and b = 3"),
            "select a from t left join u on (a between 1 and 2) and b = 3"
        );
    }

    #[test]
    fn ignores_select_value() {
        let sql = "select x between 1 and 2 as f from t";
        assert_eq!(normalized(sql), sql);
    }

    #[test]
    fn reports_issues_once() {
        let issues = crate::sql::token::issues("select a from t where x between 1 and 2");
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].span.start, 22);
    }

    #[test]
    fn normalize_is_idempotent() {
        let once = normalized("select a from t where x between 1 and 2");
        assert_eq!(normalized(&once), once);
    }
}
