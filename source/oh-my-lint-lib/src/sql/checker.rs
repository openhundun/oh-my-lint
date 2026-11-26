//! 断行检查：只产出违规与定位，不做任何文本修改。

/// 期望的版式。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    Break,
    Inline,
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Break => "break",
            Self::Inline => "inline",
        }
    }
}

/// 一条断行违规；`span` 是待替换 / 待检查的正文区间。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Violation {
    pub kind: &'static str,
    pub element_count: usize,
    pub expected: Mode,
    pub span: crate::span::Span,
    pub indent: usize,
    pub next_indent: Option<usize>,
    pub message: String,
}

const BREAK_KINDS: [&str; 6] = ["select", "from", "where", "having", "group by", "order by"];

/// 按指定方言检查；结构优先来自 AST，解析失败时回退到文本扫描。
pub fn violations_with(sql: &str, dialect: crate::sql::parse::Dialect) -> Vec<Violation> {
    crate::sql::lexer::with_dialect(dialect, || violations_inner(sql, dialect))
}

fn violations_inner(sql: &str, dialect: crate::sql::parse::Dialect) -> Vec<Violation> {
    let structure = crate::sql::astmap::structure(sql, dialect);
    let mut found = Vec::new();
    for statement in crate::sql::splitter::statements(sql) {
        let statement_text = &sql[statement.start..statement.end];
        let blocks = structure
            .as_ref()
            .and_then(|structure| structure.blocks(statement, statement_text))
            .unwrap_or_else(|| crate::sql::splitter::all_statements(statement_text));
        for (index, block_span) in blocks.into_iter().enumerate() {
            let block = &statement_text[block_span.start..block_span.end];
            let is_root = block_span.start == 0 && block_span.end == statement_text.len();
            if !is_root
                && !crate::sql::splitter::has_top_level_newline(block)
                && !crate::sql::decision::is_simple(block)
            {
                let count = crate::sql::splitter::clause_spans(block).len();
                found.push(Violation {
                    kind: "subquery",
                    element_count: count,
                    expected: Mode::Break,
                    span: crate::span::Span::new(
                        statement.start + block_span.start,
                        statement.start + block_span.end,
                    ),
                    indent: line_indent(statement_text, block_span.start),
                    next_indent: None,
                    message: format!("subquery n={count} expected=break"),
                });
                continue;
            }
            let _ = index;
            check_block(statement.start, statement_text, block_span, &mut found);
        }
    }
    found
}

/// 检查一个语句块：集合运算分支、`with` 框架与普通子句。
fn check_block(
    statement_start: usize,
    statement_text: &str,
    block: crate::span::Span,
    found: &mut Vec<Violation>,
) {
    let text = &statement_text[block.start..block.end];
    if starts_with_word(text, "with") {
        check_with_frame(statement_start, statement_text, block, found);
    }
    let clauses = crate::sql::splitter::clause_spans(text);
    if clauses
        .iter()
        .any(|clause| crate::sql::splitter::SET_OPERATORS.contains(&clause.kind))
    {
        check_set_ops(statement_start, statement_text, block, &clauses, found);
        return;
    }
    check_clauses(
        statement_start,
        statement_text,
        block.start,
        block.end,
        found,
    );
}

/// 子句级检查：断行期望、元素内的布尔组与 `case`、`from` 的 join 版式。
fn check_clauses(
    statement_start: usize,
    statement_text: &str,
    base: usize,
    end: usize,
    found: &mut Vec<Violation>,
) {
    let text = &statement_text[base..end];
    let clauses = crate::sql::splitter::clause_spans(text);
    if clauses.first().map(|clause| clause.kind) != Some("select") {
        return;
    }
    let block_indent = block_indent(statement_text, base);
    if !crate::sql::decision::is_simple(text) {
        for clause in clauses.iter().skip(1) {
            let relative = base + clause.keyword_start;
            let line_start = statement_text[..relative]
                .rfind('\n')
                .map_or(0, |index| index + 1);
            let prefix = &statement_text[line_start..relative];
            if prefix
                .chars()
                .all(|character| character == ' ' || character == '\t')
            {
                continue;
            }
            let mut start = relative;
            let bytes = statement_text.as_bytes();
            while start > 0 && matches!(bytes[start - 1], b' ' | b'\t') {
                start -= 1;
            }
            found.push(Violation {
                kind: "clause-start",
                element_count: 0,
                expected: Mode::Break,
                span: crate::span::Span::new(statement_start + start, statement_start + relative),
                indent: block_indent,
                next_indent: None,
                message: format!("{} should start a new line", clause.kind),
            });
        }
    }
    for clause in &clauses {
        if !BREAK_KINDS.contains(&clause.kind) {
            continue;
        }
        let body = &text[clause.body_start..clause.body_end];
        let elements = crate::sql::splitter::split_elements(clause.kind, body);
        if elements.is_empty() {
            continue;
        }
        let has_join = clause.kind == "from" && crate::sql::decision::has_join(body);
        if !has_join {
            let expected = if crate::sql::decision::needs_break(clause.kind, &elements) {
                Mode::Break
            } else {
                Mode::Inline
            };
            let actual = crate::sql::splitter::has_top_level_newline(body.trim());
            if (expected == Mode::Break) != actual {
                found.push(Violation {
                    kind: clause.kind,
                    element_count: elements.len(),
                    expected,
                    span: crate::span::Span::new(
                        statement_start + base + clause.body_start,
                        statement_start + base + clause.body_end,
                    ),
                    indent: line_indent(statement_text, base + clause.keyword_start),
                    next_indent: trailing_indent(body),
                    message: format!(
                        "{} n={} expected={}",
                        clause.kind,
                        elements.len(),
                        expected.as_str()
                    ),
                });
            }
        }
        check_groups(
            statement_start,
            statement_text,
            base + clause.body_start,
            body,
            found,
        );
        check_cases(
            statement_start,
            statement_text,
            base + clause.body_start,
            body,
            found,
        );
        if clause.kind == "from" {
            check_join(statement_start, statement_text, base, clause, body, found);
        }
        if clause.kind == "group by" {
            check_rollup(
                statement_start,
                statement_text,
                base + clause.body_start,
                body,
                found,
            );
        }
    }
}

/// 布尔组：>3 个条件或含复杂条件则逐条件断行。
fn check_groups(
    statement_start: usize,
    statement_text: &str,
    body_base: usize,
    body: &str,
    found: &mut Vec<Violation>,
) {
    let nested: Vec<crate::span::Span> = crate::sql::splitter::all_statements(body)
        .into_iter()
        .skip(1)
        .collect();
    for group in crate::sql::splitter::condition_group_spans(body) {
        if nested
            .iter()
            .any(|block| block.start <= group.start && group.end <= block.end)
        {
            continue;
        }
        let inner = &body[group.start..group.end];
        let conditions = crate::sql::splitter::split_elements("where", inner);
        if conditions.is_empty() {
            continue;
        }
        let expected = if conditions.len() > 3
            || conditions
                .iter()
                .any(|condition| crate::sql::decision::is_complex(&condition.text, "where"))
        {
            Mode::Break
        } else {
            Mode::Inline
        };
        let actual = crate::sql::splitter::has_top_level_newline(inner);
        if (expected == Mode::Break) == actual {
            continue;
        }
        found.push(Violation {
            kind: "group",
            element_count: conditions.len(),
            expected,
            span: crate::span::Span::new(
                statement_start + body_base + group.start,
                statement_start + body_base + group.end,
            ),
            indent: line_indent(statement_text, body_base + group.start.saturating_sub(1)),
            next_indent: None,
            message: format!(
                "group n={} expected={}",
                conditions.len(),
                expected.as_str()
            ),
        });
    }
}

/// 独立 `case`：分支复杂则展开；函数实参内的 `case` 不展开。
fn check_cases(
    statement_start: usize,
    statement_text: &str,
    body_base: usize,
    body: &str,
    found: &mut Vec<Violation>,
) {
    let nested: Vec<crate::span::Span> = crate::sql::splitter::all_statements(body)
        .into_iter()
        .skip(1)
        .collect();
    let functions = function_paren_spans(body);
    let cases = outermost(case_spans(body));
    for case in cases {
        if nested
            .iter()
            .any(|block| block.start <= case.start && case.end <= block.end)
            || functions
                .iter()
                .any(|span| span.start <= case.start && case.end <= span.end)
        {
            continue;
        }
        let text = &body[case.start..case.end];
        let expected = if crate::sql::decision::branches_complex(text) {
            Mode::Break
        } else {
            Mode::Inline
        };
        let actual = crate::sql::splitter::has_top_level_newline(text);
        if (expected == Mode::Break) == actual {
            continue;
        }
        found.push(Violation {
            kind: "case",
            element_count: 0,
            expected,
            span: crate::span::Span::new(
                statement_start + body_base + case.start,
                statement_start + body_base + case.end,
            ),
            indent: line_indent(statement_text, body_base + case.start),
            next_indent: None,
            message: format!("case expected={}", expected.as_str()),
        });
    }
}

/// `from` 含 join：首表留在关键字行、每个 join 独占一行、显式 join 后不写逗号。
fn check_join(
    statement_start: usize,
    statement_text: &str,
    base: usize,
    clause: &crate::sql::splitter::Clause,
    body: &str,
    found: &mut Vec<Violation>,
) {
    if !crate::sql::decision::has_join(body) {
        return;
    }
    let from_indent = line_indent(statement_text, base + clause.keyword_start);
    let body_base = base + clause.body_start;
    if body.trim_start_matches([' ', '\t']).starts_with('\n') {
        found.push(Violation {
            kind: "from-join",
            element_count: 0,
            expected: Mode::Break,
            span: crate::span::Span::new(
                statement_start + body_base,
                statement_start + body_base + body.len(),
            ),
            indent: from_indent,
            next_indent: None,
            message: "from should keep the first table on the keyword line".to_owned(),
        });
    }
    let tokens = crate::sql::lexer::tokens(body);
    let mut depth = 0;
    for token in &tokens {
        match token.kind {
            crate::sql::lexer::Kind::Punct if token.text == "(" => depth += 1,
            crate::sql::lexer::Kind::Punct if token.text == ")" => depth -= 1,
            crate::sql::lexer::Kind::Punct if token.text == "," && depth == 0 => {
                found.push(Violation {
                    kind: "cross-join",
                    element_count: 0,
                    expected: Mode::Break,
                    span: crate::span::Span::new(
                        statement_start + body_base + token.start,
                        statement_start + body_base + token.start + 1,
                    ),
                    indent: from_indent,
                    next_indent: None,
                    message: "implicit cross join should be written as cross join".to_owned(),
                });
            }
            crate::sql::lexer::Kind::Word
                if depth == 0 && token.text.eq_ignore_ascii_case("join") =>
            {
                let absolute = body_base + token.start;
                let line_start = statement_text[..absolute]
                    .rfind('\n')
                    .map_or(0, |index| index + 1);
                let prefix = &statement_text[line_start..absolute];
                let leading = prefix.len() - prefix.trim_start().len();
                let mut clause_start = absolute;
                for (offset, word) in prefix_words(prefix) {
                    if crate::sql::splitter::is_join_modifier(word) {
                        clause_start = line_start + offset;
                    } else {
                        break;
                    }
                }
                let at_line_start = prefix[..clause_start - line_start].trim().is_empty();
                if at_line_start {
                    if leading == from_indent {
                        continue;
                    }
                    clause_start = line_start;
                }
                found.push(Violation {
                    kind: "from-join",
                    element_count: 0,
                    expected: Mode::Break,
                    span: crate::span::Span::new(
                        statement_start + clause_start,
                        statement_start + absolute + token.text.len(),
                    ),
                    indent: from_indent,
                    next_indent: None,
                    message: "join should start a new line".to_owned(),
                });
            }
            _ => {}
        }
    }
}

/// 前缀中的词及字节偏移；从右往左用于识别 join 修饰词。
fn prefix_words(prefix: &str) -> Vec<(usize, &str)> {
    let mut words = Vec::new();
    let mut start = 0usize;
    for (index, ch) in prefix.char_indices() {
        if ch.is_whitespace() {
            if start < index {
                words.push((start, &prefix[start..index]));
            }
            start = index + ch.len_utf8();
        }
    }
    if start < prefix.len() {
        words.push((start, &prefix[start..]));
    }
    words.reverse();
    words
}

/// `rollup(...)` 参数列表：≥4 或含复杂表达式时逐元素一行。
fn check_rollup(
    statement_start: usize,
    statement_text: &str,
    body_base: usize,
    body: &str,
    found: &mut Vec<Violation>,
) {
    let tokens = crate::sql::lexer::tokens(body);
    for (index, token) in tokens.iter().enumerate() {
        if token.kind != crate::sql::lexer::Kind::Word || !token.text.eq_ignore_ascii_case("rollup")
        {
            continue;
        }
        let mut probe = index + 1;
        while probe < tokens.len() && tokens[probe].kind == crate::sql::lexer::Kind::Whitespace {
            probe += 1;
        }
        if probe >= tokens.len() || tokens[probe].text != "(" {
            continue;
        }
        let open = tokens[probe].start;
        let mut depth = 0;
        let mut close = None;
        for next in &tokens[probe..] {
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
        let inner = &body[open + 1..close];
        let args = crate::sql::splitter::split_elements("group by", inner);
        if args.is_empty() {
            continue;
        }
        let expected = if args.len() >= 4
            || args
                .iter()
                .any(|argument| crate::sql::decision::is_complex(&argument.text, "group by"))
        {
            Mode::Break
        } else {
            Mode::Inline
        };
        let actual = crate::sql::splitter::has_top_level_newline(inner);
        if (expected == Mode::Break) == actual {
            continue;
        }
        found.push(Violation {
            kind: "rollup",
            element_count: args.len(),
            expected,
            span: crate::span::Span::new(
                statement_start + body_base + open + 1,
                statement_start + body_base + close,
            ),
            indent: line_indent(statement_text, body_base + token.start),
            next_indent: None,
            message: format!("rollup n={} expected={}", args.len(), expected.as_str()),
        });
    }
}

/// 集合运算：运算符独占一行、分支按独立语句块判定、分支不加括号。
fn check_set_ops(
    statement_start: usize,
    statement_text: &str,
    block: crate::span::Span,
    clauses: &[crate::sql::splitter::Clause],
    found: &mut Vec<Violation>,
) {
    let text = &statement_text[block.start..block.end];
    let block_indent = block_indent(statement_text, block.start);
    let operators: Vec<&crate::sql::splitter::Clause> = clauses
        .iter()
        .filter(|clause| crate::sql::splitter::SET_OPERATORS.contains(&clause.kind))
        .collect();
    for operator in &operators {
        let relative = block.start + operator.keyword_start;
        let line_start = statement_text[..relative]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        let prefix = &statement_text[line_start..relative];
        if prefix
            .chars()
            .all(|character| character == ' ' || character == '\t')
            && prefix.len() == block_indent
        {
            continue;
        }
        let mut start = relative;
        let bytes = statement_text.as_bytes();
        while start > 0 && matches!(bytes[start - 1], b' ' | b'\t') {
            start -= 1;
        }
        found.push(Violation {
            kind: "set-op",
            element_count: 0,
            expected: Mode::Break,
            span: crate::span::Span::new(
                statement_start + start,
                statement_start + relative + operator.kind.len(),
            ),
            indent: block_indent,
            next_indent: None,
            message: format!("{} should start a new line", operator.kind),
        });
    }
    let mut bounds: Vec<(usize, usize)> = Vec::new();
    let mut start = 0;
    for operator in &operators {
        bounds.push((start, operator.keyword_start));
        start = operator.body_start;
    }
    bounds.push((start, text.len()));
    for (index, (branch_start, branch_end)) in bounds.into_iter().enumerate() {
        let branch = &text[branch_start..branch_end];
        let trimmed = branch.trim();
        if trimmed.is_empty() {
            continue;
        }
        let leading = branch.len() - branch.trim_start().len();
        let trailing = branch.trim_end().len();
        let absolute_start = block.start + branch_start + leading;
        if index > 0 {
            let line_start = statement_text[..absolute_start]
                .rfind('\n')
                .map_or(0, |position| position + 1);
            let prefix = &statement_text[line_start..absolute_start];
            if !(prefix
                .chars()
                .all(|character| character == ' ' || character == '\t')
                && prefix.len() == block_indent)
            {
                let bytes = statement_text.as_bytes();
                let mut start = absolute_start;
                while start > 0 && matches!(bytes[start - 1], b' ' | b'\t') {
                    start -= 1;
                }
                found.push(Violation {
                    kind: "set-op-branch",
                    element_count: 0,
                    expected: Mode::Break,
                    span: crate::span::Span::new(
                        statement_start + start,
                        statement_start + absolute_start,
                    ),
                    indent: block_indent,
                    next_indent: None,
                    message: "branch should start a new line".to_owned(),
                });
            }
        }
        if trimmed.starts_with('(') && trimmed.ends_with(')') {
            found.push(Violation {
                kind: "set-op-parens",
                element_count: 0,
                expected: Mode::Inline,
                span: crate::span::Span::new(
                    statement_start + absolute_start,
                    statement_start + block.start + branch_start + trailing,
                ),
                indent: block_indent,
                next_indent: None,
                message: "set operation branch should not be parenthesized".to_owned(),
            });
            continue;
        }
        check_clauses(
            statement_start,
            statement_text,
            absolute_start,
            block.start + branch_start + trailing,
            found,
        );
    }
}

/// `with` 框架：`with` 独行、CTE 展开时闭括号与名字同列、主查询另起一行。
fn check_with_frame(
    statement_start: usize,
    statement_text: &str,
    block: crate::span::Span,
    found: &mut Vec<Violation>,
) {
    let text = &statement_text[block.start..block.end];
    let indent = block_indent(statement_text, block.start);
    let tokens = crate::sql::lexer::tokens(text);
    let Some(first) = tokens.first() else {
        return;
    };
    let multiline = matches!(
        tokens.get(1),
        Some(token)
            if token.kind == crate::sql::lexer::Kind::Whitespace && token.text.contains('\n')
    );
    if !multiline {
        let mut end = first.text.len();
        let bytes = text.as_bytes();
        while end < bytes.len() && matches!(bytes[end], b' ' | b'\t') {
            end += 1;
        }
        found.push(Violation {
            kind: "with",
            element_count: 0,
            expected: Mode::Break,
            span: crate::span::Span::new(
                statement_start + block.start,
                statement_start + block.start + end,
            ),
            indent,
            next_indent: None,
            message: "with should start its own line".to_owned(),
        });
    }
    for inner in crate::sql::splitter::all_statements(text)
        .into_iter()
        .skip(1)
    {
        let before = &text[..inner.start];
        let trimmed = before.trim_end();
        if !trimmed.ends_with('(') {
            continue;
        }
        let before_paren = trimmed[..trimmed.len() - 1].trim_end();
        if !before_paren.to_ascii_lowercase().ends_with("as") {
            continue;
        }
        if !crate::sql::splitter::has_top_level_newline(&text[inner.start..inner.end]) {
            continue;
        }
        let close = inner.end;
        if close >= text.len() {
            continue;
        }
        let line_start = text[..close].rfind('\n').map_or(0, |index| index + 1);
        let prefix = &text[line_start..close];
        let aligned = prefix
            .chars()
            .all(|character| character == ' ' || character == '\t')
            && prefix.len() == indent;
        if aligned {
            continue;
        }
        let same_line = !prefix
            .chars()
            .all(|character| character == ' ' || character == '\t');
        let span_start = if same_line { close } else { line_start };
        found.push(Violation {
            kind: "with-close",
            element_count: 0,
            expected: Mode::Break,
            span: crate::span::Span::new(
                statement_start + block.start + span_start,
                statement_start + block.start + close + 1,
            ),
            indent,
            next_indent: None,
            message: "cte close should align with with".to_owned(),
        });
    }
    let clauses = crate::sql::splitter::clause_spans(text);
    if let Some(clause) = clauses.first() {
        let relative = block.start + clause.keyword_start;
        let line_start = statement_text[..relative]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        let prefix = &statement_text[line_start..relative];
        if prefix
            .chars()
            .all(|character| character == ' ' || character == '\t')
            && prefix.len() == indent
        {
            return;
        }
        let mut start = relative;
        let bytes = statement_text.as_bytes();
        while start > 0 && matches!(bytes[start - 1], b' ' | b'\t') {
            start -= 1;
        }
        found.push(Violation {
            kind: "with-query",
            element_count: 0,
            expected: Mode::Break,
            span: crate::span::Span::new(
                statement_start + start,
                statement_start + relative + clause.kind.len(),
            ),
            indent,
            next_indent: None,
            message: "query should start a new line".to_owned(),
        });
    }
}

/// 文本去掉前导空白后是否以某个词开头（按词边界判断）。
fn starts_with_word(text: &str, word: &str) -> bool {
    let Some(rest) = text.trim_start().strip_prefix(word) else {
        return false;
    };
    rest.chars().next().is_none_or(|character| {
        !(character.is_alphanumeric() || character == '_' || character == '$')
    })
}

/// 提取 `case … end` 区间（含嵌套）。
pub(crate) fn case_spans(text: &str) -> Vec<crate::span::Span> {
    let tokens = crate::sql::lexer::tokens(text);
    let mut spans = Vec::new();
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
        if let Some(end) = end {
            spans.push(crate::span::Span::new(token.start, end));
        }
    }
    spans
}

/// 只保留最外层的区间。
pub(crate) fn outermost(spans: Vec<crate::span::Span>) -> Vec<crate::span::Span> {
    let mut ordered = spans;
    ordered.sort_by_key(|span| (span.start, std::cmp::Reverse(span.end)));
    let mut result: Vec<crate::span::Span> = Vec::new();
    for span in ordered {
        if result
            .last()
            .is_some_and(|last| last.start <= span.start && span.end <= last.end)
        {
            continue;
        }
        result.push(span);
    }
    result
}

/// 函数调用括号的区间（从函数名到闭括号）。
pub(crate) fn function_paren_spans(text: &str) -> Vec<crate::span::Span> {
    let tokens = crate::sql::lexer::tokens(text);
    let mut spans = Vec::new();
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
            crate::sql::lexer::Kind::Word => crate::sql::decision::is_function_name(name.text),
            crate::sql::lexer::Kind::QuotedIdent => true,
            _ => false,
        };
        if !is_name {
            continue;
        }
        let mut depth = 0;
        for next in &tokens[index..] {
            if next.kind == crate::sql::lexer::Kind::Punct && next.text == "(" {
                depth += 1;
            } else if next.kind == crate::sql::lexer::Kind::Punct && next.text == ")" {
                depth -= 1;
                if depth == 0 {
                    spans.push(crate::span::Span::new(
                        name.start,
                        next.start + next.text.len(),
                    ));
                    break;
                }
            }
        }
    }
    spans
}

fn line_indent(text: &str, offset: usize) -> usize {
    let line_start = text[..offset].rfind('\n').map_or(0, |index| index + 1);
    text[line_start..offset]
        .chars()
        .take_while(|character| *character == ' ')
        .count()
}

fn block_indent(text: &str, block_start: usize) -> usize {
    let line_start = text[..block_start].rfind('\n').map_or(0, |index| index + 1);
    let prefix = &text[line_start..block_start];
    let indent = prefix
        .chars()
        .take_while(|character| *character == ' ')
        .count();
    if prefix
        .chars()
        .all(|character| character == ' ' || character == '\t')
    {
        indent
    } else if prefix.trim_end().ends_with('(') {
        indent + 4
    } else {
        indent
    }
}

fn trailing_indent(body: &str) -> Option<usize> {
    let newline = body.rfind('\n')?;
    let tail = &body[newline + 1..];
    if tail
        .chars()
        .all(|character| character == ' ' || character == '\t')
    {
        Some(tail.chars().count())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use crate::sql::checker::Violation;
    use crate::sql::parse::Dialect;

    fn violations(sql: &str) -> Vec<Violation> {
        crate::sql::checker::violations_with(sql, Dialect::Postgres)
    }

    #[test]
    fn flags_four_field_select() {
        let found = violations("select a, b, c, d from t");
        assert!(found.iter().any(|violation| {
            violation.kind == "select" && violation.expected == crate::sql::checker::Mode::Break
        }));
    }

    #[test]
    fn allows_three_conditions_inline_body() {
        assert!(
            violations("select a\nfrom t\nwhere a = 1 and b = 2 and c = 3")
                .iter()
                .all(|violation| violation.kind != "where")
        );
    }

    #[test]
    fn flags_inline_subquery() {
        let found = violations(
            "select a from t where x in (select y from u group by y having count(*) > 1 and max(z) > 0 and min(z) < 9 and sum(z) > 1)",
        );
        assert!(found.iter().any(|violation| violation.kind == "subquery"));
    }

    #[test]
    fn handles_later_statement_after_utf8_comment() {
        let sql = "-- 用户同步修复\nbegin;\nselect a, b, c, d from t;\ncommit;";
        let found = violations(sql);
        assert!(found.iter().any(|violation| {
            violation.kind == "select" && violation.expected == crate::sql::checker::Mode::Break
        }));
    }

    #[test]
    fn flags_breaking_group_inline() {
        let found =
            violations("select a\nfrom t\nwhere 1 = 1\n    and (x = 1 or y = 2 or z = 3 or w = 4)");
        assert!(found.iter().any(|violation| violation.kind == "group"));
    }

    #[test]
    fn allows_inline_simple_group() {
        let found = violations(
            "select a\nfrom t\nwhere 1 = 1\n    and (x = 1 or y = 2 or z = 3)\n    and b = 2\n    and c = 3",
        );
        assert!(found.iter().all(|violation| violation.kind != "group"));
    }
}
