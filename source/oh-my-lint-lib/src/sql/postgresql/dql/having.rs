//! `having` 子句：条件语义与 `where` 相同。

use crate::sql::postgresql::dql::clause::Mode;
use crate::sql::splitter::Element;

pub(super) fn mode(elements: &[Element]) -> Mode {
    crate::sql::postgresql::dql::r#where::mode(elements)
}
