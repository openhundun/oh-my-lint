# PostgreSQL 官方回归语料白名单

- 上游：<https://github.com/postgres/postgres>
- 分支：`REL_17_STABLE`
- 目录：`src/test/regress/sql/`
- 原始文件：`https://raw.githubusercontent.com/postgres/postgres/REL_17_STABLE/src/test/regress/sql/<file>`
- 许可：PostgreSQL License
- 范围：日常数据库 schema 资源 + `pg_dump` 输出形态；不追求覆盖 PostgreSQL 全部功能。
- 机器可读清单：[`corpus.json`](corpus.json)，固定到 commit `507f48a0a794a99a7dac1c6bd5e76fb15002ad09`。

CI 按需拉取：

```sh
manifest=public/postgres-regress/corpus.json
base=$(jq -r .raw_base "$manifest")
target=$(jq -r .target_dir "$manifest")
mkdir -p "$target"
jq -r '.files.core[]' "$manifest" | while read -r file; do
    curl -fsSL "$base/$file" -o "$target/$file"
done
```

把 `.files.core[]` 换成 `.files.extended[]` 或 `.files.pg_dump[]` 即可切换语料层。

## A. 核心对象（CI 必跑，23 个文件）

| 资源 | 文件 |
| --- | --- |
| 表 / 列 / 分区 | `create_table.sql` |
| `LIKE` / 继承表 | `create_table_like.sql`, `inherit.sql` |
| 约束 | `constraints.sql` |
| 索引 | `create_index.sql` |
| 视图 / 物化视图 | `create_view.sql`, `matview.sql` |
| 序列 | `sequence.sql` |
| schema | `create_schema.sql` |
| 类型 / 域 | `create_type.sql`, `domain.sql` |
| 函数 / 过程 | `create_function_sql.sql`, `create_procedure.sql` |
| 触发器 / 规则 | `triggers.sql`, `rules.sql` |
| `ALTER TABLE` / 改列类型 | `alter_table.sql`, `alter_column_type.sql` |
| 注释 | `comments.sql` |
| 权限 / 角色 | `privileges.sql`, `create_role.sql` |
| 行级安全 / policy | `rowsecurity.sql` |
| 生成列 / identity | `generated.sql`, `identity.sql` |

## B. pg_dump 常见扩展（stress，20 个文件）

| 资源 | 文件 |
| --- | --- |
| 索引变体 / 覆盖索引 | `create_index_spgist.sql`, `index_including.sql`, `index_including_gist.sql` |
| 排序规则 | `collate.sql` |
| 转换 / 聚合 / cast / operator / AM | `conversion.sql`, `create_aggregate.sql`, `create_cast.sql`, `create_operator.sql`, `create_am.sql` |
| 全文检索 | `tsearch.sql`, `tsdicts.sql` |
| 事件触发器 | `event_trigger.sql` |
| 外部数据 | `foreign_data.sql` |
| 发布 / 订阅 | `publication.sql`, `subscription.sql` |
| 安全标签 / 表空间 | `security_label.sql`, `tablespace.sql` |
| 通用修改 / operator 修改 | `alter_generic.sql`, `alter_operator.sql` |
| C 函数 | `create_function_c.sql` |

## C. pg_dump 输出形态（smoke，6 个文件）

| 形态 | 文件 |
| --- | --- |
| `COPY ... FROM stdin` 数据块 | `copy.sql`, `copy2.sql`, `copydml.sql` |
| `pg_dump --inserts` | `insert.sql` |
| CTAS / `SELECT INTO` | `select_into.sql` |
| `SET` / `GRANT` / 表空间前置语句 | `test_setup.sql` |

## 官方目录没有独立文件

- `CREATE TABLE AS`：继续用 [`public/sqlfluff/postgres/create_table_as.sql`](../sqlfluff/postgres/create_table_as.sql)。
- `CREATE EXTENSION`：官方回归没有独立文件；`pg_dump` 会在 schema 头部生成，建议用真实 dump smoke 覆盖。
- `ALTER INDEX`：官方回归没有独立文件；索引修改分散在 `create_index.sql` / `indexing.sql`。若结构规则要覆盖 `ALTER INDEX`，应另加本地 fixture，而不是从官方名单里硬找。

## 验证快照

2026-11-27 用当前 formatter 对 49 个白名单文件跑 `fix --write` 后再 `check`：

- 48 个文件零 findings；
- 1 个文件共 2 条结构 finding：`collate.sql`（`cte close should align with with`、`from should start a new line`）。
- 相比 2026-11-26 的 14 文件 / 57 findings，残留减少到 1 文件 / 2 findings。
- 本轮同时修复了整文件 tokenizer 失败导致 `insert/values/on conflict` 全文件跳过的问题；残留在 `with ... as (...)` 的闭括号缩进与主查询 `from` 断行，属于已知布局缺口。

因此 Tier A/B 是**导入白名单**，不保证整文件零 finding；严格 CI 门禁应只跑文件中的 DDL 语句，或继续使用现有小型 fixture。

## 不收录

- DQL 专项：`select*.sql`、`join*.sql`、`window.sql`、`aggregates.sql`、`subselect.sql` 等。
- 数据类型/运算专项：`numeric*.sql`、`json*.sql`、`arrays.sql`、`text.sql`、`geometry.sql` 等。
- DML/事务/维护：`update.sql`、`delete.sql`、`merge.sql`、`vacuum*.sql`、`lock.sql`、`transactions.sql` 等。
- 平台/编码/排序专项：`collate.icu.utf8.sql`、`collate.linux.utf8.sql`、`encoding.sql`、`euc_kr.sql` 等。
- 超大或非 schema 资源：`numeric_big.sql`、`stats_ext.sql`、`partition_join.sql`、`plpgsql.sql` 等。

## pg_dump 支持边界

当前 formatter 对 `pg_dump` 输出的处理：

- `\restrict` / `\unrestrict` 以及其他 psql 反斜杠命令行：原样保留，不参与 SQL 切分。
- `COPY ... FROM stdin`：从 `COPY` 头到 `\.` 终止符整体视为不透明数据块，逐字节保留；块之后的 SQL 继续格式化。
- schema-only 输出：`SET`、`SELECT pg_catalog.set_config(...)`、`CREATE ...`、`ALTER ... OWNER`、`COMMENT`、`GRANT` 等按现有 token/结构规则处理。
- `pg_dump --inserts` / `--column-inserts`：走 `INSERT` 语句处理；`INSERT` 本身目前只做 token 规范化。
- 数据块内部的 `;`、Tab、反斜杠不参与语句切分，避免误改 `COPY` 数据。

对应回归：`source/oh-my-lint-lib/src/sql/splitter.rs` 的
`skips_pg_dump_meta_and_copy_data`。
