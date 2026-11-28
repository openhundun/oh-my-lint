//! 条件子句：断行时整块加括号，内联时不包；`where`/`having`/`on` 共用。

use crate::span::Span;
use crate::sql::core::lexer::{self, Kind};
use crate::sql::core::splitter::{self, Connector, Element};
use crate::sql::postgresql::dql::checker::Violation;
use crate::sql::postgresql::dql::clause::{Mode, connector_word};
use crate::sql::postgresql::dql::util::trailing_indent;

#[oh_my_lint_macros::clause_rules(
    input(elements: &[Element]),
    output = Mode,
    applies_to = "WHERE, HAVING, and JOIN ON"
)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ConditionStrategy {
    #[rule(
        code = "inline-single-condition",
        when = elements.len() <= 1,
        output = Mode::Inline,
        description = "Keep a single condition inline",
        trigger = "Zero or one top-level condition, regardless of complexity. The leading layout anchor does not count.",
        layout = "Keep the condition with the clause keyword without wrapping the whole clause in parentheses or adding an anchor. Nested subqueries are formatted independently.",
        rationale = "A single condition does not need the parentheses or anchor used for a multiline list.",
        before = "select id from orders where (id = 1);",
        after = "select id from orders where id = 1;",
    )]
    SinglePredicate,
    #[rule(
        code = "break-many-conditions",
        when = elements.len() >= 4,
        output = Mode::Break,
        description = "Put four or more conditions on separate lines",
        trigger = "At least four top-level conditions, excluding the leading layout anchor. Complexity is not checked.",
        layout = "Open the parentheses on the keyword line. Indent the anchor and each condition by four spaces on separate lines, then align the closing parenthesis with the keyword line. Nested subqueries are formatted independently.",
        rationale = "Separate lines make each condition and its connector easier to see in a long list.",
        before = "select id\nfrom orders\nwhere a = 1 and b = 2 and c = 3 and d = 4;",
        after = "select id\nfrom orders\nwhere (\n    1 = 1\n    and a = 1\n    and b = 2\n    and c = 3\n    and d = 4\n);",
    )]
    ManyPredicates,
    #[rule(
        code = "break-complex-conditions",
        when = elements.iter().any(|element| crate::sql::decision::is_complex(&element.text, "where")),
        output = Mode::Break,
        description = "Break a short list that contains a complex condition",
        trigger = "Two or three top-level conditions, excluding the leading layout anchor, with at least one complex condition, such as a subquery or Boolean group.",
        layout = "Open the parentheses on the keyword line. Indent the anchor and each condition by four spaces on separate lines, then align the closing parenthesis with the keyword line. Nested subqueries are formatted independently.",
        rationale = "Putting a complex condition on its own line makes it easier to distinguish from its neighbors.",
        before = "select a.id\nfrom t a\njoin u b on a.id = b.id and (b.status = 1 or b.status = 2);",
        after = "select a.id\nfrom t a\njoin u b on (\n    1 = 1\n    and a.id = b.id\n    and (b.status = 1 or b.status = 2)\n);",
    )]
    ComplexPredicates,
    #[rule(
        code = "inline-simple-conditions",
        otherwise,
        output = Mode::Inline,
        description = "Keep two or three simple conditions inline",
        trigger = "Two or three top-level conditions, excluding the leading layout anchor, with no complex conditions.",
        layout = "Keep the conditions with the clause keyword, without outer parentheses or a layout anchor.",
        rationale = "A short list of simple conditions is easy to read without line breaks.",
        before = "select city\nfrom visits\ngroup by city\nhaving (\n    1 = 1\n    and count(*) > 1\n    and sum(duration) > 10\n);",
        after = "select city\nfrom visits\ngroup by city\nhaving count(*) > 1 and sum(duration) > 10;",
    )]
    SimplePredicates,
}

/// 条件列表的断行决策及供修复复用的谓词。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ConditionLayout {
    pub(crate) strategy: ConditionStrategy,
    /// 当前检查输入中的关键字区间，不包含待替换的正文。
    pub(crate) keyword_span: Span,
    pub(crate) elements: Vec<Element>,
}

/// 条件正文：去掉一整层结构化外层括号后，再按布尔连接词切分。
#[derive(Clone, Debug)]
pub(crate) struct ConditionBody<'a> {
    pub(crate) elements: Vec<Element>,
    outer: Option<(usize, usize)>,
    pub(crate) inner: &'a str,
}

/// 解析条件正文；只有包住整段正文的括号才算结构，用户写的局部条件组不受影响。
pub(crate) fn parse(body: &str) -> ConditionBody<'_> {
    let outer = outer_span(body);
    let inner = outer.map_or(body, |(open, close)| &body[open + 1..close]);
    ConditionBody { elements: splitter::split_elements("where", inner), outer, inner }
}

/// 外层括号的字节区间；必须包住整段正文，且只包一层。
fn outer_span(body: &str) -> Option<(usize, usize)> {
    let tokens = lexer::tokens(body);
    let first = tokens.iter().find(|token| !matches!(token.kind, Kind::Whitespace | Kind::Comment))?;
    let last = tokens.iter().rev().find(|token| !matches!(token.kind, Kind::Whitespace | Kind::Comment))?;
    if first.text != "(" || last.text != ")" {
        return None;
    }
    let mut depth = 0i32;
    for token in tokens.iter().filter(|token| !matches!(token.kind, Kind::Whitespace | Kind::Comment)) {
        match token.text {
            "(" => depth += 1,
            ")" => {
                depth -= 1;
                if depth == 0 && token.start != last.start {
                    return None;
                }
            }
            _ => {}
        }
    }
    (depth == 0).then_some((first.start, last.start))
}

impl ConditionBody<'_> {
    /// 用指定版式检查已解析的条件，不重新选择策略。
    pub(crate) fn layout_ok(&self, body: &str, expected: Mode, indent: usize) -> bool {
        match expected {
            Mode::Inline => self.outer.is_none() && !splitter::has_top_level_newline(body.trim_end()),
            Mode::Break => self.outer.is_some_and(|(open, close)| self.block_layout_ok(body, open, close, indent)),
        }
    }

    /// 左括号与关键字同行，锚点缩进四格，右括号回到关键字所在行的缩进。
    fn block_layout_ok(&self, body: &str, open: usize, close: usize, indent: usize) -> bool {
        if body[..open].contains('\n') {
            return false;
        }
        let Some(first_break) = self.inner.find('\n') else {
            return false;
        };
        if !self.inner[..first_break].trim().is_empty() {
            return false;
        }
        let Some(first_line) = self.inner[first_break + 1..].lines().find(|line| !line.trim().is_empty()) else {
            return false;
        };
        let first_indent = first_line.len() - first_line.trim_start().len();
        if first_indent != indent + 4 {
            return false;
        }
        let anchor = first_line.trim_start();
        if anchor != "1 = 1" && anchor != "1 = 0" {
            return false;
        }
        let close_line_start = body[..close].rfind('\n').map_or(0, |index| index + 1);
        let close_prefix = &body[close_line_start..close];
        close_prefix.chars().all(|character| character == ' ' || character == '\t') && close_prefix.chars().count() == indent && splitter::has_top_level_newline(self.inner)
    }
}

impl ConditionLayout {
    /// 判定非空的条件列表；正文从关键字末尾开始，`next_indent` 可覆盖正文尾部缩进。
    pub(super) fn plan(kind: &'static str, body: &str, condition: ConditionBody<'_>, keyword_span: Span, indent: usize, next_indent: Option<usize>) -> Option<Violation> {
        let strategy = ConditionStrategy::select(&condition.elements);
        let expected = strategy.mode();
        if condition.layout_ok(body, expected, indent) {
            return None;
        }
        let detail = match (expected, condition.outer.is_some()) {
            (Mode::Break, false) => "should be wrapped",
            (Mode::Break, true) => "condition block is not aligned",
            (Mode::Inline, true) => "should not be wrapped",
            (Mode::Inline, false) => "should be inline",
        };
        Some(Violation {
            kind,
            element_count: condition.elements.len(),
            expected,
            span: Span::new(keyword_span.end, keyword_span.end + body.len()),
            indent,
            next_indent: next_indent.or_else(|| trailing_indent(body)),
            message: format!("{kind} {detail}"),
            condition: Some(Self { strategy, keyword_span, elements: condition.elements }),
        })
    }

    /// 渲染已经选定的策略；只处理版式与相邻子句之间的边界。
    pub(super) fn render(&self, text: &str, violation: &Violation) -> String {
        let tail = &text[violation.span.end..];
        let next_indent = violation.next_indent.unwrap_or(violation.indent);
        let next = tail.trim_start().chars().next();
        let next_starts_newline = tail.chars().take_while(|character| character.is_whitespace()).any(|character| character == '\n');
        match self.strategy.mode() {
            Mode::Inline => {
                let suffix = if tail.is_empty() || matches!(next, Some(',' | ';')) || next_starts_newline {
                    String::new()
                } else if next != Some(')') && !crate::sql::decision::is_simple(text) {
                    format!("\n{}", " ".repeat(next_indent))
                } else {
                    " ".to_owned()
                };
                format!(" {}{suffix}", inline_text(&self.elements))
            }
            Mode::Break => self.render_block(violation.indent, !tail.is_empty() && next != Some(',') && !next_starts_newline, next_indent),
        }
    }

    /// ` (` 与关键字同行，内容 +4，闭括号回关键字缩进。
    fn render_block(&self, indent: usize, has_next: bool, next_indent: usize) -> String {
        let padding = " ".repeat(indent + 4);
        let mut output = String::from(" (");
        for line in block_lines(&self.elements) {
            output.push('\n');
            output.push_str(&padding);
            output.push_str(&line);
        }
        output.push('\n');
        output.extend(std::iter::repeat_n(' ', indent));
        output.push(')');
        if has_next {
            output.push('\n');
            output.extend(std::iter::repeat_n(' ', next_indent));
        }
        output
    }
}

pub(crate) fn mode(elements: &[Element]) -> Mode {
    ConditionStrategy::select(elements).mode()
}

/// 条件列表的锚点与逐条件行；首条件沿用整条链的连接词，避免 `1 = 0 and a or b`。
pub(crate) fn block_lines(elements: &[Element]) -> Vec<String> {
    let connective = elements.get(1).or_else(|| elements.first()).map_or(Connector::And, |element| element.connector);
    let anchor = if connective == Connector::Or { "1 = 0" } else { "1 = 1" };
    let mut lines = vec![anchor.to_owned()];
    for (index, element) in elements.iter().enumerate() {
        let connector = if index == 0 { connective } else { element.connector };
        lines.push(format!("{} {}", connector_word(connector), element.text));
    }
    lines
}

/// 内联条件；不添加锚点，也不保留结构化外层括号。
pub(crate) fn inline_text(elements: &[Element]) -> String {
    let mut joined = String::new();
    for (index, element) in elements.iter().enumerate() {
        if index == 0 {
            joined.push_str(&element.text);
            continue;
        }
        joined.push(' ');
        joined.push_str(connector_word(element.connector));
        joined.push(' ');
        joined.push_str(&element.text);
    }
    joined
}
