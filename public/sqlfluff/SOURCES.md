# sqlfluff PostgreSQL fixtures

- Upstream: <https://github.com/sqlfluff/sqlfluff>
- Path: `test/fixtures/dialects/postgres/`
- Commit: `62c7ee974a04df9f2c8ca5a5684cc1022543edfe`
- License: MIT — see [`LICENSE.md`](LICENSE.md)
- Dialect: upstream `.sqlfluff` declares `dialect = postgres`
- Imported: 2026-11-26; all 236 `.sql` files were kept, the `.yml` fixture expectations were not copied.

## Import report

Parsed with `sqlparser 0.64.0` (`PostgreSqlDialect`) before formatting:

| Class | Files | Handling |
| --- | --- | --- |
| DQL (`select`/`with` only) | 36 | formatted with `oh-my-lint fix --dialect postgres` |
| Other (DDL/DML/utility) | 70 | phase-3 input; 7 DDL/DML fixtures normalized by hand |
| Parse failures | 130 | kept; listed in `parse-failures.tsv` |

The parse failures are mostly PostgreSQL DDL outside the current parser subset
(`ALTER ...`, `CREATE ...`, roles, publications, operators, etc.); they are valid
upstream SQL, not corpus errors. Do not delete them silently.

DQL formatting: 14 files were skipped because they contain comments (R31 skips
comment blocks); the remaining DQL files were formatted (18 are clean). Four DQL
files still report findings after formatting:

| File | Findings | Note |
| --- | --- | --- |
| `postgres_postgis_operators.sql` | 197 | `fix` refused (render/parse error); needs investigation |
| `table_functions.sql` | 20 | `fix` refused |
| `set_operators.sql` | 8 | branch/select layout still disagrees with the checker |
| `select_into.sql` | 1 | `SELECT ... INTO` is not a clause in the rule set |

Skipped because of comments: `cast_with_whitespaces.sql`, `hash_operators.sql`,
`is_distinct_from.sql`, `is_json.sql`, `join_lateral.sql`, `join_no_space.sql`,
`join_types.sql`, `json_operators.sql`, `merge_with_cte.sql`, `overlaps.sql`,
`pattern_match_expressions.sql`, `pg_trgm.sql`,
`select_order_by_using_operator.sql`, `within_group.sql`.

## Parser coverage gaps

Planned formatter policy: a statement that cannot be parsed is skipped and kept
byte-for-byte; it is not rewritten and not reported as a corpus error. This also
lets handlebars-like syntax pass through untouched for now.

Observed `sqlparser 0.64.0` gaps, to report upstream:

| Construct | Example | Parser error |
| --- | --- | --- |
| `ALTER TABLE ... SET TABLESPACE` | `alter table distributors set tablespace fasttablespace;` | `Expected: (, found: tablespace` |
| index tablespace | `create index code_idx on films (code) tablespace indexspace;` | `Expected: end of statement, found: tablespace` |
| CTAS `USING method` | `create table t1 using method as (select ...);` | `Expected: end of statement, found: using` |
| column `STORAGE` / `COMPRESSION` | `a text storage plain` | `Expected: ',' or ')' after column definition, found: storage` |
| `EXPLAIN ANALYZE VERBOSE` | `explain analyze verbose select 1;` | `Expected: end of statement, found: verbose` |
| `DELETE ... WHERE CURRENT OF` | `delete from tasks where current of c_tasks;` | `Expected: end of statement, found: of` |
| table constraint `UNIQUE ... WITH (...)` | `unique (name) with (fillfactor=70)` | `Expected: ',' or ')' after column definition, found: with` |

## Phase-3 corpus

The following DDL/DML fixtures report zero findings under
`oh-my-lint check --dialect postgres`:

`alter_table.sql`, `create_index.sql`, `create_table.sql`,
`create_table_as.sql`, `create_table_column_storage_compression.sql`,
`delete.sql`, `explain.sql`.

`oh-my-lint fix` still refuses most of these files end-to-end because their
statements hit the parser gaps above; per R34 the unsupported statements are
kept byte-for-byte, so `fix` only rewrites the statements a layout module can
render.

`datetime_units.sql` also had the R35 typed-literal / interval-unit rewrite
applied.

The fixtures intentionally keep their final semicolons (R26 removes the
semicolon only in fix output). `source/oh-my-lint-lib/src/sql/tests.rs` includes
these seven fixtures with `include_str!` and asserts `check` is empty; it does
not assert `format == fixture`.
