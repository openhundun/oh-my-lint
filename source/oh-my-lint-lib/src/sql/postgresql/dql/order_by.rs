//! `order by` 子句：≥4 个元素或含复杂表达式时断行。

use crate::sql::postgresql::dql::clause::{ClauseOutput, Mode};
use crate::sql::splitter::Element;

pub(super) fn mode(elements: &[Element]) -> Mode {
    if elements.len() <= 1 {
        return Mode::Inline;
    }
    if elements.len() >= 4 || elements.iter().any(|element| crate::sql::decision::is_complex(&element.text, "order by")) {
        Mode::Break
    } else {
        Mode::Inline
    }
}

pub(super) fn render(elements: &[Element], expected: Mode) -> ClauseOutput {
    crate::sql::postgresql::dql::list::render(elements, expected)
}
