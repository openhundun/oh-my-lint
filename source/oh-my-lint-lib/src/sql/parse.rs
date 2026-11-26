//! 解析门禁：块先通过 sqlparser，再进入布局与 token 规则。
//!
//! 只暴露四个正式方言；`postgres` 是默认值。

use sqlparser::ast::Statement;
use sqlparser::dialect::{
    Dialect as ParserDialect, DuckDbDialect, HiveDialect, MySqlDialect, PostgreSqlDialect,
};
use sqlparser::parser::Parser;

/// 支持的解析方言；默认 `Postgres`。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Dialect {
    #[default]
    Postgres,
    Mysql,
    DuckDb,
    Hive,
}

impl Dialect {
    /// 命令行与诊断输出使用的名字。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Postgres => "postgres",
            Self::Mysql => "mysql",
            Self::DuckDb => "duckdb",
            Self::Hive => "hive",
        }
    }

    pub(crate) fn parser(self) -> Box<dyn ParserDialect> {
        match self {
            Self::Postgres => Box::new(PostgreSqlDialect {}),
            Self::Mysql => Box::new(MySqlDialect {}),
            Self::DuckDb => Box::new(DuckDbDialect {}),
            Self::Hive => Box::new(HiveDialect {}),
        }
    }
}

/// 解析成 sqlparser AST；错误文本带行列信息。
pub fn parse(sql: &str, dialect: Dialect) -> Result<Vec<Statement>, String> {
    Parser::parse_sql(dialect.parser().as_ref(), sql).map_err(|error| error.to_string())
}

/// 只要通过与否；失败文本带上方言名，便于定位是语法还是方言选择问题。
pub fn validate(sql: &str, dialect: Dialect) -> Result<(), String> {
    parse(sql, dialect)
        .map(|_| ())
        .map_err(|error| format!("{}: {error}", dialect.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_dql_and_ddl() {
        assert!(validate("select a from t", Dialect::Postgres).is_ok());
        assert!(validate("create table t (a int primary key)", Dialect::Postgres).is_ok());
        assert!(validate("insert into t (a) values (1)", Dialect::Postgres).is_ok());
        assert!(validate("update t set a = 1 where b = 2", Dialect::Postgres).is_ok());
        assert!(validate("select a from t", Dialect::DuckDb).is_ok());
    }

    #[test]
    fn rejects_broken_between() {
        let broken = "select a from t where (x between (select 1 from u)) and (select 2 from u)";
        assert!(validate(broken, Dialect::Postgres).is_err());
    }

    #[test]
    fn respects_dialect() {
        assert!(validate("select from t", Dialect::Mysql).is_err());
        assert!(validate("select a from t distribute by a", Dialect::Hive).is_ok());
        assert!(validate("select a from t distribute by a", Dialect::Postgres).is_err());
    }
}
