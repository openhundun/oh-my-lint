//! token 规范化：between 括号、大小写、空格、别名、case 括号与 not 改写。

use crate::span::Span;
use crate::sql::lexer::{Kind, Token, tokens};
use crate::sql::splitter;

/// 一条 token 规范化问题。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Issue {
    pub span: Span,
    pub message: String,
}

/// 一次 token 规范化编辑。
#[derive(Clone, Debug)]
struct Edit {
    span: Span,
    replacement: String,
    message: &'static str,
}

/// 需要加括号的条件区间。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct Wrap {
    start: usize,
    end: usize,
}

/// 报告全部 token 规范化问题。
pub fn issues(sql: &str) -> Vec<Issue> {
    edits(sql)
        .into_iter()
        .map(|edit| Issue {
            span: edit.span,
            message: edit.message.to_owned(),
        })
        .collect()
}

/// 应用全部 token 规范化编辑。
pub fn normalize(sql: &str) -> String {
    let mut current = sql.to_owned();
    for _ in 0..32 {
        let found = edits(&current);
        if found.is_empty() {
            break;
        }
        let mut ordered = found;
        ordered.sort_by(|left, right| {
            right
                .span
                .start
                .cmp(&left.span.start)
                .then(right.span.end.cmp(&left.span.end))
        });
        let mut changed = false;
        let mut boundary = usize::MAX;
        for edit in ordered {
            if edit.span.end > boundary || edit.span.end > current.len() {
                continue;
            }
            if current.get(edit.span.start..edit.span.end) == Some(edit.replacement.as_str()) {
                boundary = edit.span.start;
                continue;
            }
            current.replace_range(edit.span.start..edit.span.end, &edit.replacement);
            boundary = edit.span.start;
            changed = true;
        }
        if !changed {
            break;
        }
    }
    current
}

/// 收集当前文本上的全部编辑。
fn edits(sql: &str) -> Vec<Edit> {
    let mut edits = Vec::new();
    for wrap in between_wraps(sql) {
        edits.push(Edit {
            span: Span::empty(wrap.start),
            replacement: "(".to_owned(),
            message: "between condition should be parenthesized",
        });
        edits.push(Edit {
            span: Span::empty(wrap.end),
            replacement: ")".to_owned(),
            message: "between condition should be parenthesized",
        });
    }
    let tokens = tokens(sql);
    lowercase_edits(&tokens, &mut edits);
    symbol_edits(&tokens, sql, &mut edits);
    spacing_edits(&tokens, sql, &mut edits);
    keyword_edits(&tokens, sql, &mut edits);
    alias_edits(sql, &mut edits);
    case_paren_edits(sql, &mut edits);
    edits
}

/// 关键字与裸标识符小写；`<>` 改写为 `!=`；去尾分号与行尾空白。
fn lowercase_edits(tokens: &[Token], edits: &mut Vec<Edit>) {
    for token in tokens {
        if token.kind != Kind::Word {
            continue;
        }
        let lower = token.text.to_lowercase();
        if lower != token.text {
            edits.push(Edit {
                span: span(token),
                replacement: lower,
                message: "keywords and bare identifiers should be lowercase",
            });
        }
    }
}

fn symbol_edits(tokens: &[Token], sql: &str, edits: &mut Vec<Edit>) {
    for token in tokens {
        if token.kind == Kind::Punct && token.text == "<>" {
            edits.push(Edit {
                span: span(token),
                replacement: "!=".to_owned(),
                message: "use != instead of <>",
            });
        }
    }
    let trimmed_len = sql.trim_end().len();
    if trimmed_len > 0 && sql.as_bytes()[trimmed_len - 1] == b';' {
        edits.push(Edit {
            span: Span::new(trimmed_len - 1, trimmed_len),
            replacement: String::new(),
            message: "drop the trailing semicolon",
        });
    }
    let bytes = sql.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let line_start = index;
        while index < bytes.len() && bytes[index] != b'\n' {
            index += 1;
        }
        let mut end = index;
        while end > line_start && matches!(bytes[end - 1], b' ' | b'\t') {
            end -= 1;
        }
        if end < index {
            edits.push(Edit {
                span: Span::new(end, index),
                replacement: String::new(),
                message: "remove trailing whitespace",
            });
        }
        if index < bytes.len() {
            index += 1;
        }
    }
}

fn spacing_edits(tokens: &[Token], sql: &str, edits: &mut Vec<Edit>) {
    for (index, token) in tokens.iter().enumerate() {
        if token.kind != Kind::Punct {
            continue;
        }
        match token.text {
            "(" => {
                let Some(previous) = previous_non_ws(tokens, index) else {
                    continue;
                };
                let name = &tokens[previous];
                let is_name = match name.kind {
                    Kind::Word => crate::sql::decision::is_function_name(name.text),
                    Kind::QuotedIdent => true,
                    _ => false,
                };
                if is_name {
                    push_gap(
                        edits,
                        sql,
                        token_end(name),
                        token.start,
                        "",
                        "function call should hug its parentheses",
                    );
                }
            }
            "," => {
                if let Some(previous) = previous_non_ws(tokens, index) {
                    push_gap(
                        edits,
                        sql,
                        token_end(&tokens[previous]),
                        token.start,
                        "",
                        "no space before comma",
                    );
                }
                if let Some(next) = next_non_ws(tokens, index) {
                    push_gap(
                        edits,
                        sql,
                        token_end(token),
                        tokens[next].start,
                        " ",
                        "one space after comma",
                    );
                }
            }
            "=" | "!=" | "<" | "<=" | ">" | ">=" | "/" => {
                binary_gap(tokens, index, sql, edits);
            }
            "+" | "-" => {
                if is_unary(tokens, index) {
                    if let Some(next) = next_non_ws(tokens, index) {
                        push_gap(
                            edits,
                            sql,
                            token_end(token),
                            tokens[next].start,
                            "",
                            "no space after unary sign",
                        );
                    }
                } else {
                    binary_gap(tokens, index, sql, edits);
                }
            }
            "*" => {
                let previous = previous_non_ws(tokens, index);
                if previous.is_some_and(|previous| {
                    tokens[previous].kind == Kind::Punct
                        && matches!(tokens[previous].text, "(" | ".")
                }) {
                    continue;
                }
                let next = next_non_ws(tokens, index);
                if next.is_some_and(|next| {
                    tokens[next].kind == Kind::Punct && tokens[next].text == ")"
                }) {
                    continue;
                }
                binary_gap(tokens, index, sql, edits);
            }
            _ => {}
        }
    }
}

fn binary_gap(tokens: &[Token], index: usize, sql: &str, edits: &mut Vec<Edit>) {
    let token = &tokens[index];
    if let Some(previous) = previous_non_ws(tokens, index) {
        push_gap(
            edits,
            sql,
            token_end(&tokens[previous]),
            token.start,
            " ",
            "one space around binary operator",
        );
    }
    if let Some(next) = next_non_ws(tokens, index) {
        push_gap(
            edits,
            sql,
            token_end(token),
            tokens[next].start,
            " ",
            "one space around binary operator",
        );
    }
}

fn is_unary(tokens: &[Token], index: usize) -> bool {
    let Some(previous) = previous_non_ws(tokens, index) else {
        return true;
    };
    let token = &tokens[previous];
    match token.kind {
        Kind::Punct => !matches!(token.text, ")" | "]"),
        Kind::Word => matches!(
            token.text.to_ascii_lowercase().as_str(),
            "select"
                | "when"
                | "then"
                | "else"
                | "and"
                | "or"
                | "not"
                | "between"
                | "by"
                | "on"
                | "where"
                | "having"
                | "as"
                | "in"
                | "is"
                | "null"
                | "desc"
                | "asc"
                | "limit"
                | "case"
                | "return"
                | "values"
        ),
        _ => false,
    }
}

/// `left outer join`、`asc`、`nulls first` 位置、`distinct(x)`、`not x is null`。
fn keyword_edits(tokens: &[Token], sql: &str, edits: &mut Vec<Edit>) {
    for (index, token) in tokens.iter().enumerate() {
        if token.kind != Kind::Word {
            continue;
        }
        let lower = token.text.to_ascii_lowercase();
        match lower.as_str() {
            "outer" => {
                if let Some(previous) = previous_non_ws(tokens, index) {
                    let before = &tokens[previous];
                    if before.kind == Kind::Word
                        && matches!(
                            before.text.to_ascii_lowercase().as_str(),
                            "left" | "right" | "full"
                        )
                    {
                        push_gap(
                            edits,
                            sql,
                            token_end(before),
                            token_end(token),
                            "",
                            "left outer join should be left join",
                        );
                    }
                }
            }
            "asc" => {
                if let Some(previous) = previous_non_ws(tokens, index) {
                    push_gap(
                        edits,
                        sql,
                        token_end(&tokens[previous]),
                        token_end(token),
                        "",
                        "explicit asc is redundant",
                    );
                }
            }
            "nulls" => {
                let Some(which) = next_non_ws(tokens, index) else {
                    continue;
                };
                let candidate = &tokens[which];
                if candidate.kind != Kind::Word
                    || !matches!(
                        candidate.text.to_ascii_lowercase().as_str(),
                        "first" | "last"
                    )
                {
                    continue;
                }
                let Some(direction) = next_non_ws(tokens, which) else {
                    continue;
                };
                let direction_token = &tokens[direction];
                if direction_token.kind != Kind::Word
                    || !matches!(
                        direction_token.text.to_ascii_lowercase().as_str(),
                        "asc" | "desc"
                    )
                {
                    continue;
                }
                let gap = &sql[token.start..token_end(direction_token)];
                if gap.contains('\n') {
                    continue;
                }
                edits.push(Edit {
                    span: Span::new(token.start, token_end(direction_token)),
                    replacement: format!(
                        "{} nulls {}",
                        direction_token.text.to_ascii_lowercase(),
                        candidate.text.to_ascii_lowercase()
                    ),
                    message: "nulls first/last goes after the direction",
                });
            }
            "distinct" => {
                let Some(open) = next_non_ws(tokens, index) else {
                    continue;
                };
                if tokens[open].kind != Kind::Punct || tokens[open].text != "(" {
                    continue;
                }
                let Some(close) = matching_paren(tokens, open) else {
                    continue;
                };
                let inner: Vec<&Token> = tokens[open + 1..close]
                    .iter()
                    .filter(|token| token.kind != Kind::Whitespace)
                    .collect();
                if inner.len() != 1
                    || !matches!(inner[0].kind, Kind::Word | Kind::QuotedIdent | Kind::Number)
                {
                    continue;
                }
                let gap = &sql[token_end(token)..tokens[close].start];
                if gap.contains('\n') {
                    continue;
                }
                edits.push(Edit {
                    span: Span::new(token_end(token), token_end(&tokens[close])),
                    replacement: format!(" {}", inner[0].text),
                    message: "distinct is a keyword, not a function",
                });
            }
            "not" => {
                let Some(first) = next_non_ws(tokens, index) else {
                    continue;
                };
                let Some((next, ident_end)) = simple_identifier(tokens, first) else {
                    continue;
                };
                let Some(after) = next_non_ws(tokens, next.saturating_sub(1)) else {
                    continue;
                };
                if tokens[after].kind == Kind::Word && tokens[after].text.eq_ignore_ascii_case("is")
                {
                    let Some(null_token) = next_non_ws(tokens, after) else {
                        continue;
                    };
                    if tokens[null_token].kind == Kind::Word
                        && tokens[null_token].text.eq_ignore_ascii_case("null")
                    {
                        edits.push(Edit {
                            span: Span::new(token.start, token_end(&tokens[null_token])),
                            replacement: format!(
                                "{} is not null",
                                &sql[tokens[first].start..ident_end]
                            ),
                            message: "use is not null instead of not ... is null",
                        });
                    }
                } else if tokens[after].kind == Kind::Punct && tokens[after].text == "=" {
                    let Some(rhs) = next_non_ws(tokens, after) else {
                        continue;
                    };
                    let rhs_token = &tokens[rhs];
                    let simple = match rhs_token.kind {
                        Kind::Word => !matches!(
                            rhs_token.text.to_ascii_lowercase().as_str(),
                            "any" | "all" | "some" | "null" | "exists"
                        ),
                        Kind::QuotedIdent | Kind::Number | Kind::String => true,
                        _ => false,
                    };
                    let followed_by_call = next_non_ws(tokens, rhs).is_some_and(|next| {
                        tokens[next].kind == Kind::Punct && tokens[next].text == "("
                    });
                    if simple && !followed_by_call {
                        edits.push(Edit {
                            span: Span::new(token.start, token_end(rhs_token)),
                            replacement: format!(
                                "{} != {}",
                                &sql[tokens[first].start..ident_end],
                                &sql[rhs_token.start..token_end(rhs_token)]
                            ),
                            message: "use != instead of not ... = ...",
                        });
                    }
                }
            }
            _ => {}
        }
    }
}

/// `from` 里的表别名去 `as`；`select` 里缺 `as` 的字段别名补上。
fn alias_edits(sql: &str, edits: &mut Vec<Edit>) {
    for statement in splitter::statements(sql) {
        let statement_text = &sql[statement.start..statement.end];
        for block in splitter::all_statements(statement_text) {
            let block_text = &statement_text[block.start..block.end];
            let block_base = statement.start + block.start;
            for clause in splitter::clause_spans(block_text) {
                let body = &block_text[clause.body_start..clause.body_end];
                let body_base = block_base + clause.body_start;
                if clause.kind == "from" {
                    table_alias_edits(body, sql, body_base, edits);
                }
                if clause.kind == "select" {
                    field_alias_edits(body, sql, body_base, edits);
                }
            }
        }
    }
}

fn table_alias_edits(body: &str, sql: &str, base: usize, edits: &mut Vec<Edit>) {
    let tokens = tokens(body);
    let mut depth = 0;
    for (index, token) in tokens.iter().enumerate() {
        match token.kind {
            Kind::Punct if token.text == "(" => depth += 1,
            Kind::Punct if token.text == ")" => depth -= 1,
            Kind::Word if depth == 0 && token.text.eq_ignore_ascii_case("as") => {
                let Some(previous) = previous_non_ws(&tokens, index) else {
                    continue;
                };
                let before = &tokens[previous];
                let valid_before = matches!(before.kind, Kind::Word | Kind::QuotedIdent)
                    || (before.kind == Kind::Punct && before.text == ")");
                if !valid_before {
                    continue;
                }
                let Some(next) = next_non_ws(&tokens, index) else {
                    continue;
                };
                if !matches!(tokens[next].kind, Kind::Word | Kind::QuotedIdent) {
                    continue;
                }
                let gap = &sql[base + token_end(before)..base + token_end(token)];
                if gap.contains('\n') {
                    continue;
                }
                edits.push(Edit {
                    span: Span::new(base + token_end(before), base + token_end(token)),
                    replacement: String::new(),
                    message: "table alias should not use as",
                });
            }
            _ => {}
        }
    }
}

fn field_alias_edits(body: &str, sql: &str, base: usize, edits: &mut Vec<Edit>) {
    for element in comma_spans(body) {
        let element_text = &body[element.start..element.end];
        let tokens: Vec<Token> = tokens(element_text)
            .into_iter()
            .filter(|token| token.kind != Kind::Whitespace)
            .collect();
        if tokens.is_empty() {
            continue;
        }
        if tokens
            .iter()
            .any(|token| token.kind == Kind::Word && token.text.eq_ignore_ascii_case("into"))
        {
            continue;
        }
        let has_as = tokens
            .iter()
            .any(|token| token.kind == Kind::Word && token.text.eq_ignore_ascii_case("as"));
        if has_as {
            continue;
        }
        let Some(last) = tokens.last() else {
            continue;
        };
        if last.kind != Kind::Word {
            continue;
        }
        let lower = last.text.to_ascii_lowercase();
        if matches!(
            lower.as_str(),
            "end" | "null" | "true" | "false" | "distinct" | "all" | "unknown"
        ) {
            continue;
        }
        let Some(previous) = tokens.len().checked_sub(2).map(|index| &tokens[index]) else {
            continue;
        };
        let value_end = match previous.kind {
            Kind::Word | Kind::Number | Kind::String | Kind::QuotedIdent => true,
            Kind::Punct => previous.text == ")",
            _ => false,
        };
        if !value_end {
            continue;
        }
        if previous.kind == Kind::Word
            && matches!(
                previous.text.to_ascii_lowercase().as_str(),
                "distinct" | "all"
            )
        {
            continue;
        }
        let _ = sql;
        edits.push(Edit {
            span: Span::empty(base + element.start + last.start),
            replacement: "as ".to_owned(),
            message: "field alias needs as",
        });
    }
}

/// 独立 `case` 补括号（函数实参内的不加）。
fn case_paren_edits(sql: &str, edits: &mut Vec<Edit>) {
    let functions = crate::sql::checker::function_paren_spans(sql);
    let cases = crate::sql::checker::outermost(crate::sql::checker::case_spans(sql));
    for case in cases {
        if functions
            .iter()
            .any(|span| span.start <= case.start && case.end <= span.end)
        {
            continue;
        }
        let before = sql[..case.start].trim_end();
        let after = sql[case.end..].trim_start();
        if before.ends_with('(') && after.starts_with(')') {
            continue;
        }
        edits.push(Edit {
            span: Span::empty(case.start),
            replacement: "(".to_owned(),
            message: "standalone case should be parenthesized",
        });
        edits.push(Edit {
            span: Span::empty(case.end),
            replacement: ")".to_owned(),
            message: "standalone case should be parenthesized",
        });
    }
}

fn push_gap(
    edits: &mut Vec<Edit>,
    sql: &str,
    start: usize,
    end: usize,
    replacement: &str,
    message: &'static str,
) {
    if start > end || end > sql.len() {
        return;
    }
    let gap = &sql[start..end];
    if gap.contains('\n') || gap == replacement {
        return;
    }
    edits.push(Edit {
        span: Span::new(start, end),
        replacement: replacement.to_owned(),
        message,
    });
}

fn span(token: &Token) -> Span {
    Span::new(token.start, token_end(token))
}

fn token_end(token: &Token) -> usize {
    token.start + token.text.len()
}

fn previous_non_ws(tokens: &[Token], index: usize) -> Option<usize> {
    let mut probe = index;
    while probe > 0 {
        probe -= 1;
        if tokens[probe].kind != Kind::Whitespace {
            return Some(probe);
        }
    }
    None
}

fn next_non_ws(tokens: &[Token], index: usize) -> Option<usize> {
    let mut probe = index + 1;
    while probe < tokens.len() {
        if tokens[probe].kind != Kind::Whitespace {
            return Some(probe);
        }
        probe += 1;
    }
    None
}

fn matching_paren(tokens: &[Token], open: usize) -> Option<usize> {
    let mut depth = 0;
    for (index, token) in tokens.iter().enumerate().skip(open) {
        if token.kind != Kind::Punct {
            continue;
        }
        if token.text == "(" {
            depth += 1;
        } else if token.text == ")" {
            depth -= 1;
            if depth == 0 {
                return Some(index);
            }
        }
    }
    None
}

/// 简单标识符（可带 `.` 限定）；返回下一个 token 下标与结束字节。
fn simple_identifier(tokens: &[Token], start: usize) -> Option<(usize, usize)> {
    let first = tokens.get(start)?;
    if !matches!(first.kind, Kind::Word | Kind::QuotedIdent) {
        return None;
    }
    let mut end = token_end(first);
    let mut index = start;
    while let Some(dot) = next_non_ws(tokens, index) {
        if tokens[dot].kind != Kind::Punct || tokens[dot].text != "." {
            break;
        }
        let Some(part) = next_non_ws(tokens, dot) else {
            break;
        };
        if !matches!(tokens[part].kind, Kind::Word | Kind::QuotedIdent) {
            break;
        }
        end = token_end(&tokens[part]);
        index = part;
    }
    Some((index + 1, end))
}

/// 深度 0 逗号切出的元素区间。
fn comma_spans(body: &str) -> Vec<Span> {
    let masked = crate::sql::lexer::mask(body);
    let bytes = masked.as_bytes();
    let mut spans = Vec::new();
    let mut depth = 0;
    let mut start = 0;
    for (index, byte) in bytes.iter().enumerate() {
        match *byte {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b',' if depth == 0 => {
                push_trimmed(body, start, index, &mut spans);
                start = index + 1;
            }
            _ => {}
        }
    }
    push_trimmed(body, start, body.len(), &mut spans);
    spans
}

fn push_trimmed(body: &str, start: usize, end: usize, spans: &mut Vec<Span>) {
    let piece = &body[start..end];
    let leading = piece.len() - piece.trim_start().len();
    let trailing = piece.trim_end().len();
    if trailing > leading {
        spans.push(Span::new(start + leading, start + trailing));
    }
}

/// 需要加括号的条件区间。
fn between_wraps(sql: &str) -> Vec<Wrap> {
    let mut wraps = Vec::new();
    for statement in splitter::statements(sql) {
        let statement_text = &sql[statement.start..statement.end];
        for block in splitter::all_statements(statement_text) {
            let block_text = &statement_text[block.start..block.end];
            let base = statement.start + block.start;
            for clause in splitter::clause_spans(block_text) {
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
    for condition in splitter::condition_spans(body) {
        let text = &body[condition.start..condition.end];
        for group in splitter::boolean_group_spans(text) {
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
    let tokens = tokens(body);
    let mut index = 0;
    while index < tokens.len() {
        let token = tokens[index];
        if token.kind != Kind::Word || !token.text.eq_ignore_ascii_case("on") {
            index += 1;
            continue;
        }
        let start = token_end(&token);
        let mut end = body.len();
        let mut depth = 0;
        for next in &tokens[index + 1..] {
            match next.kind {
                Kind::Punct if next.text == "(" => depth += 1,
                Kind::Punct if next.text == ")" => depth -= 1,
                Kind::Punct if next.text == "," && depth == 0 => {
                    end = next.start;
                    break;
                }
                Kind::Word
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
    let tokens = tokens(block);
    for (index, token) in tokens.iter().enumerate() {
        if token.kind != Kind::Word || !token.text.eq_ignore_ascii_case("case") {
            continue;
        }
        let mut nesting = 1;
        let mut end = None;
        for next in &tokens[index + 1..] {
            if next.kind == Kind::Word && next.text.eq_ignore_ascii_case("case") {
                nesting += 1;
            } else if next.kind == Kind::Word && next.text.eq_ignore_ascii_case("end") {
                nesting -= 1;
                if nesting == 0 {
                    end = Some(token_end(next));
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
            if next.kind != Kind::Word {
                continue;
            }
            let lower = next.text.to_ascii_lowercase();
            if lower == "case" {
                nesting += 1;
            } else if lower == "end" {
                nesting -= 1;
            } else if nesting == 1 && lower == "when" {
                when_start = Some(token_end(next));
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

    fn assert_fixed(sql: &str, expected: &str) {
        let fixed = normalized(sql);
        assert_eq!(fixed, expected);
        assert_eq!(normalized(&fixed), fixed);
    }

    #[test]
    fn skips_into_targets_and_unknown_predicates() {
        assert_fixed(
            "SELECT foo, bar INTO baz FROM qux;",
            "select foo, bar into baz from qux",
        );
        assert_fixed("SELECT TRUE IS NOT UNKNOWN;", "select true is not unknown");
    }

    #[test]
    fn wraps_between_condition() {
        assert_fixed(
            "select a from t where x between 1 and 2",
            "select a from t where (x between 1 and 2)",
        );
    }

    #[test]
    fn wraps_between_with_nested_and_in_subqueries() {
        assert_fixed(
            "select a from t where d between (select x from u where p = 1 and q = 2) and (select y from u)",
            "select a from t where (d between (select x from u where p = 1 and q = 2) and (select y from u))",
        );
    }

    #[test]
    fn leaves_wrapped_condition_alone() {
        let sql = "select a from t where (x between 1 and 2)";
        assert_eq!(normalized(sql), sql);
    }

    #[test]
    fn wraps_only_the_between_condition_inline() {
        assert_fixed(
            "select a from t where a = 1 and b between 2 and 3",
            "select a from t where a = 1 and (b between 2 and 3)",
        );
    }

    #[test]
    fn wraps_inside_boolean_group() {
        assert_fixed(
            "select a from t where (a = 1 or b between 2 and 3)",
            "select a from t where (a = 1 or (b between 2 and 3))",
        );
    }

    #[test]
    fn wraps_case_when_condition() {
        assert_fixed(
            "select (case when x between 1 and 2 then 1 else 0 end) as f from t",
            "select (case when (x between 1 and 2) then 1 else 0 end) as f from t",
        );
    }

    #[test]
    fn wraps_on_condition() {
        assert_fixed(
            "select a from t left join u on a between 1 and 2 and b = 3",
            "select a from t left join u on (a between 1 and 2) and b = 3",
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
        assert_eq!(issues.len(), 2);
    }

    #[test]
    fn lowercases_words_and_keeps_literals() {
        assert_fixed(
            "SELECT A, 'Keep Me' AS B FROM T",
            "select a, 'Keep Me' as b from t",
        );
    }

    #[test]
    fn rewrites_symbols_and_spacing() {
        assert_fixed(
            "select a,b from t where x<>y",
            "select a, b from t where x != y",
        );
        assert_fixed("select a from t;", "select a from t");
        assert_fixed("select a from t;   ", "select a from t");
    }

    #[test]
    fn hugs_function_parentheses() {
        assert_fixed(
            "select count (*) from t group by rollup (a, b)",
            "select count(*) from t group by rollup(a, b)",
        );
    }

    #[test]
    fn removes_outer_and_asc() {
        assert_fixed(
            "select a from t left outer join u on t.id = u.id order by a asc",
            "select a from t left join u on t.id = u.id order by a",
        );
    }

    #[test]
    fn rewrites_distinct_call() {
        assert_fixed("select distinct(a) from t", "select distinct a from t");
    }

    #[test]
    fn rewrites_not_equals() {
        assert_fixed(
            "select a from t where not b = 1",
            "select a from t where b != 1",
        );
        assert_fixed(
            "select a from t where not b = any(select c from u)",
            "select a from t where not b = any(select c from u)",
        );
    }

    #[test]
    fn rewrites_not_is_null() {
        assert_fixed(
            "select a from t where not b is null",
            "select a from t where b is not null",
        );
    }

    #[test]
    fn adds_field_alias_as() {
        assert_fixed(
            "select sum(a) total from t",
            "select sum(a) as total from t",
        );
        assert_fixed("select a b from t", "select a as b from t");
        assert_fixed("select distinct a from t", "select distinct a from t");
    }

    #[test]
    fn removes_table_alias_as() {
        assert_fixed(
            "select a from t as x, (select 1) as y",
            "select a from t x, (select 1) y",
        );
    }

    #[test]
    fn parenthesizes_standalone_case() {
        assert_fixed(
            "select case when a = 1 then 1 else 0 end as f from t",
            "select (case when a = 1 then 1 else 0 end) as f from t",
        );
        assert_fixed(
            "select sum(case when a = 1 then 1 else 0 end) as f from t",
            "select sum(case when a = 1 then 1 else 0 end) as f from t",
        );
    }
}
