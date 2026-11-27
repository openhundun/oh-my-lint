//! 逗号列表子句的共用渲染：`select` / `from` / `group by` / `order by`。

use crate::sql::postgresql::dql::clause::{ClauseOutput, Mode};
use crate::sql::splitter::Element;

pub(super) fn render(elements: &[Element], expected: Mode) -> ClauseOutput {
    match expected {
        Mode::Inline => ClauseOutput::Inline(elements.iter().map(|element| element.text.as_str()).collect::<Vec<_>>().join(", ")),
        Mode::Break => ClauseOutput::Exline(
            elements
                .iter()
                .enumerate()
                .map(|(index, element)| if index + 1 < elements.len() { format!("{},", element.text) } else { element.text.clone() })
                .collect(),
        ),
    }
}
