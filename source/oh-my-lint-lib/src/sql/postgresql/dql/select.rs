//! `select` 子句：元素数量、别名完整度与复杂表达式决定断行。

use crate::sql::postgresql::dql::clause::{ClauseOutput, Mode};
use crate::sql::splitter::Element;

pub(super) fn mode(elements: &[Element]) -> Mode {
    let count = elements.len();
    if count <= 1 {
        return if elements
            .first()
            .is_some_and(|element| crate::sql::decision::has_subquery(&element.text) || crate::sql::decision::function_contains_subquery(&element.text))
        {
            Mode::Break
        } else {
            Mode::Inline
        };
    }
    let complex = elements.iter().any(|element| crate::sql::decision::is_complex(&element.text, "select"));
    if count >= 4 || complex || (count == 3 && elements.iter().all(|element| crate::sql::decision::has_alias(&element.text))) {
        Mode::Break
    } else {
        Mode::Inline
    }
}

pub(super) fn render(elements: &[Element], expected: Mode) -> ClauseOutput {
    crate::sql::postgresql::dql::list::render(elements, expected)
}
