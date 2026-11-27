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
    edits(sql).into_iter().map(|edit| Issue { span: edit.span, message: edit.message.to_owned() }).collect()
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
        ordered.sort_by(|left, right| right.span.start.cmp(&left.span.start).then(right.span.end.cmp(&left.span.end)));
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
    default_form_edits(&tokens, sql, &mut edits);
    if crate::sql::lexer::current_dialect() == crate::sql::parse::Dialect::Postgres {
        typed_literal_edits(&tokens, sql, &mut edits);
        interval_cast_edits(&tokens, &mut edits);
    }
    type_alias_edits(&tokens, sql, &mut edits);
    alias_edits(sql, &mut edits);
    case_paren_edits(sql, &mut edits);
    edits
}

/// 关键字与裸标识符小写；`<>` 改写为 `!=`；去行尾空白。
fn lowercase_edits(tokens: &[Token], edits: &mut Vec<Edit>) {
    for token in tokens {
        if token.kind != Kind::Word {
            continue;
        }
        let lower = token.text.to_lowercase();
        if lower != token.text {
            edits.push(Edit { span: span(token), replacement: lower, message: "keywords and bare identifiers should be lowercase" });
        }
    }
}

fn symbol_edits(tokens: &[Token], sql: &str, edits: &mut Vec<Edit>) {
    for token in tokens {
        if token.kind == Kind::Punct && token.text == "<>" {
            edits.push(Edit { span: span(token), replacement: "!=".to_owned(), message: "use != instead of <>" });
        }
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
            edits.push(Edit { span: Span::new(end, index), replacement: String::new(), message: "remove trailing whitespace" });
        }
        if index < bytes.len() {
            index += 1;
        }
    }
}

fn spacing_edits(tokens: &[Token], sql: &str, edits: &mut Vec<Edit>) {
    let flavors = statement_flavors(tokens, sql);
    for (index, token) in tokens.iter().enumerate() {
        if token.kind != Kind::Punct {
            continue;
        }
        match token.text {
            "(" => {
                let Some(previous) = previous_non_ws(tokens, index) else {
                    continue;
                };
                let Some(hug) = paren_hugs(tokens, &flavors, previous) else {
                    continue;
                };
                let name = &tokens[previous];
                if word_is(name, "distinct") && previous_non_ws(tokens, previous).is_some_and(|before| word_is(&tokens[before], "select")) {
                    continue;
                }
                push_gap(
                    edits,
                    sql,
                    token_end(name),
                    token.start,
                    if hug { "" } else { " " },
                    if hug {
                        "function call should hug its parentheses"
                    } else {
                        "keyword construct should keep one space before its parentheses"
                    },
                );
            }
            "," => {
                if let Some(previous) = previous_non_ws(tokens, index) {
                    push_gap(edits, sql, token_end(&tokens[previous]), token.start, "", "no space before comma");
                }
                if let Some(next) = next_non_ws(tokens, index) {
                    push_gap(edits, sql, token_end(token), tokens[next].start, " ", "one space after comma");
                }
            }
            "=" => {
                if exclude_operator(tokens, index) || operator_definition(tokens, index) {
                    continue;
                }
                if named_argument(tokens, index) {
                    if let Some(previous) = previous_non_ws(tokens, index) {
                        push_gap(edits, sql, token_end(&tokens[previous]), token.start, "", "named argument equals should hug");
                    }
                    if let Some(next) = next_non_ws(tokens, index) {
                        push_gap(edits, sql, token_end(token), tokens[next].start, "", "named argument equals should hug");
                    }
                } else {
                    binary_gap(tokens, index, sql, edits);
                }
            }
            "!=" | "<" | "<=" | ">" | ">=" | "/" => {
                if !operator_definition(tokens, index) {
                    binary_gap(tokens, index, sql, edits);
                }
            }
            "+" | "-" => {
                if operator_definition(tokens, index) {
                    continue;
                }
                if is_unary(tokens, index) {
                    if let Some(next) = next_non_ws(tokens, index) {
                        push_gap(edits, sql, token_end(token), tokens[next].start, "", "no space after unary sign");
                    }
                } else {
                    binary_gap(tokens, index, sql, edits);
                }
            }
            "*" => {
                let previous = previous_non_ws(tokens, index);
                if previous.is_some_and(|previous| tokens[previous].kind == Kind::Punct && matches!(tokens[previous].text, "(" | ".")) {
                    continue;
                }
                let next = next_non_ws(tokens, index);
                if next.is_some_and(|next| tokens[next].kind == Kind::Punct && matches!(tokens[next].text, ")" | ",")) {
                    continue;
                }
                binary_gap(tokens, index, sql, edits);
            }
            _ => {}
        }
    }
}

/// D4：`exclude` 元素里的 `room with =` 是运算符名，不是二元运算。
fn exclude_operator(tokens: &[Token], index: usize) -> bool {
    previous_non_ws(tokens, index).is_some_and(|previous| word_is(&tokens[previous], "with"))
}

/// `OPERATOR 1 <` / `COMMUTATOR =` 里的符号是定义名，不是二元运算。
fn operator_definition(tokens: &[Token], index: usize) -> bool {
    let Some(previous) = previous_non_ws(tokens, index) else {
        return false;
    };
    if word_is(&tokens[previous], "operator")
        || word_is(&tokens[previous], "commutator")
        || word_is(&tokens[previous], "negator")
        || word_is(&tokens[previous], "restrict")
        || word_is(&tokens[previous], "join")
    {
        return true;
    }
    previous_non_ws(tokens, previous).is_some_and(|before| word_is(&tokens[before], "operator"))
}

fn binary_gap(tokens: &[Token], index: usize, sql: &str, edits: &mut Vec<Edit>) {
    let token = &tokens[index];
    if let Some(previous) = previous_non_ws(tokens, index) {
        push_gap(edits, sql, token_end(&tokens[previous]), token.start, " ", "one space around binary operator");
    }
    if let Some(next) = next_non_ws(tokens, index) {
        push_gap(edits, sql, token_end(token), tokens[next].start, " ", "one space around binary operator");
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
                    if before.kind == Kind::Word && matches!(before.text.to_ascii_lowercase().as_str(), "left" | "right" | "full") {
                        push_gap(edits, sql, token_end(before), token_end(token), "", "left outer join should be left join");
                    }
                }
            }
            "asc" => {
                let Some(previous) = previous_non_ws(tokens, index) else {
                    continue;
                };
                if matches!(tokens[previous].text, "=" | "=>") {
                    continue;
                }
                push_gap(edits, sql, token_end(&tokens[previous]), token_end(token), "", "explicit asc is redundant");
            }
            "nulls" => {
                let Some(which) = next_non_ws(tokens, index) else {
                    continue;
                };
                let candidate = &tokens[which];
                if candidate.kind != Kind::Word {
                    continue;
                }
                let candidate_lower = candidate.text.to_ascii_lowercase();
                if candidate_lower == "distinct" {
                    if let Some(previous) = previous_non_ws(tokens, index)
                        && !word_is(&tokens[previous], "not")
                    {
                        push_gap(edits, sql, token_end(&tokens[previous]), token_end(candidate), "", "nulls distinct is the default");
                    }
                    continue;
                }
                if !matches!(candidate_lower.as_str(), "first" | "last") {
                    continue;
                }
                let previous = previous_non_ws(tokens, index);
                let direction_before = previous.map(|previous| tokens[previous].text.to_ascii_lowercase());
                let default_suffix = match direction_before.as_deref() {
                    Some("desc") => candidate_lower == "first",
                    _ => candidate_lower == "last",
                };
                if default_suffix {
                    if let Some(previous) = previous {
                        push_gap(edits, sql, token_end(&tokens[previous]), token_end(candidate), "", "default nulls ordering is redundant");
                    }
                    continue;
                }
                let Some(direction) = next_non_ws(tokens, which) else {
                    continue;
                };
                let direction_token = &tokens[direction];
                if direction_token.kind != Kind::Word || !matches!(direction_token.text.to_ascii_lowercase().as_str(), "asc" | "desc") {
                    continue;
                }
                let gap = &sql[token.start..token_end(direction_token)];
                if gap.contains('\n') {
                    continue;
                }
                edits.push(Edit {
                    span: Span::new(token.start, token_end(direction_token)),
                    replacement: format!("{} nulls {}", direction_token.text.to_ascii_lowercase(), candidate.text.to_ascii_lowercase()),
                    message: "nulls first/last goes after the direction",
                });
            }
            "distinct" => {
                if !previous_non_ws(tokens, index).is_some_and(|previous| word_is(&tokens[previous], "select")) {
                    continue;
                }
                let Some(open) = next_non_ws(tokens, index) else {
                    continue;
                };
                if tokens[open].kind != Kind::Punct || tokens[open].text != "(" {
                    continue;
                }
                let Some(close) = matching_paren(tokens, open) else {
                    continue;
                };
                let inner: Vec<&Token> = tokens[open + 1..close].iter().filter(|token| token.kind != Kind::Whitespace).collect();
                if inner.len() != 1 || !matches!(inner[0].kind, Kind::Word | Kind::QuotedIdent | Kind::Number) {
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
                if tokens[after].kind == Kind::Word && tokens[after].text.eq_ignore_ascii_case("is") {
                    let Some(null_token) = next_non_ws(tokens, after) else {
                        continue;
                    };
                    if tokens[null_token].kind == Kind::Word && tokens[null_token].text.eq_ignore_ascii_case("null") {
                        edits.push(Edit {
                            span: Span::new(token.start, token_end(&tokens[null_token])),
                            replacement: format!("{} is not null", &sql[tokens[first].start..ident_end]),
                            message: "use is not null instead of not ... is null",
                        });
                    }
                } else if tokens[after].kind == Kind::Punct && tokens[after].text == "=" {
                    let Some(rhs) = next_non_ws(tokens, after) else {
                        continue;
                    };
                    let rhs_token = &tokens[rhs];
                    let simple = match rhs_token.kind {
                        Kind::Word => !matches!(rhs_token.text.to_ascii_lowercase().as_str(), "any" | "all" | "some" | "null" | "exists"),
                        Kind::QuotedIdent | Kind::Number | Kind::String => true,
                        _ => false,
                    };
                    let followed_by_call = next_non_ws(tokens, rhs).is_some_and(|next| tokens[next].kind == Kind::Punct && tokens[next].text == "(");
                    if simple && !followed_by_call {
                        edits.push(Edit {
                            span: Span::new(token.start, token_end(rhs_token)),
                            replacement: format!("{} != {}", &sql[tokens[first].start..ident_end], &sql[rhs_token.start..token_end(rhs_token)]),
                            message: "use != instead of not ... = ...",
                        });
                    }
                }
            }
            _ => {}
        }
    }
}

/// 语句种类：只区分 token 规则需要上下文的三类 DDL。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StatementFlavor {
    CreateTable,
    CreateIndex,
    AlterTable,
    Other,
}

fn statement_flavors(tokens: &[Token], sql: &str) -> Vec<(Span, StatementFlavor)> {
    let mut flavors = Vec::new();
    for statement in splitter::statements(sql) {
        flavors.push((statement, classify_statement(tokens, statement)));
    }
    flavors
}

fn classify_statement(tokens: &[Token], statement: Span) -> StatementFlavor {
    let mut words = tokens.iter().filter(|token| token.kind == Kind::Word && token.start >= statement.start && token.start < statement.end);
    let Some(first) = words.next() else {
        return StatementFlavor::Other;
    };
    if word_is(first, "alter") {
        return if words.next().is_some_and(|word| word_is(word, "table")) {
            StatementFlavor::AlterTable
        } else {
            StatementFlavor::Other
        };
    }
    if !word_is(first, "create") {
        return StatementFlavor::Other;
    }
    for word in words.take(4) {
        if word_is(word, "table") {
            return StatementFlavor::CreateTable;
        }
        if word_is(word, "index") {
            return StatementFlavor::CreateIndex;
        }
        let lower = word.text.to_ascii_lowercase();
        if !matches!(lower.as_str(), "or" | "replace" | "unique" | "temp" | "temporary" | "unlogged" | "global" | "local" | "materialized") {
            return StatementFlavor::Other;
        }
    }
    StatementFlavor::Other
}

fn flavor_at(flavors: &[(Span, StatementFlavor)], offset: usize) -> StatementFlavor {
    statement_at(flavors, offset)
        .map(|statement| flavors.iter().find(|(span, _)| *span == statement).map_or(StatementFlavor::Other, |(_, flavor)| *flavor))
        .unwrap_or(StatementFlavor::Other)
}

fn statement_at(flavors: &[(Span, StatementFlavor)], offset: usize) -> Option<Span> {
    flavors.iter().find(|(span, _)| span.start <= offset && offset <= span.end).map(|(span, _)| *span)
}

fn depth_before(tokens: &[Token], statement: Span, index: usize) -> i32 {
    let mut depth = 0;
    for token in &tokens[..index.min(tokens.len())] {
        if token.start < statement.start || token.kind != Kind::Punct {
            continue;
        }
        match token.text {
            "(" => depth += 1,
            ")" => depth -= 1,
            _ => {}
        }
    }
    depth
}

fn word_is(token: &Token, text: &str) -> bool {
    token.kind == Kind::Word && token.text.eq_ignore_ascii_case(text)
}

fn punct(token: &Token, text: &str) -> bool {
    token.kind == Kind::Punct && token.text == text
}

fn ident(token: &Token) -> bool {
    matches!(token.kind, Kind::Word | Kind::QuotedIdent)
}

/// `(` 前作为关键字构造出现的词；这些词与括号之间保留一个空格。
fn spaced_paren_word(lower: &str) -> bool {
    matches!(
        lower,
        "check"
            | "primary"
            | "unique"
            | "foreign"
            | "key"
            | "references"
            | "include"
            | "exclude"
            | "partition"
            | "inherits"
            | "tablespace"
            | "set"
            | "opclass"
            | "explain"
            | "default"
            | "to"
            | "also"
            | "instead"
            | "conflict"
            | "materialized"
            | "overlaps"
    )
}

/// `(` 前是否是表名/方法名等关系位置，而不是函数调用。
fn relation_paren(tokens: &[Token], flavors: &[(Span, StatementFlavor)], name_index: usize) -> bool {
    let mut probe = name_index;
    loop {
        let Some(dot) = previous_non_ws(tokens, probe) else {
            return false;
        };
        if !punct(&tokens[dot], ".") {
            break;
        }
        let Some(part) = previous_non_ws(tokens, dot) else {
            return false;
        };
        if !ident(&tokens[part]) {
            return false;
        }
        probe = part;
    }
    let Some(before) = previous_non_ws(tokens, probe) else {
        return false;
    };
    if tokens[before].kind != Kind::Word {
        return false;
    }
    let lower = tokens[before].text.to_ascii_lowercase();
    let flavor = flavor_at(flavors, tokens[name_index].start);
    match lower.as_str() {
        "table" | "references" | "using" | "into" => true,
        "exists" | "of" => flavor == StatementFlavor::CreateTable,
        "on" => flavor == StatementFlavor::CreateIndex,
        "by" => flavor == StatementFlavor::CreateTable,
        "as" => {
            let Some(table) = previous_non_ws(tokens, before) else {
                return false;
            };
            ident(&tokens[table]) && previous_non_ws(tokens, table).is_some_and(|word| word_is(&tokens[word], "into"))
        }
        _ => false,
    }
}

/// CTE 名称后的字段列表 `name (a, b) as (` 需要空格。
fn cte_column_paren(tokens: &[Token], index: usize) -> bool {
    if tokens[index].kind != Kind::Word {
        return false;
    }
    if tokens[index].text.eq_ignore_ascii_case("with") || tokens[index].text.eq_ignore_ascii_case("recursive") {
        return false;
    }
    let Some(open) = next_non_ws(tokens, index) else {
        return false;
    };
    if !punct(&tokens[open], "(") {
        return false;
    }
    let Some(close) = matching_paren(tokens, open) else {
        return false;
    };
    let Some(after) = next_non_ws(tokens, close) else {
        return false;
    };
    if !word_is(&tokens[after], "as") {
        return false;
    }
    let Some(before) = previous_non_ws(tokens, index) else {
        return false;
    };
    if word_is(&tokens[before], "with") || word_is(&tokens[before], "recursive") {
        return true;
    }
    if !punct(&tokens[before], ",") {
        return false;
    }
    let mut depth = 0i32;
    let mut probe = before;
    while probe > 0 {
        probe -= 1;
        let token = &tokens[probe];
        match token.kind {
            Kind::Punct if token.text == ")" => depth += 1,
            Kind::Punct if token.text == "(" => depth -= 1,
            Kind::Word if depth == 0 && token.text.eq_ignore_ascii_case("with") => return true,
            _ => {}
        }
    }
    false
}

/// 派生表别名后的字段列表 `) t (a, b)` 需要空格，不能当函数调用。
fn alias_column_paren(tokens: &[Token], index: usize) -> bool {
    let token = &tokens[index];
    if !matches!(token.kind, Kind::Word | Kind::QuotedIdent) {
        return false;
    }
    if token.kind == Kind::Word
        && matches!(
            token.text.to_ascii_lowercase().as_str(),
            "over" | "filter" | "within" | "partition" | "group" | "order" | "by" | "using" | "on"
        )
    {
        return false;
    }
    let Some(before) = previous_non_ws(tokens, index) else {
        return false;
    };
    if punct(&tokens[before], ")") {
        return true;
    }
    word_is(&tokens[before], "as") && previous_non_ws(tokens, before).is_some_and(|index| punct(&tokens[index], ")"))
}

/// R39：判断 `(` 应紧贴前一个词（函数/类型调用）还是留一个空格（关键字构造）。
fn paren_hugs(tokens: &[Token], flavors: &[(Span, StatementFlavor)], index: usize) -> Option<bool> {
    let token = &tokens[index];
    if cte_column_paren(tokens, index) || alias_column_paren(tokens, index) {
        return Some(false);
    }
    match token.kind {
        Kind::QuotedIdent => Some(!relation_paren(tokens, flavors, index)),
        Kind::Word => {
            let lower = token.text.to_ascii_lowercase();
            if lower == "over" {
                return Some(true);
            }
            if spaced_paren_word(&lower) || relation_paren(tokens, flavors, index) {
                return Some(false);
            }
            if matches!(lower.as_str(), "left" | "right") {
                return Some(true);
            }
            Some(crate::sql::core::keywords::is_function_name(token.text))
        }
        _ => None,
    }
}

/// R37：`=` 左侧是调用实参开头处的裸名字时，视为命名参数。
fn named_argument(tokens: &[Token], index: usize) -> bool {
    let Some(mut lhs) = previous_non_ws(tokens, index) else {
        return false;
    };
    if !matches!(tokens[lhs].kind, Kind::Word | Kind::QuotedIdent) {
        return false;
    }
    while let Some(dot) = previous_non_ws(tokens, lhs) {
        if !punct(&tokens[dot], ".") {
            break;
        }
        let Some(part) = previous_non_ws(tokens, dot) else {
            break;
        };
        if !matches!(tokens[part].kind, Kind::Word | Kind::QuotedIdent) {
            break;
        }
        lhs = part;
    }
    let Some(before) = previous_non_ws(tokens, lhs) else {
        return false;
    };
    let open = if punct(&tokens[before], "(") {
        before
    } else if punct(&tokens[before], ",") {
        let Some(open) = enclosing_open(tokens, before) else {
            return false;
        };
        open
    } else {
        return false;
    };
    let Some(next) = next_non_ws(tokens, index) else {
        return false;
    };
    if punct(&tokens[next], ")") {
        return false;
    }
    let Some(name) = previous_non_ws(tokens, open) else {
        return false;
    };
    match tokens[name].kind {
        Kind::QuotedIdent => true,
        Kind::Word => {
            let lower = tokens[name].text.to_ascii_lowercase();
            if matches!(lower.as_str(), "with" | "set" | "opclass" | "include") {
                return true;
            }
            if matches!(
                lower.as_str(),
                "check"
                    | "where"
                    | "on"
                    | "having"
                    | "when"
                    | "case"
                    | "and"
                    | "or"
                    | "not"
                    | "select"
                    | "values"
                    | "in"
                    | "exists"
                    | "filter"
                    | "distinct"
                    | "order"
                    | "group"
                    | "primary"
                    | "foreign"
                    | "unique"
                    | "key"
                    | "references"
                    | "exclude"
                    | "partition"
                    | "inherits"
                    | "tablespace"
            ) {
                return false;
            }
            crate::sql::core::keywords::is_function_name(tokens[name].text)
        }
        _ => false,
    }
}

fn enclosing_open(tokens: &[Token], index: usize) -> Option<usize> {
    let mut depth = 0;
    let mut probe = index;
    while probe > 0 {
        probe -= 1;
        let token = &tokens[probe];
        if token.kind != Kind::Punct {
            continue;
        }
        if token.text == ")" {
            depth += 1;
        } else if token.text == "(" {
            if depth == 0 {
                return Some(probe);
            }
            depth -= 1;
        }
    }
    None
}

/// R38：默认形式的省略。
fn default_form_edits(tokens: &[Token], sql: &str, edits: &mut Vec<Edit>) {
    let flavors = statement_flavors(tokens, sql);
    for (index, token) in tokens.iter().enumerate() {
        if token.kind != Kind::Word {
            continue;
        }
        let lower = token.text.to_ascii_lowercase();
        match lower.as_str() {
            "match" => {
                let Some(simple) = next_non_ws(tokens, index) else {
                    continue;
                };
                if word_is(&tokens[simple], "simple") {
                    remove_phrase(tokens, sql, edits, index, simple, "match simple is the default");
                }
            }
            "not" => {
                let Some(deferrable) = next_non_ws(tokens, index) else {
                    continue;
                };
                if !word_is(&tokens[deferrable], "deferrable") {
                    continue;
                }
                let Some(initially) = next_non_ws(tokens, deferrable) else {
                    continue;
                };
                if !word_is(&tokens[initially], "initially") {
                    continue;
                }
                let Some(immediate) = next_non_ws(tokens, initially) else {
                    continue;
                };
                if word_is(&tokens[immediate], "immediate") {
                    remove_phrase(tokens, sql, edits, index, immediate, "not deferrable initially immediate is the default");
                }
            }
            "on" => {
                let Some(commit) = next_non_ws(tokens, index) else {
                    continue;
                };
                if !word_is(&tokens[commit], "commit") {
                    continue;
                }
                let Some(preserve) = next_non_ws(tokens, commit) else {
                    continue;
                };
                if !word_is(&tokens[preserve], "preserve") {
                    continue;
                }
                let Some(rows) = next_non_ws(tokens, preserve) else {
                    continue;
                };
                if word_is(&tokens[rows], "rows") {
                    remove_phrase(tokens, sql, edits, index, rows, "on commit preserve rows is the default");
                }
            }
            "storage" | "compression" => {
                let Some(default) = next_non_ws(tokens, index) else {
                    continue;
                };
                if !word_is(&tokens[default], "default") {
                    continue;
                }
                if previous_non_ws(tokens, index).is_some_and(|previous| word_is(&tokens[previous], "set")) {
                    continue;
                }
                remove_phrase(tokens, sql, edits, index, default, "default column storage is redundant");
            }
            "without" => {
                let Some(oids) = next_non_ws(tokens, index) else {
                    continue;
                };
                if word_is(&tokens[oids], "oids") {
                    remove_phrase(tokens, sql, edits, index, oids, "without oids is the default");
                }
            }
            "with" => {
                let Some(data) = next_non_ws(tokens, index) else {
                    continue;
                };
                if word_is(&tokens[data], "data") && ctas_with_data(tokens, &flavors, index, data) {
                    remove_phrase(tokens, sql, edits, index, data, "with data is the default");
                }
            }
            "null" if column_null_constraint(tokens, &flavors, index) => {
                remove_phrase(tokens, sql, edits, index, index, "explicit null column constraint is redundant");
            }
            _ => {}
        }
    }
}

fn remove_phrase(tokens: &[Token], sql: &str, edits: &mut Vec<Edit>, first: usize, last: usize, message: &'static str) {
    let Some(previous) = previous_non_ws(tokens, first) else {
        return;
    };
    push_gap(edits, sql, token_end(&tokens[previous]), token_end(&tokens[last]), "", message);
}

fn ctas_with_data(tokens: &[Token], flavors: &[(Span, StatementFlavor)], with_index: usize, data_index: usize) -> bool {
    let offset = tokens[with_index].start;
    if flavor_at(flavors, offset) != StatementFlavor::CreateTable {
        return false;
    }
    let Some(statement) = statement_at(flavors, offset) else {
        return false;
    };
    if depth_before(tokens, statement, with_index) != 0 {
        return false;
    }
    let has_as = tokens[..with_index]
        .iter()
        .any(|token| token.kind == Kind::Word && token.start >= statement.start && token.text.eq_ignore_ascii_case("as"));
    if !has_as {
        return false;
    }
    match next_non_ws(tokens, data_index) {
        None => true,
        Some(next) => punct(&tokens[next], ";") || tokens[next].start >= statement.end,
    }
}

fn column_null_constraint(tokens: &[Token], flavors: &[(Span, StatementFlavor)], index: usize) -> bool {
    let offset = tokens[index].start;
    let Some(statement) = statement_at(flavors, offset) else {
        return false;
    };
    let depth = depth_before(tokens, statement, index);
    match flavor_at(flavors, offset) {
        StatementFlavor::CreateTable if depth == 1 => {}
        StatementFlavor::AlterTable if depth == 0 => {}
        _ => return false,
    }
    let Some(previous) = previous_non_ws(tokens, index) else {
        return false;
    };
    match tokens[previous].kind {
        Kind::Word => {
            let lower = tokens[previous].text.to_ascii_lowercase();
            if matches!(
                lower.as_str(),
                "select"
                    | "where"
                    | "and"
                    | "or"
                    | "on"
                    | "having"
                    | "when"
                    | "then"
                    | "else"
                    | "returning"
                    | "values"
                    | "in"
                    | "is"
                    | "not"
                    | "default"
                    | "set"
                    | "as"
                    | "from"
                    | "by"
                    | "order"
                    | "group"
                    | "limit"
                    | "coalesce"
                    | "nullif"
                    | "null"
                    | "constraint"
                    | "cast"
                    | "using"
                    | "into"
                    | "case"
                    | "union"
                    | "intersect"
                    | "except"
            ) {
                return false;
            }
        }
        Kind::QuotedIdent => {}
        Kind::Punct if matches!(tokens[previous].text, ")" | "]") => {}
        _ => return false,
    }
    if let Some(before) = previous_non_ws(tokens, previous)
        && word_is(&tokens[before], "constraint")
    {
        return false;
    }
    match next_non_ws(tokens, index) {
        None => true,
        Some(next) => {
            punct(&tokens[next], ",")
                || punct(&tokens[next], ")")
                || (tokens[next].kind == Kind::Word
                    && matches!(
                        tokens[next].text.to_ascii_lowercase().as_str(),
                        "not" | "default" | "unique" | "primary" | "references" | "check" | "constraint" | "collate" | "storage" | "compression" | "generated"
                    ))
        }
    }
}

/// R35：PostgreSQL 类型字面量改写成强转。
fn typed_literal_edits(tokens: &[Token], sql: &str, edits: &mut Vec<Edit>) {
    let mut index = 0;
    while index < tokens.len() {
        if tokens[index].kind != Kind::Word {
            index += 1;
            continue;
        }
        let Some((string_index, type_name)) = typed_literal_at(tokens, sql, index) else {
            index += 1;
            continue;
        };
        if tokens[string_index].start <= token_end(&tokens[index]) {
            index += 1;
            continue;
        }
        if previous_non_ws(tokens, index).is_some_and(|previous| punct(&tokens[previous], ".") || punct(&tokens[previous], "::")) {
            index += 1;
            continue;
        }
        let string = &tokens[string_index];
        let body = if type_name.starts_with("interval") {
            normalize_interval_literal(string.text).unwrap_or_else(|| string.text.to_owned())
        } else {
            string.text.to_owned()
        };
        edits.push(Edit {
            span: Span::new(tokens[index].start, token_end(string)),
            replacement: format!("{body}::{type_name}"),
            message: "postgres typed literal should use a cast",
        });
        index = string_index + 1;
    }
}

fn typed_literal_at(tokens: &[Token], sql: &str, index: usize) -> Option<(usize, String)> {
    let lower = tokens[index].text.to_ascii_lowercase();
    match lower.as_str() {
        "date" | "timetz" | "timestamptz" | "interval" => literal_after(tokens, index, &lower),
        "time" | "timestamp" => {
            let mut last = index;
            let mut modifier = String::new();
            let next = next_non_ws(tokens, index)?;
            let mut cursor = next;
            if punct(&tokens[next], "(") {
                let close = matching_paren(tokens, next)?;
                modifier = sql[tokens[next].start..token_end(&tokens[close])].to_owned();
                last = close;
                cursor = next_non_ws(tokens, close)?;
            }
            let (name, last) = if word_is(&tokens[cursor], "with") || word_is(&tokens[cursor], "without") {
                let with = word_is(&tokens[cursor], "with");
                let time = next_non_ws(tokens, cursor)?;
                if !word_is(&tokens[time], "time") {
                    return None;
                }
                let zone = next_non_ws(tokens, time)?;
                if !word_is(&tokens[zone], "zone") {
                    return None;
                }
                let name = match (lower.as_str(), with) {
                    ("timestamp", true) => "timestamptz",
                    ("timestamp", false) => "timestamp",
                    (_, true) => "timetz",
                    (_, false) => "time",
                };
                (name, zone)
            } else {
                (lower.as_str(), last)
            };
            let string = next_non_ws(tokens, last)?;
            if tokens[string].kind != Kind::String {
                return None;
            }
            Some((string, format!("{name}{modifier}")))
        }
        _ => None,
    }
}

fn literal_after(tokens: &[Token], type_index: usize, name: &str) -> Option<(usize, String)> {
    let string = next_non_ws(tokens, type_index)?;
    if tokens[string].kind != Kind::String {
        return None;
    }
    Some((string, name.to_owned()))
}

/// R35：`'…'::interval` 里的单位词小写单数。
fn interval_cast_edits(tokens: &[Token], edits: &mut Vec<Edit>) {
    for (index, token) in tokens.iter().enumerate() {
        if token.kind != Kind::String {
            continue;
        }
        let Some(cast) = next_non_ws(tokens, index) else {
            continue;
        };
        if !punct(&tokens[cast], "::") {
            continue;
        }
        let Some(ty) = next_non_ws(tokens, cast) else {
            continue;
        };
        if !word_is(&tokens[ty], "interval") {
            continue;
        }
        let Some(normalized) = normalize_interval_literal(token.text) else {
            continue;
        };
        edits.push(Edit { span: span(token), replacement: normalized, message: "interval units should be lowercase singular" });
    }
}

fn normalize_interval_literal(text: &str) -> Option<String> {
    let start = text.find('\'')?;
    let end = text.rfind('\'')?;
    if end <= start {
        return None;
    }
    let body = &text[start + 1..end];
    let normalized = normalize_interval_body(body);
    if normalized == body {
        return None;
    }
    Some(format!("{}{}{}", &text[..=start], normalized, &text[end..]))
}

fn normalize_interval_body(body: &str) -> String {
    let trimmed = body.trim_start();
    let bytes = trimmed.as_bytes();
    let iso = matches!(bytes.first(), Some(b'P' | b'p')) && bytes.get(1).is_some_and(u8::is_ascii_digit);
    if iso {
        return body.to_owned();
    }
    let mut out = String::with_capacity(body.len());
    let mut index = 0;
    while index < body.len() {
        let ch = body[index..].chars().next().expect("char boundary");
        if ch.is_ascii_alphabetic() {
            let start = index;
            index += 1;
            while index < body.len() && body.as_bytes()[index].is_ascii_alphabetic() {
                index += 1;
            }
            let word = &body[start..index];
            let lower = word.to_ascii_lowercase();
            let attached = start > 0 && body.as_bytes()[start - 1].is_ascii_digit();
            if attached {
                if let Some(unit) = interval_abbreviation(&lower) {
                    out.push(' ');
                    out.push_str(unit);
                } else if let Some(unit) = interval_unit(&lower) {
                    out.push_str(unit);
                } else {
                    out.push_str(word);
                }
            } else if let Some(unit) = interval_unit(&lower) {
                out.push_str(unit);
            } else {
                out.push_str(word);
            }
        } else {
            out.push(ch);
            index += ch.len_utf8();
        }
    }
    out
}

fn interval_unit(word: &str) -> Option<&'static str> {
    Some(match word {
        "year" | "years" => "year",
        "month" | "months" => "month",
        "week" | "weeks" => "week",
        "day" | "days" => "day",
        "hour" | "hours" | "hr" | "hrs" => "hour",
        "minute" | "minutes" | "min" | "mins" => "minute",
        "second" | "seconds" | "sec" | "secs" => "second",
        "millisecond" | "milliseconds" => "millisecond",
        "microsecond" | "microseconds" => "microsecond",
        "decade" | "decades" => "decade",
        "century" | "centuries" => "century",
        "millennium" | "millenniums" | "millennia" => "millennium",
        "quarter" | "quarters" => "quarter",
        _ => return None,
    })
}

fn interval_abbreviation(word: &str) -> Option<&'static str> {
    Some(match word {
        "y" | "yr" | "yrs" => "year",
        "mon" | "mons" => "month",
        "w" => "week",
        "d" => "day",
        "h" | "hr" | "hrs" => "hour",
        "m" | "min" | "mins" => "minute",
        "s" | "sec" | "secs" => "second",
        "ms" => "millisecond",
        "us" => "microsecond",
        _ => return None,
    })
}

/// R36：类型位置上的类型别名归一。
fn type_alias_edits(tokens: &[Token], sql: &str, edits: &mut Vec<Edit>) {
    let flavors = statement_flavors(tokens, sql);
    let mut index = 0;
    while index < tokens.len() {
        if tokens[index].kind != Kind::Word {
            index += 1;
            continue;
        }
        if previous_non_ws(tokens, index).is_some_and(|previous| word_is(&tokens[previous], "collate")) {
            index += 1;
            continue;
        }
        let Some((last, replacement)) = type_alias_at(tokens, sql, index) else {
            index += 1;
            continue;
        };
        if next_non_ws(tokens, last).is_some_and(|next| tokens[next].kind == Kind::String) {
            index = last + 1;
            continue;
        }
        if !type_context(tokens, &flavors, index) {
            index += 1;
            continue;
        }
        edits.push(Edit {
            span: Span::new(tokens[index].start, token_end(&tokens[last])),
            replacement,
            message: "use the canonical type name",
        });
        index = last + 1;
    }
}

fn type_alias_at(tokens: &[Token], sql: &str, index: usize) -> Option<(usize, String)> {
    let lower = tokens[index].text.to_ascii_lowercase();
    match lower.as_str() {
        "int4" | "integer" => Some((index, "int".to_owned())),
        "int8" => Some((index, "bigint".to_owned())),
        "int2" => Some((index, "smallint".to_owned())),
        "bool" => Some((index, "boolean".to_owned())),
        "numeric" => Some((index, "decimal".to_owned())),
        "real" => Some((index, "float4".to_owned())),
        "double" => {
            let next = next_non_ws(tokens, index)?;
            word_is(&tokens[next], "precision").then(|| (next, "float8".to_owned()))
        }
        "character" => {
            let next = next_non_ws(tokens, index);
            if let Some(next) = next {
                if word_is(&tokens[next], "varying") {
                    return Some((next, "varchar".to_owned()));
                }
                if word_is(&tokens[next], "set") || word_is(&tokens[next], "large") {
                    return None;
                }
            }
            character_terminates(tokens, next).then(|| (index, "char".to_owned()))
        }
        "char" => {
            let next = next_non_ws(tokens, index)?;
            word_is(&tokens[next], "varying").then(|| (next, "varchar".to_owned()))
        }
        "timestamp" | "time" => {
            let next = next_non_ws(tokens, index)?;
            let mut modifier = String::new();
            let mut cursor = next;
            if punct(&tokens[next], "(") {
                let close = matching_paren(tokens, next)?;
                let after = next_non_ws(tokens, close)?;
                if !word_is(&tokens[after], "with") && !word_is(&tokens[after], "without") {
                    return None;
                }
                modifier = sql[tokens[next].start..token_end(&tokens[close])].to_owned();
                cursor = after;
            }
            let with = if word_is(&tokens[cursor], "with") {
                true
            } else if word_is(&tokens[cursor], "without") {
                false
            } else {
                return None;
            };
            let time = next_non_ws(tokens, cursor)?;
            if !word_is(&tokens[time], "time") {
                return None;
            }
            let zone = next_non_ws(tokens, time)?;
            if !word_is(&tokens[zone], "zone") {
                return None;
            }
            let name = match (lower.as_str(), with) {
                ("timestamp", true) => "timestamptz",
                ("timestamp", false) => "timestamp",
                (_, true) => "timetz",
                (_, false) => "time",
            };
            Some((zone, format!("{name}{modifier}")))
        }
        _ => None,
    }
}

fn character_terminates(tokens: &[Token], next: Option<usize>) -> bool {
    let Some(next) = next else {
        return true;
    };
    let token = &tokens[next];
    if token.kind == Kind::Punct {
        return matches!(token.text, "(" | ")" | "," | "[" | "]" | ";");
    }
    if token.kind != Kind::Word {
        return false;
    }
    matches!(
        token.text.to_ascii_lowercase().as_str(),
        "not" | "null" | "default" | "primary" | "unique" | "references" | "check" | "constraint" | "collate" | "storage" | "compression" | "generated" | "using" | "with" | "array"
    )
}

fn type_context(tokens: &[Token], flavors: &[(Span, StatementFlavor)], index: usize) -> bool {
    if previous_non_ws(tokens, index).is_some_and(|previous| punct(&tokens[previous], "::")) {
        return true;
    }
    if in_cast_type(tokens, index) {
        return true;
    }
    match flavor_at(flavors, tokens[index].start) {
        StatementFlavor::CreateTable => in_create_table_column(tokens, flavors, index),
        StatementFlavor::AlterTable => in_alter_type(tokens, index),
        _ => false,
    }
}

fn in_cast_type(tokens: &[Token], index: usize) -> bool {
    let mut depth = 0;
    let mut seen_as = false;
    let mut probe = index;
    while probe > 0 {
        probe -= 1;
        let token = &tokens[probe];
        if token.kind == Kind::Punct {
            if token.text == ")" {
                depth += 1;
            } else if token.text == "(" {
                if depth == 0 {
                    if !seen_as {
                        return false;
                    }
                    let Some(name) = previous_non_ws(tokens, probe) else {
                        return false;
                    };
                    return tokens[name].kind == Kind::Word && (word_is(&tokens[name], "cast") || word_is(&tokens[name], "try_cast"));
                }
                depth -= 1;
            }
        } else if depth == 0 && word_is(token, "as") {
            seen_as = true;
        }
    }
    false
}

fn in_create_table_column(tokens: &[Token], flavors: &[(Span, StatementFlavor)], index: usize) -> bool {
    let Some(statement) = statement_at(flavors, tokens[index].start) else {
        return false;
    };
    if depth_before(tokens, statement, index) != 1 {
        return false;
    }
    let Some(name) = previous_non_ws(tokens, index) else {
        return false;
    };
    if !ident(&tokens[name]) {
        return false;
    }
    if tokens[name].kind == Kind::Word && non_type_leader(&tokens[name].text.to_ascii_lowercase()) {
        return false;
    }
    let Some(before) = previous_non_ws(tokens, name) else {
        return false;
    };
    punct(&tokens[before], "(") || punct(&tokens[before], ",")
}

fn in_alter_type(tokens: &[Token], index: usize) -> bool {
    let Some(previous) = previous_non_ws(tokens, index) else {
        return false;
    };
    if word_is(&tokens[previous], "type") {
        return true;
    }
    if !ident(&tokens[previous]) {
        return false;
    }
    if tokens[previous].kind == Kind::Word && matches!(tokens[previous].text.to_ascii_lowercase().as_str(), "column" | "add" | "type" | "rename" | "to" | "set") {
        return false;
    }
    let Some(before) = previous_non_ws(tokens, previous) else {
        return false;
    };
    word_is(&tokens[before], "column") || word_is(&tokens[before], "add")
}

fn non_type_leader(lower: &str) -> bool {
    matches!(
        lower,
        "all"
            | "and"
            | "as"
            | "between"
            | "by"
            | "case"
            | "cast"
            | "check"
            | "coalesce"
            | "constraint"
            | "cross"
            | "distinct"
            | "else"
            | "except"
            | "exclude"
            | "exists"
            | "foreign"
            | "from"
            | "full"
            | "group"
            | "having"
            | "in"
            | "inner"
            | "intersect"
            | "into"
            | "is"
            | "join"
            | "left"
            | "like"
            | "limit"
            | "not"
            | "null"
            | "nullif"
            | "on"
            | "or"
            | "order"
            | "partition"
            | "primary"
            | "returning"
            | "right"
            | "select"
            | "set"
            | "then"
            | "union"
            | "unique"
            | "using"
            | "values"
            | "when"
            | "where"
            | "with"
    )
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
                let valid_before = matches!(before.kind, Kind::Word | Kind::QuotedIdent) || (before.kind == Kind::Punct && before.text == ")");
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
        let tokens: Vec<Token> = tokens(element_text).into_iter().filter(|token| token.kind != Kind::Whitespace).collect();
        if tokens.is_empty() {
            continue;
        }
        if tokens.iter().any(|token| token.kind == Kind::Word && token.text.eq_ignore_ascii_case("into")) {
            continue;
        }
        // `IS JSON OBJECT` 的 OBJECT 是类型修饰，不是字段别名。
        if tokens.iter().any(|token| token.kind == Kind::Word && token.text.eq_ignore_ascii_case("is"))
            && tokens.iter().any(|token| token.kind == Kind::Word && token.text.eq_ignore_ascii_case("json"))
        {
            continue;
        }
        let trailing_ddl = tokens
            .iter()
            .any(|token| token.kind == Kind::Word && matches!(token.text.to_ascii_lowercase().as_str(), "with" | "without" | "tablespace" | "using"))
            || tokens
                .windows(2)
                .any(|pair| pair[0].kind == Kind::Word && pair[0].text.eq_ignore_ascii_case("on") && pair[1].kind == Kind::Word && pair[1].text.eq_ignore_ascii_case("commit"));
        if trailing_ddl {
            continue;
        }
        let has_as = tokens.iter().any(|token| token.kind == Kind::Word && token.text.eq_ignore_ascii_case("as"));
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
        if matches!(lower.as_str(), "end" | "null" | "true" | "false" | "distinct" | "all" | "unknown") {
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
                (previous.text.to_ascii_lowercase().as_str(), lower.as_str()),
                ("character", "varying") | ("char", "varying") | ("double", "precision") | ("time", "zone")
            )
        {
            continue;
        }
        if previous.kind == Kind::Word && matches!(previous.text.to_ascii_lowercase().as_str(), "distinct" | "all") {
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
    let functions = crate::sql::core::spans::function_paren_spans(sql);
    let cases = crate::sql::core::spans::outermost(crate::sql::core::spans::case_spans(sql));
    for case in cases {
        if functions.iter().any(|span| span.start <= case.start && case.end <= span.end) {
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

fn push_gap(edits: &mut Vec<Edit>, sql: &str, start: usize, end: usize, replacement: &str, message: &'static str) {
    if start > end || end > sql.len() {
        return;
    }
    let gap = &sql[start..end];
    if gap.contains('\n') || gap == replacement {
        return;
    }
    edits.push(Edit { span: Span::new(start, end), replacement: replacement.to_owned(), message });
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
            collect_list(&text[group.start..group.end], base + condition.start + group.start, wraps);
        }
        if has_top_level_between(text) && !is_wrapped(text) {
            wraps.push(Wrap { start: base + condition.start, end: base + condition.end });
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
                Kind::Word if depth == 0 && matches!(next.text.to_ascii_lowercase().as_str(), "join" | "left" | "right" | "full" | "inner" | "cross" | "natural") => {
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
        if depth == 0 && (index == 0 || !is_word(bytes[index - 1])) && lower.get(index..).is_some_and(|rest| rest.starts_with("between")) {
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
        assert_fixed("SELECT foo, bar INTO baz FROM qux;", "select foo, bar into baz from qux;");
        assert_fixed("SELECT TRUE IS NOT UNKNOWN;", "select true is not unknown;");
    }

    #[test]
    fn wraps_between_condition() {
        assert_fixed("select a from t where x between 1 and 2", "select a from t where (x between 1 and 2)");
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
        assert_fixed("select a from t where a = 1 and b between 2 and 3", "select a from t where a = 1 and (b between 2 and 3)");
    }

    #[test]
    fn wraps_inside_boolean_group() {
        assert_fixed("select a from t where (a = 1 or b between 2 and 3)", "select a from t where (a = 1 or (b between 2 and 3))");
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
        assert_fixed("SELECT A, 'Keep Me' AS B FROM T", "select a, 'Keep Me' as b from t");
    }

    #[test]
    fn rewrites_symbols_and_spacing() {
        assert_fixed("select a,b from t where x<>y", "select a, b from t where x != y");
        assert_fixed("select a from t;", "select a from t;");
        assert_fixed("select a from t;   ", "select a from t;");
    }

    #[test]
    fn spaces_insert_target_and_conflict() {
        assert_fixed(
            "INSERT INTO t AS d(did, name) VALUES(1, 'x') ON CONFLICT(did) DO NOTHING",
            "insert into t as d (did, name) values (1, 'x') on conflict (did) do nothing",
        );
    }

    #[test]
    fn spaces_cte_column_lists() {
        assert_fixed("WITH RECURSIVE t(n) AS (SELECT 1) SELECT * FROM t", "with recursive t (n) as (select 1) select * from t");
    }

    #[test]
    fn hugs_function_parentheses() {
        assert_fixed("select count (*) from t group by rollup (a, b)", "select count(*) from t group by rollup(a, b)");
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
        assert_fixed("select a from t where not b = 1", "select a from t where b != 1");
        assert_fixed("select a from t where not b = any(select c from u)", "select a from t where not b = any(select c from u)");
    }

    #[test]
    fn rewrites_not_is_null() {
        assert_fixed("select a from t where not b is null", "select a from t where b is not null");
    }

    #[test]
    fn adds_field_alias_as() {
        assert_fixed("select sum(a) total from t", "select sum(a) as total from t");
        assert_fixed("select a b from t", "select a as b from t");
        assert_fixed("select distinct a from t", "select distinct a from t");
    }

    #[test]
    fn removes_table_alias_as() {
        assert_fixed("select a from t as x, (select 1) as y", "select a from t x, (select 1) y");
    }

    #[test]
    fn parenthesizes_standalone_case() {
        assert_fixed("select case when a = 1 then 1 else 0 end as f from t", "select (case when a = 1 then 1 else 0 end) as f from t");
        assert_fixed("select sum(case when a = 1 then 1 else 0 end) as f from t", "select sum(case when a = 1 then 1 else 0 end) as f from t");
    }

    #[test]
    fn rewrites_pg_typed_literals() {
        assert_fixed(
            "SELECT DATE '2020-01-01' AS d, TIME '12:30' AS t, TIMESTAMP WITH TIME ZONE '2020-01-01 00:00' AS z",
            "select '2020-01-01'::date as d, '12:30'::time as t, '2020-01-01 00:00'::timestamptz as z",
        );
        assert_fixed(
            "select timestamptz '2020-01-01 00:00', interval '8 HOURS'",
            "select '2020-01-01 00:00'::timestamptz, '8 hour'::interval",
        );
        assert_fixed("select timestamp(3) with time zone '2020-01-01'", "select '2020-01-01'::timestamptz(3)");
    }

    #[test]
    fn typed_literals_are_postgres_only() {
        use crate::sql::lexer::with_dialect;
        use crate::sql::parse::Dialect;

        with_dialect(Dialect::Mysql, || {
            assert_eq!(normalized("SELECT DATE '2020-01-01'"), "select date '2020-01-01'");
            assert_eq!(normalized("select interval '8 HOURS'"), "select interval '8 HOURS'");
            assert_eq!(normalized("select '3S'::interval"), "select '3S'::interval");
        });
        with_dialect(Dialect::Postgres, || {
            assert_eq!(normalized("SELECT DATE '2020-01-01'"), "select '2020-01-01'::date");
            assert_eq!(normalized("select interval '8 HOURS'"), "select '8 hour'::interval");
        });
    }

    #[test]
    fn normalizes_interval_units() {
        assert_fixed("select interval '1 HOUR 2 MINUTES'", "select '1 hour 2 minute'::interval");
        assert_fixed("select '3S'::interval", "select '3 second'::interval");
        assert_fixed("select interval '1 day 2 HOURS'", "select '1 day 2 hour'::interval");
        assert_fixed("select interval 'P1D'", "select 'P1D'::interval");
    }

    #[test]
    fn preserves_literals_and_quoted_identifiers() {
        let sql = "select 'Luso Films' as title, \"C\" as c from films";
        assert_eq!(normalized(sql), sql);
        assert_eq!(normalized("select '8 HOURS' as x"), "select '8 HOURS' as x");
    }

    #[test]
    fn canonicalizes_type_aliases() {
        assert_fixed(
            "create table t (a integer primary key, b int8, c bool, d numeric(10, 2), e real, f double precision, g timestamp with time zone, h character varying(20), i character)",
            "create table t (a int primary key, b bigint, c boolean, d decimal(10, 2), e float4, f float8, g timestamptz, h varchar(20), i char)",
        );
        assert_fixed("create table t (a timestamp(3), b numeric (10, 2))", "create table t (a timestamp(3), b decimal(10, 2))");
        assert_fixed(
            "select a::int4, cast(b as double precision), c::character varying, d::timestamp without time zone, e::time with time zone from t",
            "select a::int, cast(b as float8), c::varchar, d::timestamp, e::timetz from t",
        );
    }

    #[test]
    fn keeps_collation_names() {
        assert_fixed("create table t (a text collate numeric, b numeric)", "create table t (a text collate numeric, b decimal)");
        assert_fixed("select a from t order by a collate numeric", "select a from t order by a collate numeric");
    }

    #[test]
    fn keeps_calls_named_like_types() {
        assert_fixed("select varchar(a), numeric(b), int4(c) from t", "select varchar(a), numeric(b), int4(c) from t");
    }

    #[test]
    fn keeps_distinct_constraint_parens() {
        assert_fixed("create table t (fld int, unique nulls not distinct (fld))", "create table t (fld int, unique nulls not distinct (fld))");
        assert_fixed("create unique index i on t (fld) nulls not distinct", "create unique index i on t (fld) nulls not distinct");
        assert_fixed("select distinct(a) from t", "select distinct a from t");
    }

    #[test]
    fn hugs_named_arguments() {
        assert_fixed("create table t (a int) with (fillfactor = 70)", "create table t (a int) with (fillfactor=70)");
        assert_fixed("alter table t set (parameter_1 = 'value')", "alter table t set (parameter_1='value')");
        assert_fixed("select foo(a = 1, b = 2) from t", "select foo(a=1, b=2) from t");
        assert_fixed("alter table t set (opt1, opt2=5, ns.opt5=6, opt7=asc)", "alter table t set (opt1, opt2=5, ns.opt5=6, opt7=asc)");
        assert_fixed("alter table t set (opt1, opt2 = 5, ns.opt5 = 6, opt7 = asc)", "alter table t set (opt1, opt2=5, ns.opt5=6, opt7=asc)");
        assert_fixed("create table t (a int) with (arg1=3, arg5='str')", "create table t (a int) with (arg1=3, arg5='str')");
        assert_fixed("select a from t where t.a = 1", "select a from t where t.a = 1");
        assert_fixed("select a from t where a = 1 and b = 2", "select a from t where a = 1 and b = 2");
    }

    #[test]
    fn spaces_keyword_parentheses_and_hugs_calls() {
        assert_fixed("select a from t where a in(1, 2, 3)", "select a from t where a in (1, 2, 3)");
        assert_fixed(
            "create table t (a int check(a > 0), unique(a), b varchar (10))",
            "create table t (a int check (a > 0), unique (a), b varchar(10))",
        );
        assert_fixed(
            "create table t (a int, primary key(a), foreign key(a) references u(a))",
            "create table t (a int, primary key (a), foreign key (a) references u (a))",
        );
        assert_fixed("create index i on t using gin(a) include(b)", "create index i on t using gin (a) include (b)");
        assert_fixed("explain(analyze true, buffers false) select 1", "explain (analyze true, buffers false) select 1");
        assert_fixed("select left(lower(name), 1), right(code, 2) from t", "select left(lower(name), 1), right(code, 2) from t");
        assert_fixed("create table t (a int default (1 + 2))", "create table t (a int default (1 + 2))");
        assert_fixed(
            "create table t (a int generated always as identity (increment 4))",
            "create table t (a int generated always as identity (increment 4))",
        );
        assert_fixed(
            "create table t (during tsrange, exclude using gist (during with =, room with &&))",
            "create table t (during tsrange, exclude using gist (during with =, room with &&))",
        );
    }

    #[test]
    fn omits_default_forms() {
        assert_fixed(
            "create table t (a int null, b int not null, c int default null)",
            "create table t (a int, b int not null, c int default null)",
        );
        assert_fixed("select a from t order by a asc nulls last", "select a from t order by a");
        assert_fixed("select a from t order by a desc nulls first", "select a from t order by a desc");
        assert_fixed("create index i on t (a asc)", "create index i on t (a)");
        assert_fixed(
            "create table t (a int references u (b) match simple not deferrable initially immediate)",
            "create table t (a int references u (b))",
        );
        assert_fixed("create table t (a int) on commit preserve rows", "create table t (a int)");
        assert_fixed("create table t (a text storage default, b text compression default)", "create table t (a text, b text)");
        assert_fixed("create table t (a int) without oids", "create table t (a int)");
        assert_fixed("create table t as select 1 with data", "create table t as select 1");
    }

    #[test]
    fn keeps_non_default_forms() {
        for sql in [
            "select a from t order by a nulls first",
            "select a from t order by a desc nulls last",
            "create table t (a int references u (b) match full)",
            "create table t (a int references u (b) deferrable initially deferred)",
            "create table t (a int) on commit delete rows",
            "create table t as select 1 with no data",
            "alter table t alter column a set storage default",
            "create unique index i on t (a) nulls not distinct",
        ] {
            assert_eq!(normalized(sql), sql);
        }
    }

    #[test]
    fn keeps_null_outside_column_defaults() {
        for sql in [
            "select null as x from t",
            "select a from t where a is null",
            "update t set a = null where b = 1",
            "create table t (a int default null)",
            "create table t (a int not null)",
        ] {
            assert_eq!(normalized(sql), sql);
        }
    }
}
