//! AST 结构映射：把 sqlparser 的 1-based 行列位置换成字节区间。
//!
//! sqlparser 的 `Location` 以字符计数，`Span` 的 end 是排他位置；这里一次性建行索引。

use std::ops::ControlFlow;

use sqlparser::ast::{Query, Spanned as _, Visit, Visitor};
use sqlparser::tokenizer::{Location, Span as AstSpan};

use crate::span::Span;
use crate::sql::parse::Dialect;

/// 一段 SQL 的结构：全部 `Query` 节点（文档序）。
///
/// `Query::span()` 会漏掉表达式外包的括号（例如 `in (select ...)` 的 `)`），
/// 所以这些区间只当“查询正文”使用，不拿来当语句边界。
pub struct Structure {
    pub queries: Vec<Span>,
}

/// 解析并映射；解析失败返回 `None`，调用方回退到文本扫描。
pub fn structure(sql: &str, dialect: Dialect) -> Option<Structure> {
    let statements = crate::sql::parse::parse(sql, dialect).ok()?;
    let index = LineIndex::new(sql);
    let mut collector = QueryCollector {
        sql,
        index: &index,
        spans: Vec::new(),
    };
    for statement in &statements {
        let _ = statement.visit(&mut collector);
    }
    let mut queries = collector.spans;
    queries.sort();
    queries.dedup();
    Some(Structure { queries })
}

impl Structure {
    /// 语句的块列表（相对语句起点）：根块 + 嵌套查询。
    ///
    /// 根查询必须从语句起点开始，否则返回 `None`，由调用方回退到文本扫描。
    pub fn blocks(&self, statement: Span, statement_text: &str) -> Option<Vec<Span>> {
        let mut queries: Vec<Span> = self
            .queries
            .iter()
            .filter(|query| statement.start <= query.start && query.end <= statement.end)
            .copied()
            .collect();
        queries.sort();
        let root = *queries.first()?;
        if root.start != statement.start {
            return None;
        }
        let lexical = crate::sql::splitter::all_statements(statement_text);
        let mut blocks: Vec<Span> = queries
            .into_iter()
            .filter(|query| query.start > root.start)
            .map(|query| {
                let start = query.start - statement.start;
                let end = lexical
                    .iter()
                    .find(|block| block.start == start)
                    .map_or_else(
                        || trailing_paren(statement_text, query.end - statement.start),
                        |block| block.end,
                    );
                Span::new(start, end)
            })
            .collect();
        blocks.push(Span::new(0, statement_text.len()));
        blocks.sort();
        blocks.dedup();
        Some(blocks)
    }
}

/// 行首字节偏移；用于把 `Location` 的行列位置换成字节偏移。
struct LineIndex {
    starts: Vec<usize>,
}

impl LineIndex {
    fn new(text: &str) -> Self {
        let mut starts = vec![0];
        starts.extend(text.match_indices('\n').map(|(index, _)| index + 1));
        Self { starts }
    }

    fn offset(&self, text: &str, location: Location) -> Option<usize> {
        let line = usize::try_from(location.line).ok()?.checked_sub(1)?;
        let start = *self.starts.get(line)?;
        let end = self
            .starts
            .get(line + 1)
            .map_or(text.len(), |next| next - 1);
        let column = usize::try_from(location.column).ok()?.checked_sub(1)?;
        let line_text = &text[start..end];
        let byte = line_text
            .char_indices()
            .nth(column)
            .map_or(line_text.len(), |(index, _)| index);
        Some(start + byte)
    }

    fn span(&self, text: &str, span: AstSpan) -> Option<Span> {
        if span.start.line == 0 {
            return None;
        }
        let start = self.offset(text, span.start)?;
        let end = self.offset(text, span.end)?;
        (start <= end && end <= text.len()).then(|| Span::new(start, end))
    }
}

/// 查询正文之后若只剩空白再遇 `)`，把区间扩到 `)` 之前，和文本扫描器的块边界一致。
fn trailing_paren(text: &str, end: usize) -> usize {
    let bytes = text.as_bytes();
    let mut index = end;
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    if index < bytes.len() && bytes[index] == b')' {
        index
    } else {
        end
    }
}

struct QueryCollector<'a> {
    sql: &'a str,
    index: &'a LineIndex,
    spans: Vec<Span>,
}

impl Visitor for QueryCollector<'_> {
    type Break = ();

    fn pre_visit_query(&mut self, query: &Query) -> ControlFlow<()> {
        if let Some(span) = self.index.span(self.sql, query.span()) {
            self.spans.push(span);
        }
        ControlFlow::Continue(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn structure_of(sql: &str) -> Structure {
        structure(sql, Dialect::Generic).expect("structure")
    }

    #[test]
    fn maps_statement_and_nested_queries() {
        let sql = "with c as (select a from t) select * from c where x in (select y from u)";
        let structure = structure_of(sql);
        assert_eq!(structure.queries.len(), 3);
        let texts: Vec<&str> = structure
            .queries
            .iter()
            .map(|span| &sql[span.start..span.end])
            .collect();
        assert!(texts.iter().any(|text| text.starts_with("with c as")));
        assert!(texts.iter().any(|text| text.starts_with("select a")));
        assert!(texts.iter().any(|text| text.starts_with("select y")));
    }

    #[test]
    fn maps_multibyte_text() {
        let sql = "select '中文' from t";
        let structure = structure_of(sql);
        assert_eq!(structure.queries.len(), 1);
        assert_eq!(
            &sql[structure.queries[0].start..structure.queries[0].end],
            sql
        );
    }

    #[test]
    fn blocks_are_relative_to_statement() {
        let sql = "select a from (select b from t) x";
        let structure = structure_of(sql);
        let statement = crate::sql::splitter::statements(sql)[0];
        let blocks = structure
            .blocks(statement, &sql[statement.start..statement.end])
            .expect("blocks");
        assert_eq!(blocks.len(), 2);
        assert_eq!(
            &sql[statement.start + blocks[1].start..statement.start + blocks[1].end],
            "select b from t"
        );
    }

    #[test]
    fn ddl_has_no_query_blocks() {
        let sql = "create table t (a int)";
        let structure = structure_of(sql);
        assert!(structure.queries.is_empty());
        assert!(structure.blocks(Span::new(0, sql.len()), sql).is_none());
    }
}
