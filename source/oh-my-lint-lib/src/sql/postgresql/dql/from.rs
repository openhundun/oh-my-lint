//! `from` 子句：≥4 个元素、含 join 或派生表时断行。

use crate::sql::postgresql::dql::clause::{ClauseOutput, Mode};
use crate::sql::splitter::Element;

pub(super) fn mode(elements: &[Element]) -> Mode {
    let count = elements.len();
    if count >= 4 || elements.iter().any(|element| crate::sql::decision::has_join(&element.text)) {
        return Mode::Break;
    }
    if count >= 2 && elements.iter().any(|element| crate::sql::decision::is_derived(&element.text)) {
        return Mode::Break;
    }
    Mode::Inline
}

pub(super) fn render(elements: &[Element], expected: Mode) -> ClauseOutput {
    crate::sql::postgresql::dql::list::render(elements, expected)
}
