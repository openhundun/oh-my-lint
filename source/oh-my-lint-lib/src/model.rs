//! 规则模型：一条规则看到什么、报告什么、能改什么。

/// 目前只接 SQL；新语言先加枚举值，再加对应规则模块。
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    Sql,
}

/// 诊断级别；风格类规则默认 `Warning`。
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
}

/// 修复的作用域，决定块级模式能否安全应用。
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FixScope {
    /// 只依赖块本身，块内改完不会破坏块外。
    Local,
    /// 需要全文件一致（例如 import 别名改名可能被块外引用）。
    FileConsistent,
    /// 需要跨文件事实（workspace 依赖、模块顺序等）。
    Structural,
}

/// 修复的安全等级；`Unsafe` 需要显式 `--unsafe`。
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FixSafety {
    Safe,
    Unsafe,
}

/// 一次纯文本替换，坐标相对于所在块。
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct TextEdit {
    pub span: crate::span::Span,
    pub replacement: String,
}

/// 一条规则给出的修复方案。
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Fix {
    pub edits: Vec<TextEdit>,
    pub scope: FixScope,
    pub safety: FixSafety,
}

/// 一条诊断；`span` 相对于所在块。
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Finding {
    pub rule: String,
    pub severity: Severity,
    pub message: String,
    pub span: crate::span::Span,
    pub fix: Option<Fix>,
}

/// 检查与修复的最小单位：`.sql` 文件整体，或 Python 常量里的一段 SQL。
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Block {
    pub span: crate::span::Span,
    pub text: String,
    pub language: Language,
}

/// 规则接口；实现者只关心块内文本，文件与范围由引擎处理。
pub trait Rule: Send + Sync {
    /// 稳定 id，用于配置、输出与测试。
    fn id(&self) -> &'static str;

    /// 一行说明，供 `rules` 命令使用。
    fn description(&self) -> &'static str;

    /// 检查一个块，返回块内坐标的诊断。
    fn check(&self, block: &Block) -> Vec<Finding>;

    /// 尽力修复一个块；返回 `None` 表示这条规则不提供修复。
    fn fix(&self, _block: &Block) -> Option<Fix> {
        None
    }

    /// 修复前的防御：块内容不满足前提时拒绝出方案。
    fn can_fix(&self, _block: &Block) -> std::result::Result<(), crate::error::Error> {
        Ok(())
    }
}
