# SQL 断行与 token 规则

断行只回答"这段文本能不能一行读完"；token 规范化只回答"同一语义的规范写法"。判定自下而上分层，每层只读下一层的结果。

章节按危险程度排序：**DQL（只读）→ DML（改数据）→ DDL（改结构）**。术语、token 和解析门禁是三者共用部分，放在最前。

## 通用

### 术语

- T1 `语句块`：深度 0 的分号或「空行 + select/with」切出的文本；根语句、子查询体、CTE 体、集合分支同构。
- T2 `子句`：深度 0、最长匹配的关键字（`select` `from` `where` `on` `group by` `having` `order by` `limit` `union all` `union` `intersect` `except`）；`with` 单独成行，不算子句。
- T3 `元素`：`where`/`on`/`having` 按深度 0 的 `and`/`or` 切分（`between … and` 不切），其余子句按深度 0 逗号切分；首个元素是 `1 = 1`/`1 = 0` 锚点时丢弃，不计入元素数。
- T4 `布尔组`：深度 0 含 `and`/`or` 的括号内容；函数名、限定名、子查询之后的括号不算。
- T5 `类型修饰` 与 `列约束`：`storage`/`compression`/`collate` 是列定义里的类型修饰；`not null`/`default`/`generated`/`unique`/`check`/`references` 是列约束。主键是表级约束，不挂在字段后面。

### token

- R25 关键字与裸标识符小写；单引号字面量、反引号/双引号标识符逐字节保留。
- R26 文本末尾（最后一条被 formatter 处理的语句）必须有一个分号；语句之间的原有分隔符保留。尾部含注释时不强制补分号，避免把分号写进注释。
- R27 二元运算符两侧空格（`= != < <= > >= + - * /`）、函数与类型括号紧贴、逗号前无空格后一个空格、行尾无空格；`<>` 改写为 `!=`；`left outer join`/`full outer join` 的 `outer` 按 R38 省略。
- R28 字段别名必须有 `as`；表与派生表别名裸写；独立 `case` 表达式加圆括号（函数实参内不加）；条件位置的 `between` 写成 `(x between a and b)`，`select` 值表达式不加；`distinct(x)` 改写为 `distinct x`；`over (`/`rollup (` 紧贴为 `over(`/`rollup(`；`nulls first` 位于排序方向之后；`not x is null` 改写为 `x is not null`，`not x = 1` 改写为 `x != 1`；`order by` 位置序号不改写。
- R35 PG 类型字面量改为强转：`date '…'`/`time '…'`/`timestamp '…'`/`timestamptz '…'`/`interval '…'` 写成 `'…'::date` 等；interval 内容里的单位词小写单数（`'8 HOURS'`→`'8 hour'`、`'3S'`→`'3 second'`、`'1 HOUR 2 MINUTES'`→`'1 hour 2 minute'`），数字与其余字面量内容保留。只对 postgres 方言生效。
- R36 类型别名归一到受限子集：`int`（`integer`/`int4`）、`bigint`（`int8`）、`smallint`（`int2`）、`boolean`（`bool`）、`varchar`（`character varying`/`char varying`）、`char`（`character`）、`decimal`（`numeric`）、`float4`（`real`）、`float8`（`double precision`）、`timestamptz`（`timestamp with time zone`）、`serial`/`bigserial`/`smallserial`；`timestamp without time zone`→`timestamp`，`time without time zone`→`time`，`time with time zone`→`timetz`。`numeric` 只在类型位置改，`collate numeric` 这类排序规则名不动。
- R37 命名参数等号两侧不加空格（`with (fillfactor=70)`、`set (parameter_1='value')`、`opclass (opt1=1)`）；表达式里的 `a = b` 仍有空格。
- R38 默认形式省略：显式 `null`、`asc`、`nulls distinct`、ASC 下的 `nulls last`、DESC 下的 `nulls first`、`match simple`、`not deferrable initially immediate`、`on commit preserve rows`、列定义里的 `storage default`/`compression default`、`without oids`、`outer`。保留非默认形式；两种非默认形式并存时，右对齐区分后缀（`distinct`、`first`/`last`）。`alter … set storage default` 是 action，不省略。
- R39 SQL 关键字构造与括号之间留一个空格：`primary key (`、`unique (`、`check (`、`foreign key (`、`references t (`、`exclude using gist (`、`include (`、`with (`、`set (`、`options (`、`using gin (`；类型和函数调用紧贴：`varchar(40)`、`now()`、`nextval(…)`、`char_length(…)`。

### 解析门禁

- R34 语句块先按 `--dialect`（`postgres`/`mysql`/`duckdb`/`hive`，默认 `postgres`）解析。解析成功才跑布局与 token 规则，输出需再次解析通过；解析失败则跳过该块、逐字节保留原样，不报 finding、不产出修复，并记录上游不支持的构造。跳过策略也让 handlebars 之类的非 SQL 片段原样通过。
- R34a `pg_dump` 输出先识别不透明区间：以 `\` 开头的 psql 元命令行（`\restrict`/`\unrestrict`/`\connect` 等）和 `COPY ... FROM stdin` 到行首 `\.` 的数据块不参与解析、token 与结构规则，逐字节保留；这些区间前后的 SQL 仍按 R34 处理。数据块内的分号、Tab、反斜杠不参与语句切分；`COPY` 头也保持原样。

## DQL

### 表达式

- R1 `原子表达式`：不含子查询、`case … end`、`over(`、复杂函数调用、布尔组，且 `in` 列表不超过 3 个常量；`cast(单字段或字面量 as 类型)`、`coalesce`/`nullif`（所有参数都是简单操作数）、零参 `current_date()`/`current_timestamp()`、单列聚合（`count`/`sum`/`avg`/`min`/`max` 与 `stddev`/`variance` 家族，参数为单个字段、`*` 或字面量，可带 `distinct`）视为原子。
- R2 `复杂表达式`：含子查询、`case … end`、`select`/`order by` 位置的 `over(`、上述白名单之外的函数调用、布尔组，或 `in` 列表超过 3 个常量；`greatest`/`least`、`substring`、`concat`、`abs` 等仍算复杂，`sum(a * b)` 这类参数不是单个字段的聚合也算复杂。
- R3 复杂度只决定"元素 ≥2 的子句是否断行"；单元素子句永不逐元素断行。
- R4 独立 `case` 元素：任一分支持子查询或为复杂表达式时展开，`(case`、`when …` 与 `then …`、`else …`、`end)` 各独占一行并缩进 +4；函数实参内的 `case` 不因本规则展开。
- R5 子查询展开优先：函数实参或 `case` 分支里出现非简单子查询体时，子查询按 R11 展开，外层不额外折行。

### 子句

- R6 `from` 断行：元素 ≥4，或任一元素含 `join`，或元素 ≥2 且含派生表（`(select`/`(with`/`(values`）。
- R7 `select` 断行：元素 ≥4，或含复杂表达式，或元素恰为 3 且全部以 `as <标识符>` 结尾（标识符含裸名、双引号名、反引号名）。
- R8 `where`/`on`/`having`/`order by` 断行：元素 ≥4 或含复杂表达式。
- R9 `group by` 断行：元素 ≥4。
- R10 `limit` 不是元素列表，永不折行；`rollup(...)` 的参数按"元素 ≥4 或含复杂表达式"判定，展开时一元素一行、`)` 对齐 `rollup` 所在缩进。

### 语句与子查询

- R11 `简单语句块`：`select` 元素 ≤3、其他子句元素 ≤1、无 `with` 与集合运算、每个子句都不触发断行，且体内所有嵌套子查询体都简单。
- R12 不简单的语句块每个子句关键字独占一行；简单的语句块整行或各子句分行都合规，工具不合并子句行。
- R13 集合运算：运算符独占一行、与两侧分支同缩进；每个分支按语句块独立判定；分支不加括号。
- R14 `with`：`with` 独占一行；`<名称> as (` 与 `with` 同列；CTE 之间以 `),` 结尾；`)` 与 `<名称>` 同列；含 `with` 的语句不单行。
- R15 递归只发生在两层：子查询体按 R11、`case` 分支按 R4；其余构造不另设判定层。

### 版式

- R16 缩进每级 4 空格；子句关键字与语句首列同列，断行元素再 +4。
- R17 内联：`select`/`from`/`group by`/`order by` 用 `, ` 连接；`where`/`on`/`having` 用 ` and `/` or ` 连接。
- R18 断行：一元素一行；`select`/`from`/`group by`/`order by` 逗号留行尾、末元素不带逗号；跨行元素随所在行整体平移，内部相对缩进不变。
- R19 `where`/`on`/`having` 逐元素断行首行写锚点：首个连接词是 `or` 写 `1 = 0`，否则写 `1 = 1`；其后每行以 `and`/`or` 开头；单元素子句的块内换行不加锚点。
- R20 布尔组断行：`and (`/`or (` 起行，组内首行写 `1 = 1`（AND 组）或 `1 = 0`（OR 组），逐行 `and`/`or`，闭合 `)` 与组起始同缩进；组内联时丢弃锚点、以连接词连接。
- R21 子查询展开：`(` 留在元素行尾，体内各子句 +4，闭合 `)` 与元素缩进同列。
- R22 派生表：`from` 单元素时 `from (` 同行、`) <别名>` 与 `from` 同列；多元素时 `(` 独占元素位、`) <别名>` 与 `(` 同列。
- R23 `from` 与 join：首个表与 `from` 同行；每个 `join`/`cross join` 独占一行、缩进等于 `from`；`on` 与 1–3 个条件同行（元素 ≥4 或含复杂表达式时断行，`on 1 = 1` 起行）；显式 join 之后的逗号表改写为 `cross join`，纯逗号表列表保留。
- R24 常量 `in` 列表：超过 3 个常量只参与复杂度判定；列表本身不重排，整行或逐元素一行都合规，两形态不得混用。
- R33 单条件括号断行：条件位置的括号内只有一个条件且含子查询时，`(` 留在条件行尾，内容缩进 +4，`)` 回到条件行缩进；token 层不得因此再补一层括号。

## DML

- M1 `DELETE`：子句为 `with`、`delete from`、`using`、`where`、`returning`。简单判定同 R11：`returning` 元素 ≤3 且不触发 R7，其他子句元素 ≤1，`using` 无 join，CTE 各体简单；简单语句整行。非简单时每个子句独占一行、0 缩进；`where` 按 R8/R19，`returning` 按 R7，`using` 按 R6/R23；显式 join 的缩进等于 `using`；`returning` 别名必须有 `as`。`where` 超过一个条件即不算简单，但条件本身仍只在 ≥4 或复杂时逐元素断行。`with` 各部分都简单时允许整行。
- M2 `EXPLAIN`：`explain` 头与被执行语句必然分行，被执行语句在下一行、0 缩进。头部只占一行，选项以 `, ` 连接，不因数量折行；布尔值统一 `true`/`false`（`on`/`off`、`1`/`0` 只在布尔选项位置改写），裸选项保持裸写，`format text|xml|json|yaml` 与 `serialize none|text|binary` 保留；`analyse`/`analyze` 不互相归一。被执行语句按 DQL 处理。

## DDL

- D1 `CREATE TABLE` 主体：有表约束即视为复杂，整体断行：`create table <name> (` 首行，元素一行一个、+4，`);` 独立行。无表约束时：全部列只有 `name type`（`not null` 属于 type）≤3 列内联；任一列带 `default`/`generated`/`unique`/`check`/`references`/`storage`/`compression`/`collate` 等额外修饰时 ≤2 列内联；否则断行。单元素不因复杂度断行；空体 `()` 保持内联。
- D2 列声明顺序：`name type` → `storage` → `compression` → `collate` → `not null` → `default` → `generated` → `unique` → `check` → `references`。`constraint <name>` 必须紧贴它命名的约束，不能作为尾部独立段。主键提升为表级、独占一行（仅 CREATE TABLE；ALTER 的列级主键暂不拆，见 D7）；主键列必须显式 `not null`（typed table 写 `name with options not null`）。显式 `null` 按 R38 省略。
- D3 多行对齐：断行的列定义按 `name`、`type`、`storage`、`compression`、`collate`、`not null`、其余约束的顺序分列对齐；某一列出现类型修饰时，所有行保留该对齐列，空单元补空格；没有后续内容的行不补行尾空格。`not null` 虽然属于 type 语义，仍作为独立对齐列。内联主体不对齐；R38 的非默认变体右对齐。
- D4 表约束：`primary key (...)`、`unique (...)`、`foreign key (...) references ...`、`check (...)`、`exclude ...` 各占一行、缩进 +4。`primary key`/`unique`/`foreign key` 的字段参数始终同一行；`exclude` 元素列表在 ≤3 个简单元素时同行，≥4 或含复杂表达式时一元素一行，闭合 `)` 回约束缩进，后续 `include`/`with`/`using index`/`where` 各占一行。约束名用 `constraint <name>` 前缀；关键字与括号之间按 R39 留空格。
- D5 表属性：body 断行时，每个属性子句独占一行、0 缩进；body 内联时，属性子句 ≤2、每个属性的参数元素 ≤3、且无复杂表达式，则全部同行，否则每个属性一行。属性包括 `inherits`、`partition of`、`for values`、`partition by`、`using`、`with`/`without oids`、`on commit`、`tablespace`。不引入行宽阈值。
- D6 `CREATE INDEX`：整条语句一行；索引字段/表达式、`include`、`with`、`where`、`tablespace` 都不折行，字段参数始终同一行。`using <method> (` 留空格；命名参数按 R37。
- D7 `ALTER TABLE`：顶层 action 1–2 个时整条一行；≥3 个时 `alter table <name>` 首行，每个 action 一行、+4，逗号留行尾，最后一行带 `;`。action 内部：列定义按 D2，约束按 D4，`exclude` 列表按 D4，`set (...)`/`reset (...)` 同行，`attach partition ... for values ...` 同行，`using <expr>` 同行。暂不把 `add column ... primary key` 拆成多个 action。
- D8 `CREATE TABLE AS`：header（`create ... table ...` 及其 `on commit`/`using`/`without oids`/`tablespace`/`with (...)`）保持一行，`as (` 留在 header；查询体简单时整条一行，不简单时 body +4、`)` 回 0。查询体统一加括号，`as select …` 改为 `as (select …)`；`with no data` 接在 `)` 后同行，默认的 `with data` 按 R38 省略。查询体按 DQL。

## 不变量

- R29 断行不看行长，不按文件或语句另设字符阈值；表属性内联只看 D5 的结构条件。
- R30 幂等：格式化结果再次格式化逐字节不变；合规文本零改动；块外字节与块内未列入 R25–R28、R35–R39 的字符逐字节保留；R34 跳过的块逐字节保留。
- R31 含注释的语句块不参与结构判定；token 规则仍运行并逐字节保留注释，结构版式保持原样。
- R32 范围：所有 sqlparser 能解析的语句。DQL、DML、DDL 有结构版式；其余语句只做 token 规范化。宿主语言不处理。
