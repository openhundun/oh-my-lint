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

`oh-my-lint fix` applies token normalization to parseable DDL/DML even when no
layout module can render the statement; unsupported statements hit the parser
gaps above and are kept byte-for-byte per R34. The fixtures use R26 terminating
semicolons; the formatter adds a missing final semicolon.

A full pass over all 236 fixtures rewrites 160 files. All parseable statements,
including those in comment-bearing files, receive R25–R39 token normalization;
comments are preserved. Eleven structural findings remain in five files:
`select.sql` (`ORDER BY`), `select_into.sql` (`SELECT INTO`), `set_operators.sql`
and `table_command.sql` (parenthesized set-operation branches), and
`table_functions.sql` (set operation inside `ARRAY(...)`).
`postgres_postgis_operators.sql` is token-normalized but keeps its original
layout around `(VALUES ...)` derived tables.

`source/oh-my-lint-lib/src/sql/tests.rs` includes the seven DDL/DML fixtures
with `include_str!` and asserts `check` is empty; it does not assert
`format == fixture`, because unsupported statements are preserved byte-for-byte.

## Parser coverage snapshot

A pass over the 236 fixtures found 1,532 statements that sqlparser accepts.
Only 645 are in the six implemented layout families (`Query`, `Delete`,
`Explain`, `CreateTable`, `CreateIndex`, `AlterTable`); the other 887 fall into
82 `Other` AST variants, including `CreatePolicy` (7), `AlterPolicy` (8),
`DropPolicy` (5), `CreateFunction` (23), `CreateView` (28), `Insert` (33),
`Update` (13), `Merge` (15) and `Truncate` (15). These now receive token
normalization only; their structural layout is unchanged.

The same pass found 825 statements across 133 files that sqlparser rejects,
e.g. `ALTER DATABASE`, `ALTER DOMAIN`, `ALTER EVENT TRIGGER`, `ALTER EXTENSION`
and `ALTER INDEX ... SET TABLESPACE`; they remain byte-for-byte under R34.
Comment-bearing statement blocks are token-normalized with comments preserved;
they do not participate in structural layout (R31).
