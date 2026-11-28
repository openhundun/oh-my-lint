//! 子句策略分发：每个关键字模块只决定自己的断行模式与元素版式。

use crate::sql::postgresql::dql::from;
use crate::sql::postgresql::dql::group_by;
use crate::sql::postgresql::dql::having;
use crate::sql::postgresql::dql::order_by;
use crate::sql::postgresql::dql::select;
use crate::sql::postgresql::dql::r#where;
use crate::sql::splitter::{Connector, Element};

/// 期望的版式。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Mode {
    Break,
    Inline,
}

impl Mode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Break => "break",
            Self::Inline => "inline",
        }
    }
}

/// 列表子句输出：内联文本或逐元素行。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ClauseOutput {
    Inline(String),
    Exline(Vec<String>),
}

/// 子句是否必须一元素一行；未识别的子句沿用通用阈值。
pub(crate) fn mode(kind: &str, elements: &[Element]) -> Mode {
    match kind {
        "select" => select::mode(elements),
        "from" => from::mode(elements),
        "where" => r#where::mode(elements),
        "having" => having::mode(elements),
        "group by" => group_by::mode(elements),
        "order by" => order_by::mode(elements),
        _ => generic_mode(elements),
    }
}

/// `needs_break` 的兼容入口，DML 也复用同一套子句阈值。
pub(crate) fn needs_break(kind: &str, elements: &[Element]) -> bool {
    mode(kind, elements) == Mode::Break
}

/// 渲染普通列表；条件列表由 `ConditionLayout` 渲染。
pub(crate) fn render(kind: &str, elements: &[Element], expected: Mode) -> Option<ClauseOutput> {
    match kind {
        "select" => Some(select::render(elements, expected)),
        "from" => Some(from::render(elements, expected)),
        "group by" => Some(group_by::render(elements, expected)),
        "order by" => Some(order_by::render(elements, expected)),
        _ => None,
    }
}

/// 条件连接词转成文本；首元素为 `and`，保持 `where 1 = 1` 的语料形态。
pub(crate) fn connector_word(connector: Connector) -> &'static str {
    match connector {
        Connector::Or => "or",
        _ => "and",
    }
}

/// 通用规则：≥4 或含复杂表达式时逐元素断行。
fn generic_mode(elements: &[Element]) -> Mode {
    if elements.len() < 4 && !elements.iter().any(|element| crate::sql::decision::is_complex(&element.text, "")) {
        Mode::Inline
    } else {
        Mode::Break
    }
}
