# oh-my-lint

按房规检查并修复代码的命令行工具。核心约束是**最小 diff**：检查与修复都以语法块为单位，块外的字节一个也不动；修复结果附带统一 patch，便于人工复核或交给 agent。

v0.1 只实现 **SQL 断行与 token 规范化**（规则 `sql-break`）：DQL 按子句复杂度决定内联或逐元素换行；DML（`delete`/`explain`）与 DDL（`create table`、`create index`、`alter table`、CTAS）按各自版式渲染，token 层覆盖大小写、空格、类型别名与默认形式省略。SQL 是第一个切片，不是完整产品；其它语言的适配器见路线图。

## 结构

| 路径                                | 说明                                                          |
| ----------------------------------- | ------------------------------------------------------------- |
| `Cargo.toml`                        | Cargo workspace                                               |
| `source/oh-my-lint-lib/`            | 规则模型、块抽取、修复引擎、SQL 规则                          |
| `source/oh-my-lint-cli/`            | 二进制 `oh-my-lint`：`check`、`fix`、`rules`、`explain`        |
| `source/oh-my-lint-macros/`         | 从子句枚举的属性生成策略选择与规则说明                        |
| `script/oh-my-lint.sh`              | 本地与 CI 共用入口：检查、构建、跨平台打包                    |
| `.github/workflows/release.yaml`    | 只准备环境并调用 script；构建与打包逻辑在脚本里               |
| `LICENSE`                           | UNLICENSE                                                     |

## 命令

| 命令                                                                              | 说明                                                                          |
| --------------------------------------------------------------------------------- | ----------------------------------------------------------------------------- |
| `oh-my-lint check <paths> [--dialect <name>] [--json] [--statistics] [--max-warnings N]` | 检查 SQL 块；`--max-warnings` 默认 `0`，findings 数超过它才以 `1` 退出 |
| `oh-my-lint fix <paths> [--dialect <name>] [--dry-run] [--diff] [--write] [--unsafe]` | 修复 SQL 块；默认 dry-run，只有 `--write` 才写回文件 |
| `oh-my-lint fix --file <path> --range L:C-L:C [--dry-run] [--diff] [--json]`      | 只修复落在指定行列范围内的块                                                  |
| `oh-my-lint fix --stdin [--stdin-filename <path>] [--lang sql]`                   | 从标准输入读代码，把修复后的块文本写到标准输出                                |
| `oh-my-lint rules [--json]`                                                       | 列出内置规则名及说明                                                          |
| `oh-my-lint explain <rule-or-code>`                                               | 按规则族或编码查说明，例如 `oh-my-lint explain break-many-conditions`          |

退出码：`0` 干净或修复后无剩余发现；`1` 有 findings，或 dry-run 下会改动，或仍有 skipped；`2` 参数、I/O、解析或修复被拒绝。

`check --json` 输出 `{ "findings": [...], "summary": {...} }`；`fix --json` 输出数组，每项为 `{ "file", "block_range", "fixed_text", "patch", "applied", "skipped", "remaining", "written" }`。

查询的 `WHERE`、`HAVING`、`JOIN ... ON` 条件列表可以在诊断里带 `code`，在修复结果里带 `applied_codes`。`rule` 和 `applied` 仍使用规则族 `sql-break`。编码只用于归因、统计和查说明，不能开关规则或按编码选择修复。SQL 文本和 diff 输出不混入编码摘要。

规则编码使用英文 kebab-case；命令帮助、诊断和规则说明只提供英文。

维护条件规则时，改 [`ConditionStrategy`](source/oh-my-lint-lib/src/sql/postgresql/dql/where.rs) 对应枚举项的 `#[rule(...)]`。`when` 是判据，`output` 是版式；`#[clause_rules]` 按声明顺序选中第一条匹配规则，同时生成 `check`、`fix` 和说明目录使用的规则名。渲染逻辑仍由 `ConditionLayout` 共享。

## 支持范围

| 载体                            | 支持                                                          |
| ------------------------------- | ------------------------------------------------------------- |
| `.sql` 文件                     | 整个文件作为一个 SQL 块                                       |
| Python 模块级 `SQL = """..."""` | 按常量抽取 SQL 块；函数内的同名局部量不算                     |
| 其它语言                        | 尚未接入                                                      |

规则 `sql-break` 覆盖：DQL 的子句断行、布尔组断行与锚点、含子查询的单条件括号断行、独立 `case` 展开、内联/多层标量子查询展开、函数实参递归展开、`from` 的 join 版式与隐式 `cross join` 改写；DML 的 `delete`、`explain`、`update`、`insert ... select` 与 `WITH` CTE 分行；DDL 的 `create table`（列顺序、对齐、表约束、表属性）、`create index`、`alter table`、CTAS，以及同表 DDL 的分组紧排与索引对齐。token 层覆盖大小写、空格、`<>`/`outer`/`asc`、别名 `as`、派生表别名与 CTE 字段列表、`distinct`、`case` 括号与 `not` 改写，以及 R35–R39 的 PG 类型字面量、类型别名、命名参数、默认形式省略与关键字括号；`VALUES` 行构造器按 M3 逐元组断行，`insert` 字段列表收回头行，`on conflict` 另起一行。

所有块按 `--dialect`（`postgres`/`mysql`/`duckdb`/`hive`，默认 `postgres`）交给 sqlparser 做词法与语法分析；字符串、注释、引号按方言识别（`$$`、`E''`、`#` 注释、反引号）。R34 逐语句解析：解析失败的语句逐字节保留，不跑布局与 token 规则、不报 finding、不产出修复，且不影响同一块中其他可解析语句。所有解析成功的语句（包括 `create policy`、`create function`、`grant` 等 `Other` 类型）都执行 R25–R39 token 规范化；DQL/DML/DDL 的对应门类有结构版式。含注释的语句只做 token 规范化、保留注释和原结构；尾部注释不补分号。R35 的 PG 类型字面量改写只在 `postgres` 方言生效。

`pg_dump` 输出按 R34a 处理：`\restrict`/`\unrestrict` 等 psql 元命令行原样保留；`COPY ... FROM stdin` 从头部到 `\.` 视为不透明数据块，数据中的分号、Tab 与反斜杠不参与切分，块前后的 SQL 继续格式化。schema-only、`--inserts`/`--column-inserts` 和含 `COPY` 数据块的完整 dump 都走这一入口。官方回归语料的白名单见 [`public/postgres-regress/WHITELIST.md`](public/postgres-regress/WHITELIST.md)，机器可读清单见 [`public/postgres-regress/corpus.json`](public/postgres-regress/corpus.json)。

完整判据（术语、递归规则与版式）见 [`RULES.md`](RULES.md)。RULES.md 按危险程度组织为 DQL → DML → DDL；v0.1 代码已实现这三类版式与 R34 的逐语句跳过，DML/DDL 覆盖 `delete`、`explain`、`create table`、`create index`、`alter table` 与 CTAS。

## 最小 diff 契约

- 修复只应用 `FixScope::Local`；`FileConsistent` 与 `Structural` 的修复进入 `skipped`。
- 只替换块内文本；块外字节保持不变，块之间互不影响。
- 元素内部文本（字符串字面量、标识符）逐字节保留；修复会改换行、缩进、锚点、连接词、join 关键字，并把隐式 cross join 写成显式 `cross join`。
- 格式化幂等：`format(format(x)) == format(x)`；合规输入不做任何改动。
- 文本末尾（最后一条被 formatter 处理的语句）必须有分号；缺时补一个，已有分号保留。文件以单个 `\n` 结尾，语句之间最多保留一行空行。
- 对布局模块支持的语句，修复后重新 `check` 为空；只走 token-only 回退的语句可能仍保留结构 finding（如 `VALUES` 派生表、集合运算、`SELECT INTO`），这类 finding 留待规则补齐；被 `skipped` 的块保持原样，`check` 继续报告。
- 无法解析的语句正文按 R34 逐字节保留、不报告；前导注释仍会删除，`check` 不输出解析错误，也不猜结构。
- 修复覆盖 DQL 的子句、布尔组、含子查询的单条件括号、独立 `case`、内联/多层子查询、函数实参、隐式 cross join、from join 换行、`with` 框架、集合运算与 `rollup` 参数；DML 的 `delete`/`explain`/`update`/`insert ... select`/`WITH` CTE；DDL 的 `create table` 对齐与约束、`create index`、`alter table`、CTAS 与同表 DDL 分组。所有解析成功的其他语句只做 token 层修复：大小写、空格、`as`、`distinct`、`case` 括号、`between` 括号、`not` 改写与 R35–R39；可解析语句的 `--`/`/* */` 注释直接删除，无法解析语句正文保留、前导注释删除。

## 构建与测试

本地与 CI 都走同一个脚本；GitHub Action 只负责 checkout、toolchain 和 `cross` 这类环境准备。

```bash
# 全量检查：fmt / clippy / test / release build / smoke
bash script/oh-my-lint.sh job__check

# 单个步骤：step__fmt / step__clippy / step__test / step__build / step__smoke
bash script/oh-my-lint.sh job__check step__clippy

# 打包一个目标；产物在 dist/
bash script/oh-my-lint.sh job__package aarch64-apple-darwin
```

`job__package` 对 `*-linux-musl` 目标调用 `cross`，其余目标调用 `cargo`；Unix 打 `tar.gz`，Windows 打 `zip`。

## 发布

每次 push 到 `main` 或手动派发，工作流先跑 `job__check`，再按矩阵准备环境并调用 `job__package <target>`；六个目标由脚本构建、打包，最后 upsert 名为 `latest` 的 release，同名资产会被覆盖。Action 里没有 cargo/tar/zip 逻辑。

| 平台          | target                     | 资产                                          |
| ------------- | -------------------------- | --------------------------------------------- |
| Linux musl amd64 | `x86_64-unknown-linux-musl` | `oh-my-lint-x86_64-unknown-linux-musl.tar.gz` |
| Linux musl arm64 | `aarch64-unknown-linux-musl` | `oh-my-lint-aarch64-unknown-linux-musl.tar.gz` |
| macOS amd64   | `x86_64-apple-darwin`      | `oh-my-lint-x86_64-apple-darwin.tar.gz`       |
| macOS arm64   | `aarch64-apple-darwin`     | `oh-my-lint-aarch64-apple-darwin.tar.gz`      |
| Windows amd64 | `x86_64-pc-windows-msvc`   | `oh-my-lint-x86_64-pc-windows-msvc.zip`       |
| Windows arm64 | `aarch64-pc-windows-msvc`  | `oh-my-lint-aarch64-pc-windows-msvc.zip`      |

安装：从 <https://github.com/openhundun/oh-my-lint/releases/latest> 下载对应资产，解压后把二进制放进 `.agents/__data__/bin/`。

## 路线图

| 方向           | 内容                                                                    |
| -------------- | ----------------------------------------------------------------------- |
| 语言适配器     | Python、JavaScript/TypeScript、Shell 的块抽取与各自规则                 |
| 修复能力       | 内联子查询展开、可解析语句注释剥离；再谨慎引入 `FileConsistent`         |
| 宿主集成       | Biome、checkstyle、ast-grep；把规则包接入现有工具，而不是重造整条流水线 |
