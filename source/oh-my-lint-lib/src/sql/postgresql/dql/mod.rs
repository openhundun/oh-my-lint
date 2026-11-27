//! DQL 子句布局；每个子句只描述自己的输入/输出模式。

mod case;
pub(crate) mod checker;
pub(crate) mod clause;
pub(crate) mod fix;
mod from;
mod function;
mod group;
mod group_by;
mod having;
mod join;
mod list;
mod on;
mod order_by;
mod rollup;
mod select;
mod set_op;
mod subquery;
mod util;
pub(crate) mod values;
pub(crate) mod r#where;
mod with;

pub(crate) use clause::Mode;
