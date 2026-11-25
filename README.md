# oh-my-lint

按房规检查并修复代码的命令行工具。核心约束是**最小 diff**：检查与修复都以语法块为单位，块外的字节一个也不动；修复结果附带统一 patch，便于人工复核或交给 agent。

v0.1 只实现 **SQL 断行决策**（规则 `sql-break`）：根据子句复杂度决定内联还是逐元素换行。SQL 是第一个切片，不是完整产品；其它语言的适配器见路线图。

## 结构

| 路径                                | 说明                                                          |
| ----------------------------------- | ------------------------------------------------------------- |
| `Cargo.toml`                        | Cargo workspace（`source/oh-my-lint-lib`、`source/oh-my-lint-cli`） |
| `source/oh-my-lint-lib/`            | 规则模型、块抽取、修复引擎、SQL 规则                          |
| `source/oh-my-lint-cli/`            | 二进制 `oh-my-lint`：`check`、`fix`、`rules`、`explain`        |
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
| `oh-my-lint rules [--json]`                                                       | 列出已注册规则                                                                |
| `oh-my-lint explain <rule>`                                                       | 输出一条规则的说明                                                            |

退出码：`0` 干净或修复后无剩余发现；`1` 有 findings，或 dry-run 下会改动，或仍有 skipped；`2` 参数、I/O、解析或修复被拒绝。

`check --json` 输出 `{ "findings": [...], "summary": {...} }`；`fix --json` 输出数组，每项为 `{ "file", "block_range", "fixed_text", "patch", "applied", "skipped", "remaining", "written" }`。

## 支持范围

| 载体                            | 支持                                                          |
| ------------------------------- | ------------------------------------------------------------- |
| `.sql` 文件                     | 整个文件作为一个 SQL 块                                       |
| Python 模块级 `SQL = """..."""` | 按常量抽取 SQL 块；函数内的同名局部量不算                     |
| 其它语言                        | 尚未接入                                                      |

规则 `sql-break` 覆盖：子句断行、布尔组断行与锚点、含子查询的单条件括号断行、独立 `case` 展开、内联子查询展开、`from` 的 join 版式与隐式 `cross join` 改写。token 层覆盖大小写、空格、`<>`/`outer`/`asc`、别名 `as`、`distinct`、`case` 括号与 `not` 改写。

所有块先按 `--dialect` 解析（`generic`/`postgres`/`mysql`/`sqlite`/`hive`/`snowflake`/`bigquery`，默认 `generic`）；解析失败只报 `sql does not parse`，不跑布局规则、不产出修复。

完整判据（术语、递归规则与版式）见 [`RULES.md`](RULES.md)。

## 最小 diff 契约

- 修复只应用 `FixScope::Local`；`FileConsistent` 与 `Structural` 的修复进入 `skipped`。
- 只替换块内文本；块外字节保持不变，块之间互不影响。
- 元素内部文本（字符串字面量、标识符）逐字节保留；修复会改换行、缩进、锚点、连接词、join 关键字，并把隐式 cross join 写成显式 `cross join`。
- 格式化幂等：`format(format(x)) == format(x)`；合规输入不做任何改动。
- 对 `fix` 接受的块，修复后重新 `check` 为空；被 `skipped` 的块保持原样，`check` 继续报告。
- 块无法按选定方言解析时不修复；`check` 只输出解析错误，不猜结构。
- 修复覆盖子句、布尔组、含子查询的单条件括号、独立 `case`、内联子查询、隐式 cross join、`with` 框架、集合运算与 `rollup` 参数；token 层覆盖大小写、空格、`as`、`distinct`、`case` 括号、`between` 括号与 `not` 改写。

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
| 修复能力       | 内联子查询展开、注释保留；再谨慎引入 `FileConsistent`                   |
| 宿主集成       | Biome、checkstyle、ast-grep；把规则包接入现有工具，而不是重造整条流水线 |
