# SQL 策略化重构路线图

目标：把当前“通用 checker + fix 文本手术”的 DQL 实现，收敛为“每个子句自己描述输入/输出模式”的策略式布局器；`check` 与 `fix` 共用同一棵布局树，消除两者行为漂移。DDL/DML 已经是整句 layout，主要做拆分和接口收敛。

本文件是长期规划与 todo 清单；具体规则判据仍以 [`RULES.md`](RULES.md) 为准。

## 基线

| 项目 | 基线 |
| --- | --- |
| 起始提交 | `13b0f45` |
| 单元测试 | `cargo test -p oh-my-lint-lib --lib`：212 passed |
| DDL/DML fixture | 7 个，`check` 全绿 |
| 公司脚本 | 188 文件，`188 blocks, 0 findings` |
| PostgreSQL 白名单 | 49 文件；当前 1 文件 2 findings（见 `public/postgres-regress/WHITELIST.md`） |
| sqlfluff PostgreSQL | 200+ 文件，作为 smoke/幂等语料，不要求全绿 |

## 设计约束

1. **公共 API 不变**：`sql::check`、`sql::check_with`、`sql::format`、`sql::format_with` 继续作为测试与 CLI 的契约。
2. **core 与方言分离**：`lexer`、`token`、`splitter`、`astmap`、`parse`、关键字分类是方言无关核心，放在 `sql/core`；Postgres 房规放在 `sql/postgresql`。
3. **单一 renderer**：子句模块输出带 span 的布局文本；`check` 比较原文与规范输出，`fix` 替换差异区间。迁移完成前旧 `checker/fix` 作为 fallback。
4. **子句递归**：语句模块只负责自身骨架；子查询、函数实参、`case`、`values` 等交给子节点 renderer，按 `Input`/`Output` 枚举显式传递上下文。
5. **文本树优先**：从 sqlparser AST 只取结构与 span，不直接用 AST 重建 SQL；保留原始字面量、引号和不可解析语句字节。
6. **文件层最后做**：跨语句的分组、排序、对齐、注释归并只放 `file/`；子句模块永远不跨语句。
7. **token 规则保持单层**：大小写、空格、运算符、类型别名规则不能拆到各个关键字模块，否则风格会漂移。
8. **不引入行宽**：所有断行仅按语法/词法结构判定。

## 目标目录

```text
sql/
├── mod.rs                    # 公共 check / format；语句切分与派发
├── core/                     # dialect-neutral
│   ├── lexer.rs
│   ├── token.rs
│   ├── splitter.rs
│   ├── astmap.rs
│   ├── parse.rs
│   ├── keywords.rs
│   ├── layout.rs             # Mode / Ctx / Rendered / span
│   └── dispatch.rs
├── postgresql/
│   ├── dql/
│   │   ├── mod.rs
│   │   ├── select.rs / from.rs / where.rs
│   │   ├── group_by.rs / having.rs / order_by.rs
│   │   ├── join.rs / on.rs
│   │   ├── with.rs / values.rs
│   │   ├── in.rs / between_and.rs
│   │   └── over.rs / filter.rs / case.rs
│   ├── dml/
│   │   ├── insert.rs / update.rs / delete.rs
│   │   └── using.rs / on_conflict.rs
│   ├── ddl/
│   │   ├── create_table.rs / create_index.rs
│   │   ├── alter_table.rs / comment_on.rs
│   │   └── create_view.rs
│   └── file/
│       ├── reorder.rs
│       ├── realign.rs
│       └── remerge.rs
└── mysql/                    # 规则真正分化后再建，不提前放空目录
```

## 性能基线

| 场景 | 重构后 | 说明 |
| --- | ---: | --- |
| 公司 188 文件 / 2.33 MB `check` | 123s → **66s** | check=render-diff 后单次渲染；空 findings 仍跳过 fix |
| 204 KB 单文件 | 8.77s | 大文件仍受多次 parse/tokenize 影响 |
| 21 KB 单文件 | 2.00s | 小文件固定开销明显 |
| 212 单测 | 0.67s | 行为测试不是性能门禁 |

后续性能工作：fix 批处理、parse/token 缓存、文件级并行；详见评估报告。

## 子句输入/输出

每个子句模块自带上下文枚举，由 `dql::dispatch` 穷尽匹配。以 `values` 为样板：

```rust
enum ValuesInput {
    ForceInline,
    ForceExline,
    CtxInsert,
    CtxSubquery,
}

enum ValuesOutput {
    Inline { text: String, span: Span },
    Exline { lines: Vec<String>, span: Span },
}
```

布局树节点保留源 span；`check` 依据节点期望输出与原文的差异生成 finding，`fix` 直接替换节点 span。这样 check/fix 不再各写一套判断。

## 阶段与验收

### P0 基线与规划

- [x] 记录本文件与 todo 清单
- [x] 固定起始提交、测试数、公司语料结果
- 验收：后续每个阶段都以同一组门禁对比

### P1 核心抽取（零行为变化）

- [x] `lexer` / `token` / `splitter` / `astmap` / `parse` 迁入 `sql/core`
- [x] `is_function_name` 与结构关键字表迁入 `core/keywords`
- [x] `sql/mod.rs` 保留兼容导出，公共 API 不变
- [x] 现有测试只允许改 `use` 路径，不允许改断言
- 验收：212 tests 全绿；公司脚本 188/0；`format` 输出与基线逐字节一致

### P2 `values` 垂直切片

- [x] `values` 显式拆成 `ValuesInput` / `ValuesOutput`
- [x] 覆盖 insert values、派生表 values、单行/多行、`on conflict`
- [x] 旧 `values::format` 保留为 adapter
- 验收：现有 values 测试、公司文件、DDL/DML fixture 全绿

### P3 DQL 子句接入

- [x] `select` / `from` / `where` / `group by` / `having` / `order by` 独立文件
- [x] `join` 独立文件；`on` 复用条件/join 版式；`in` / `between` 由表达式与 token 策略处理
- [x] `with` / 集合运算 / `case` / `rollup` 独立文件
- [x] `over` / `filter` 保留在 `expr` / token 策略中：它们是函数修饰符，不是独立子句布局，不强行拆文件
- [x] 嵌套子查询与函数实参递归拆入 `subquery` / `function`
- [x] 旧 checker/fix 保留 fallback；DQL 结构逻辑已收敛到 `postgresql/dql`
- 验收：DQL 行为测试、7 fixture、公司语料无回归；`format` 幂等

### P4 DML 拆分

- [x] `insert` / `update` / `delete` / `explain` 拆入 `postgresql/dml`
- [x] `using` 归入 delete；`on conflict` 归入 `dml/on_conflict`
- [x] CTE 经 `format_cte_prefix` 复用 DQL 版式
- [x] DML 测试与公司语料回归

### P5 DDL 拆分

- [x] `create table` 独立；`create index` / `alter table` / CTAS 共享 `misc` 的 token/空格策略；`comment on` 在 `file/realign`
  - 不再把 `misc` 拆成三个薄文件：三者共用 `inline`/`space_parens`/`depth_zero_word`，强行拆会复制或暴露内部 helper
- [x] 列对齐、约束排序、同表 DDL 分组保持
- [x] DDL/DML fixture 与 PG 白名单不新增 findings

### P6 文件层收起

- [x] `groups` → `file/reorder`
- [x] `comments` 的 `comment on` 对齐 → `file/realign`
- [x] pg_dump 不透明区间保持
- [x] 全量 fix 幂等（`no changes`）
- 验收：公司脚本 188/0；全量 fix 幂等

### P7 旧引擎清理

- [x] 删除 check 侧旧分支；DQL `checker/fix` 仅作为 `format_statement` 内部 planner/renderer，不再存在第二条检查路径
- [x] `check` 统一为 render-diff：逐语句比较 `format_statement`
- [x] 删除已无调用的内部测试与 helper（调试测试、`statement_rest` 等）
- 验收：`check(format(sql))` 为空；`format(format(sql)) == format(sql)`；测试与语料门禁全绿；PG 白名单残 2 findings

## 测试与语料门禁

每个阶段至少运行：

```sh
cargo fmt --all
cargo test -p oh-my-lint-lib --lib
cargo clippy --workspace --all-targets -- -D warnings
git diff --check
```

涉及公司语料时：

```sh
./target/release/oh-my-lint check --statistics --dialect postgres /opt/code/openhundun-workspace/.agents/__code__/company-sql
./target/release/oh-my-lint fix --write --dialect postgres /opt/code/openhundun-workspace/.agents/__code__/company-sql
```

语料分层：

1. **精确门禁**：7 个 DDL/DML fixture + 行为测试的精确字符串。
2. **差分门禁**：公司 188 文件与 PG 白名单；只允许 findings 减少，不允许新增。
3. **smoke 门禁**：200+ sqlfluff PG 文件；格式幂等、不可解析语句逐字节保留、无 panic。

## 风险与回退

- DQL 是唯一真正的重写区；DDL/DML 以移动和拆分为主。
- `astmap` 目前只有 Query span；子句级 span 是 P1/P3 的主要基础工作。
- 旧 `checker/fix` 作为 renderer 内部 planner/renderer 保留；check 已不再走独立违规检测路径。
- 文件层重排风险最高，必须最后接入并有公司语料差分。
- 白名单当前 2 findings（`collate.sql` 的 CTE 闭括号与主查询 `from`），作为已知布局缺口记录。

## 进度

| 阶段 | 状态 | 提交 | 备注 |
| --- | --- | --- | --- |
| P0 基线与规划 | done | — | 本文件 |
| P1 核心抽取 | done | — | `core/` 已抽出，212 tests / 公司 188/0 |
| P2 values | done | — | `ValuesInput` / `ValuesOutput` + adapter |
| P3 DQL | done | — | 子句/递归已拆；over/filter 保留在表达式策略 |
| P4 DML | done | — | insert/update/delete/explain + on conflict |
| P5 DDL | done | — | create_table/misc/realign；白名单 2 findings（基线 57） |
| P6 file | done | — | reorder/comments/realign；fix 无 changes |
| P7 清理 | done | — | check=render-diff；旧 check 分支删除；215 tests |
| P8 条件块 | done | — | Break 整块加括号；where/having/on/DML 共用；231 tests；公司 0；PG 2 |

## P8 条件块

- `where` / `having` / `on` 使用同一个 `where` 条件解析器；断行时整块加 `(` / `)`，锚点放在块内，闭括号回关键字缩进。
- 内联条件不保留结构化外层括号；`ON (a = b)` 这类单条件包裹会被解开。
- DML 的 `where` 复用同一渲染；CTAS 的简单判定会先解开结构化外层括号，避免把条件块重新压回一行。
- `on` 从 `join_starts` 之后的第一个深度 0 `on` 取正文，遇到下一个 join 或顶层逗号截止。
- CTE 字段列表判定要求 `as` 后紧跟 `(`，避免 `with` 查询里的 `round(...) as alias` 被误判成 CTE 列而加空格。
- `over (...)` 内的 `case` 强制内联，窗口整体仍按简单表达式处理；窗口外的独立 `case` 继续按分支复杂度展开。
- 子查询只按 `is_simple` 判定是否内联，不再额外按子句数量强制断行；`select` / `from` / `where` 三个简单子句也保持一行。
- 任意嵌套查询或条件布尔组都会使父级退出 simple；条件里任意层级的布尔组都会触发外层断行，避免 q07/q85/q88 这类嵌套条件被压成一行。
