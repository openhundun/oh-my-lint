//! `where` 子句：≥4 个条件或含复杂表达式时断行；断行时首行写锚点。

use crate::sql::postgresql::dql::clause::{ClauseOutput, Mode, connector_word};
use crate::sql::splitter::{Connector, Element};

pub(super) fn mode(elements: &[Element]) -> Mode {
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
        Mode::Inline => {
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
            ClauseOutput::Inline(joined)
        }
        Mode::Break => {
            let connector = elements.get(1).or_else(|| elements.first()).map_or(Connector::And, |element| element.connector);
            let anchor = if connector == Connector::Or { "1 = 0" } else { "1 = 1" };
            let mut lines = vec![anchor.to_owned()];
            for element in elements {
                lines.push(format!("{} {}", connector_word(element.connector), element.text));
            }
            ClauseOutput::Exline(lines)
        }
    }
}
