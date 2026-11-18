//! SQL 断行与修复引擎的公共库。
//!
//! `oh-my-lint-lib` 只做三件事：定义规则模型、按块执行检查、产出最小 diff 的修复；
//! CLI 与语言规则都依赖这里，不反向依赖。

pub mod diff;
pub mod engine;
pub mod error;
pub mod model;
pub mod source;
pub mod span;
pub mod sql;
