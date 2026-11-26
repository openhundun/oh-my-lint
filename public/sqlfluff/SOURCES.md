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
| Other (DDL/DML/utility) | 70 | untouched; phase-3 input |
| Parse failures | 130 | kept; listed in `parse-failures.tsv` |

The parse failures are mostly PostgreSQL DDL outside the current parser subset
(`ALTER ...`, `CREATE ...`, roles, publications, operators, etc.); they are valid
upstream SQL, not corpus errors. Do not delete them silently.

DQL formatting: 14 files were skipped because they contain comments (R31 skips
comment blocks); the remaining DQL files were formatted (18 are clean). Four DQL
files still report findings after formatting:

| File | Findings | Note |
| --- | --- | --- |
| `postgres_postgis_operators.sql` | 198 | `fix` refused (render/parse error); needs investigation |
| `table_functions.sql` | 21 | `fix` refused |
| `set_operators.sql` | 8 | branch/select layout still disagrees with the checker |
| `select_into.sql` | 1 | `SELECT ... INTO` is not a clause in the rule set |

Skipped because of comments: `cast_with_whitespaces.sql`, `hash_operators.sql`,
`is_distinct_from.sql`, `is_json.sql`, `join_lateral.sql`, `join_no_space.sql`,
`join_types.sql`, `json_operators.sql`, `merge_with_cte.sql`, `overlaps.sql`,
`pattern_match_expressions.sql`, `pg_trgm.sql`,
`select_order_by_using_operator.sql`, `within_group.sql`.

This corpus is phase-3 input, not a golden no-op corpus; it is intentionally not
wired into `job__check` (the smoke uses an inline fixture).
