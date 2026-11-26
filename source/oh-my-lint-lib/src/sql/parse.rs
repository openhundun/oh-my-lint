//! 解析门禁：每条语句单独通过 sqlparser，再按门类进入布局与 token 规则。
//!
//! 只暴露四个正式方言；`postgres` 是默认值。

use sqlparser::ast::{SetExpr, Statement};
use sqlparser::dialect::{Dialect as ParserDialect, DuckDbDialect, HiveDialect, MySqlDialect, PostgreSqlDialect};
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

/// 单条语句的门类；只有查询、`DELETE`、`EXPLAIN` 与三种 DDL 有自己的版式。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatementKind {
    Query,
    Delete,
    Explain,
    CreateTable,
    CreateIndex,
    AlterTable,
    Insert,
    Other,
}

/// 解析成 sqlparser AST；错误文本带行列信息。
pub fn parse(sql: &str, dialect: Dialect) -> Result<Vec<Statement>, String> {
    Parser::parse_sql(dialect.parser().as_ref(), sql).map_err(|error| error.to_string())
}

/// 把一条语句（不含尾分号）解析成门类；解析失败或不是恰好一条语句时返回 `Err`。
pub fn classify_one(sql: &str, dialect: Dialect) -> Result<StatementKind, String> {
    let statements = parse(sql, dialect)?;
    let statement = match statements.as_slice() {
        [statement] => statement,
        [] => return Err("no statement to classify".to_owned()),
        _ => {
            return Err(format!("expected one statement, found {}", statements.len()));
        }
    };
    Ok(match statement {
        // `WITH … DELETE/UPDATE/INSERT/MERGE` 包在 `Statement::Query` 的 body 里。
        Statement::Query(query) => match query.body.as_ref() {
            SetExpr::Delete(_) => StatementKind::Delete,
            SetExpr::Update(_) | SetExpr::Insert(_) | SetExpr::Merge(_) => StatementKind::Other,
            _ => StatementKind::Query,
        },
        Statement::Delete(_) => StatementKind::Delete,
        Statement::Explain { .. } => StatementKind::Explain,
        Statement::CreateTable(_) => StatementKind::CreateTable,
        Statement::CreateIndex(_) => StatementKind::CreateIndex,
        Statement::AlterTable(_) => StatementKind::AlterTable,
        Statement::Insert(_) => StatementKind::Insert,
        _ => StatementKind::Other,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_dql_and_ddl() {
        assert!(parse("select a from t", Dialect::Postgres).is_ok());
        assert!(parse("create table t (a int primary key)", Dialect::Postgres).is_ok());
        assert!(parse("insert into t (a) values (1)", Dialect::Postgres).is_ok());
        assert!(parse("update t set a = 1 where b = 2", Dialect::Postgres).is_ok());
        assert!(parse("select a from t", Dialect::DuckDb).is_ok());
    }

    #[test]
    fn rejects_broken_between() {
        let broken = "select a from t where (x between (select 1 from u)) and (select 2 from u)";
        assert!(parse(broken, Dialect::Postgres).is_err());
    }

    #[test]
    fn respects_dialect() {
        assert!(parse("select from t", Dialect::Mysql).is_err());
        assert!(parse("select a from t distribute by a", Dialect::Hive).is_ok());
        assert!(parse("select a from t distribute by a", Dialect::Postgres).is_err());
    }

    #[test]
    fn classifies_statement_kinds() {
        let cases = [
            ("select a from t", StatementKind::Query),
            ("delete from t where a = 1", StatementKind::Delete),
            ("explain analyze select a from t", StatementKind::Explain),
            ("create table t (a int)", StatementKind::CreateTable),
            ("create index i on t (a)", StatementKind::CreateIndex),
            ("alter table t add column b int", StatementKind::AlterTable),
            ("with keep as (select 1) delete from t using keep", StatementKind::Delete),
            ("with src as (select 1) update t set a = 1", StatementKind::Other),
            ("insert into t (a) values (1)", StatementKind::Insert),
        ];
        for (sql, expected) in cases {
            assert_eq!(classify_one(sql, Dialect::Postgres), Ok(expected), "classify {sql}");
        }
    }

    #[test]
    fn classify_rejects_broken_sql() {
        let broken = "select a from t where (x between (select 1 from u)) and (select 2 from u)";
        assert!(classify_one(broken, Dialect::Postgres).is_err());
        assert!(classify_one("select 1; select 2", Dialect::Postgres).is_err());
    }
}
