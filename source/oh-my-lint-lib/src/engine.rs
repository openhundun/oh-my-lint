//! 引擎：按块执行规则，块级修复只碰块内，不做整文件重排。

/// 一条带位置信息的诊断；`finding.span` 相对于 `block_span`。
#[derive(Clone, Debug)]
pub struct LocatedFinding {
    pub block_span: crate::span::Span,
    pub finding: crate::model::Finding,
    pub fixable: bool,
}

/// 修复被跳过的原因；CLI 原样转给 agent，避免它猜。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Skipped {
    pub rule: String,
    pub reason: String,
}

/// 一个块的修复结果；`original` / `fixed` 都是块内文本。
#[derive(Clone, Debug)]
pub struct BlockFix {
    pub block_span: crate::span::Span,
    pub original: String,
    pub fixed: String,
    pub applied: Vec<String>,
    pub skipped: Vec<Skipped>,
    pub remaining: Vec<crate::model::Finding>,
}

impl BlockFix {
    /// 是否有实际改动。
    pub fn changed(&self) -> bool {
        self.original != self.fixed
    }
}

/// 规则集合；顺序就是应用顺序。
pub struct Engine {
    rules: Vec<Box<dyn crate::model::Rule>>,
}

impl Engine {
    /// 用指定规则构造，测试与后续语言扩展都走这里。
    pub fn new(rules: Vec<Box<dyn crate::model::Rule>>) -> Self {
        Self { rules }
    }

    /// 默认规则集：目前只有 SQL 断行。
    pub fn with_defaults() -> Self {
        Self::new(vec![Box::new(crate::sql::SqlRule::default())])
    }

    /// 遍历已注册规则，供 `rules` / `explain` 命令使用。
    pub fn rules(&self) -> impl Iterator<Item = &dyn crate::model::Rule> {
        self.rules.iter().map(|rule| rule.as_ref())
    }

    /// 检查整份文档，返回块内坐标的诊断。
    pub fn check(&self, document: &crate::source::Document) -> Vec<LocatedFinding> {
        let mut found = Vec::new();
        for block in &document.blocks {
            for rule in &self.rules {
                let findings = rule.check(block);
                if findings.is_empty() {
                    continue;
                }
                let fixable = rule.fix(block).is_some();
                for finding in findings {
                    found.push(LocatedFinding { block_span: block.span, finding, fixable });
                }
            }
        }
        found
    }

    /// 修复文档；`range` 为 `None` 时处理所有块。
    ///
    /// 块级模式只应用 `FixScope::Local`；其余修复进入 `skipped`，由调用方升级到
    /// `--whole-file`（后续版本）或人工处理。
    pub fn fix(&self, document: &crate::source::Document, range: Option<crate::span::Span>, allow_unsafe: bool) -> Vec<BlockFix> {
        let mut fixes = Vec::new();
        for block in &document.blocks {
            if let Some(range) = range
                && !block.span.intersects(range)
            {
                continue;
            }
            if let Some(fix) = self.fix_block(block, allow_unsafe) {
                fixes.push(fix);
            }
        }
        fixes
    }

    fn fix_block(&self, block: &crate::model::Block, allow_unsafe: bool) -> Option<BlockFix> {
        let mut current = block.text.clone();
        let mut applied = Vec::new();
        let mut skipped = Vec::new();

        for rule in &self.rules {
            let candidate = crate::model::Block { span: crate::span::Span::default(), text: current.clone(), language: block.language };
            if let Err(error) = rule.can_fix(&candidate) {
                skipped.push(Skipped { rule: rule.id().to_owned(), reason: error.to_string() });
                continue;
            }
            let Some(fix) = rule.fix(&candidate) else {
                continue;
            };
            if fix.scope != crate::model::FixScope::Local {
                skipped.push(Skipped { rule: rule.id().to_owned(), reason: format!("{:?} fix needs file or workspace context", fix.scope) });
                continue;
            }
            if fix.safety == crate::model::FixSafety::Unsafe && !allow_unsafe {
                skipped.push(Skipped { rule: rule.id().to_owned(), reason: "unsafe fix; pass --unsafe".to_owned() });
                continue;
            }
            match apply_edits(&current, &fix.edits) {
                Ok(next) if next != current => {
                    current = next;
                    applied.push(rule.id().to_owned());
                }
                Ok(_) => {}
                Err(reason) => skipped.push(Skipped { rule: rule.id().to_owned(), reason }),
            }
        }

        if current == block.text && skipped.is_empty() {
            return None;
        }

        let fixed_block = crate::model::Block { span: crate::span::Span::default(), text: current.clone(), language: block.language };
        let remaining = self.rules.iter().flat_map(|rule| rule.check(&fixed_block)).collect();
        Some(BlockFix { block_span: block.span, original: block.text.clone(), fixed: current, applied, skipped, remaining })
    }
}

/// 按偏移倒序应用文本编辑；任何越界都拒绝整条修复，避免半截改动。
pub fn apply_edits(text: &str, edits: &[crate::model::TextEdit]) -> std::result::Result<String, String> {
    let mut ordered = edits.to_vec();
    ordered.sort_by(|left, right| right.span.start.cmp(&left.span.start).then(right.span.end.cmp(&left.span.end)));
    let mut output = text.to_owned();
    for edit in ordered {
        if edit.span.end > output.len() || edit.span.start > edit.span.end {
            return Err(format!("edit out of bounds: {:?}", edit.span));
        }
        output.replace_range(edit.span.start..edit.span.end, &edit.replacement);
    }
    Ok(output)
}

/// 文件级修复结果：把块内修复映射回文件坐标。
#[derive(Clone, Debug)]
pub struct FileFix {
    pub path: std::path::PathBuf,
    pub block_span: crate::span::Span,
    pub original: String,
    pub fixed: String,
    pub applied: Vec<String>,
    pub skipped: Vec<Skipped>,
    pub remaining: Vec<crate::model::Finding>,
}

impl Engine {
    /// 把块修复结果映射到文件；不写盘，由 CLI 决定 `--write` / `--diff`。
    pub fn apply(&self, document: &crate::source::Document, range: Option<crate::span::Span>, allow_unsafe: bool) -> Vec<FileFix> {
        self.fix(document, range, allow_unsafe)
            .into_iter()
            .map(|fix| FileFix {
                path: document.path.clone(),
                block_span: fix.block_span,
                original: fix.original,
                fixed: fix.fixed,
                applied: fix.applied,
                skipped: fix.skipped,
                remaining: fix.remaining,
            })
            .collect()
    }
}

/// 把一组文件级修复写回整份文本；块外字节保持不变。
pub fn splice(text: &str, fixes: &[FileFix]) -> String {
    let mut ordered = fixes.to_vec();
    ordered.sort_by_key(|fix| std::cmp::Reverse(fix.block_span.start));
    let mut output = text.to_owned();
    for fix in ordered {
        if fix.block_span.end > output.len() {
            continue;
        }
        output.replace_range(fix.block_span.start..fix.block_span.end, &fix.fixed);
    }
    normalize_file_end(&output)
}

/// 文件以单个换行结尾；块内文本由 SQL 规则负责其余空白。
pub fn normalize_file_end(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    format!("{}\n", text.trim_end_matches(['\n', '\r']))
}
