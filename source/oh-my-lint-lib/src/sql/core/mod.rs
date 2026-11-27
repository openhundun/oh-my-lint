//! 方言无关核心：词法、token 规范化、语句/子句切分、AST span 与关键字分类。

pub(crate) mod astmap;
pub(crate) mod keywords;
pub(crate) mod lexer;
pub(crate) mod parse;
pub(crate) mod spans;
pub(crate) mod splitter;
pub(crate) mod token;
