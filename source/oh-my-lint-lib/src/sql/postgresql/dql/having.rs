//! `having` 子句：条件语义与 `where` 相同。

use crate::sql::postgresql::dql::clause::{ClauseOutput, Mode};
use crate::sql::splitter::Element;

pub(super) fn mode(elements: &[Element]) -> Mode {
    crate::sql::postgresql::dql::r#where::mode(elements)
}

pub(super) fn render(elements: &[Element], expected: Mode) -> ClauseOutput {
    crate::sql::postgresql::dql::r#where::render(elements, expected)
}
