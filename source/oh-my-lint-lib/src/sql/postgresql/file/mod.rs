//! 文件级能力：同表 DDL 重排、注释对齐、空白收敛；子句模块不跨语句。

pub(crate) mod comments;
pub(crate) mod realign;
pub(crate) mod reorder;
