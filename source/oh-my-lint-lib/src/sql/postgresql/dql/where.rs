//! 条件子句：断行时整块加括号，内联时不包；`where`/`having`/`on` 共用。

use crate::sql::core::lexer::{self, Kind};
use crate::sql::core::splitter::{self, Connector, Element};
use crate::sql::postgresql::dql::clause::{ClauseOutput, Mode, connector_word};

/// 条件正文：去掉一整层结构化外层括号后，再按布尔连接词切分。
#[derive(Clone, Debug)]
pub(crate) struct ConditionBody<'a> {
    pub(crate) elements: Vec<Element>,
    pub(crate) wrapped: bool,
    pub(crate) inner: &'a str,
}

/// 解析条件正文；只有包住整段正文的括号才算结构，用户写的局部条件组不受影响。
pub(crate) fn parse(body: &str) -> ConditionBody<'_> {
    match outer_span(body) {
        Some((open, close)) => ConditionBody {
            elements: splitter::split_elements("where", &body[open + 1..close]),
            wrapped: true,
            inner: &body[open + 1..close],
        },
        None => ConditionBody { elements: splitter::split_elements("where", body), wrapped: false, inner: body },
    }
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

/// 断行条件块是否已按房规对齐：`where (` 同行、锚点 +4、闭括号回关键字缩进。
pub(crate) fn block_layout_ok(body: &str, indent: usize) -> bool {
    let Some((open, close)) = outer_span(body) else {
        return false;
    };
    if body[..open].contains('\n') {
        return false;
    }
    let inner = &body[open + 1..close];
    let Some(first_break) = inner.find('\n') else {
        return false;
    };
    if !inner[..first_break].trim().is_empty() {
        return false;
    }
    let Some(first_line) = inner[first_break + 1..].lines().find(|line| !line.trim().is_empty()) else {
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
    close_prefix.chars().all(|character| character == ' ' || character == '\t') && close_prefix.chars().count() == indent
}

pub(crate) fn mode(elements: &[Element]) -> Mode {
    if elements.len() <= 1 {
        return Mode::Inline;
    }
    if elements.len() >= 4 || elements.iter().any(|element| crate::sql::decision::is_complex(&element.text, "where")) {
        Mode::Break
    } else {
        Mode::Inline
    }
}

pub(super) fn render(elements: &[Element], expected: Mode) -> ClauseOutput {
    match expected {
        Mode::Inline => ClauseOutput::Inline(inline_text(elements)),
        Mode::Break => ClauseOutput::Block(block_lines(elements)),
    }
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
