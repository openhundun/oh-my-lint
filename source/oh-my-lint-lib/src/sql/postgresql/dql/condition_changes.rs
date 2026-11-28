//! 条件布局修复的归因；用关键字的词法顺序穿过现有的 token 和嵌套重排。

use crate::span::Span;
use crate::sql::core::{lexer, splitter};
use crate::sql::postgresql::dql::r#where::{self, ConditionStrategy};

pub(crate) struct ConditionChange {
    pub(crate) keyword: Span,
    pub(crate) strategy: ConditionStrategy,
}

/// 只记录被替换过的条件。查询渲染不增删或交换 WHERE/HAVING/ON 关键字。
#[derive(Default)]
pub(crate) struct ConditionChanges(Vec<(usize, ConditionStrategy)>);

impl ConditionChanges {
    pub(super) fn record(&mut self, text: &str, keyword: Span, strategy: ConditionStrategy) {
        if let Some(index) = Self::keywords(text).position(|span| span == keyword) {
            self.insert(index, strategy);
        }
    }

    pub(super) fn extend_nested(&mut self, text: &str, start: usize, nested: Self) {
        if !nested.0.is_empty() {
            let base = Self::keywords(text).take_while(|span| span.start < start).count();
            self.extend_at(base, nested);
        }
    }

    pub(crate) fn extend(&mut self, pass: Self) {
        self.extend_at(0, pass);
    }

    fn extend_at(&mut self, base: usize, pass: Self) {
        for (index, strategy) in pass.0 {
            self.insert(base + index, strategy);
        }
    }

    fn insert(&mut self, index: usize, strategy: ConditionStrategy) {
        match self.0.binary_search_by_key(&index, |&(index, _)| index) {
            Ok(position) => self.0[position].1 = strategy,
            Err(position) => self.0.insert(position, (index, strategy)),
        }
    }

    /// 已合规的子句可能随父级先压平再重建。只报告原来不合规、最终落地的决定。
    pub(crate) fn confirmed(self, original: &str, rendered: &str) -> Vec<ConditionChange> {
        if self.0.is_empty() || original == rendered {
            return Vec::new();
        }
        let original_keywords: Vec<_> = Self::keywords(original).collect();
        let rendered_keywords: Vec<_> = Self::keywords(rendered).collect();
        if original_keywords.len() != rendered_keywords.len()
            || !original_keywords
                .iter()
                .zip(&rendered_keywords)
                .all(|(left, right)| original[left.start..left.end].eq_ignore_ascii_case(&rendered[right.start..right.end]))
        {
            return Vec::new();
        }
        let original_sites = ConditionSite::collect(original);
        let rendered_sites = ConditionSite::collect(rendered);
        self.0
            .into_iter()
            .filter_map(|(index, strategy)| {
                let keyword = *original_keywords.get(index)?;
                let before = ConditionSite::at(&original_sites, keyword)?;
                let after = ConditionSite::at(&rendered_sites, rendered_keywords[index])?;
                (!before.matches(original, strategy) && after.matches(rendered, strategy)).then_some(ConditionChange { keyword, strategy })
            })
            .collect()
    }

    fn keywords(text: &str) -> impl Iterator<Item = Span> + '_ {
        lexer::tokens(text)
            .into_iter()
            .filter(|token| token.kind == lexer::Kind::Word && ["where", "having", "on"].iter().any(|word| token.text.eq_ignore_ascii_case(word)))
            .map(|token| Span::new(token.start, token.start + token.text.len()))
    }
}

struct ConditionSite {
    keyword: Span,
    body: Span,
    indent: usize,
}

impl ConditionSite {
    fn collect(text: &str) -> Vec<Self> {
        let mut sites = Vec::new();
        for query in splitter::all_statements(text) {
            let query_text = &text[query.start..query.end];
            for clause in splitter::clause_spans(query_text) {
                let start = query.start + clause.keyword_start;
                let body = Span::new(query.start + clause.body_start, query.start + clause.body_end);
                let indent = super::util::line_indent(text, start);
                match clause.kind {
                    "where" | "having" => sites.push(Self { keyword: Span::new(start, start + clause.kind.len()), body, indent }),
                    "from" => {
                        for on in super::on::clauses(&text[body.start..body.end]) {
                            let start = body.start + on.body_start;
                            sites.push(Self {
                                keyword: Span::new(body.start + on.keyword_start, start),
                                body: Span::new(start, body.start + on.body_end),
                                indent,
                            });
                        }
                    }
                    _ => {}
                }
            }
        }
        sites.sort_unstable_by_key(|site| site.keyword);
        sites.dedup_by_key(|site| site.keyword);
        sites
    }

    fn at(sites: &[Self], keyword: Span) -> Option<&Self> {
        sites.binary_search_by_key(&keyword, |site| site.keyword).ok().map(|index| &sites[index])
    }

    fn matches(&self, text: &str, strategy: ConditionStrategy) -> bool {
        let body = &text[self.body.start..self.body.end];
        r#where::parse(body).layout_ok(body, strategy.mode(), self.indent)
    }
}
