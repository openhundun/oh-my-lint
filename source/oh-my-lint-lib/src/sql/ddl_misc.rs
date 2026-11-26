//! 其余 DDL 版式：`CREATE INDEX`（D6）、`ALTER TABLE`（D7）、`CREATE TABLE AS`（D8）。

use crate::sql::decision;
use crate::sql::lexer::{Kind, Token, tokens};
use crate::sql::parse::Dialect;

/// `ddl_misc` 负责的三类语句。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LayoutKind {
    CreateIndex,
    AlterTable,
    CreateTableAs,
}

/// 关键字后括号保留一个空格（R39 的关键字一侧）。
const KEYWORD_PARENS: [&str; 31] = [
    "and",
    "as",
    "attach",
    "between",
    "by",
    "check",
    "constraint",
    "default",
    "exclude",
    "for",
    "foreign",
    "from",
    "identity",
    "in",
    "include",
    "not",
    "on",
    "opclass",
    "or",
    "partition",
    "primary",
    "references",
    "reset",
    "select",
    "set",
    "unique",
    "using",
    "values",
    "where",
    "with",
    "without",
];

/// 关键字后跟限定名再跟括号时仍需留空格的引导词。
const QUALIFIED_KEYWORDS: [&str; 5] = ["by", "on", "references", "table", "using"];

/// 渲染 `CREATE INDEX`、`ALTER TABLE`、CTAS；无法渲染时返回 `None`。
pub(crate) fn layout(sql: &str, dialect: Dialect) -> Option<String> {
    if crate::sql::lexer::has_comment(sql) {
        return None;
    }
    let normalized = crate::sql::token::normalize(sql);
    let statement = normalized.trim();
    let statement = statement.strip_suffix(';').unwrap_or(statement).trim_end();
    if statement.is_empty() {
        return None;
    }
    let output = match detect_kind(statement)? {
        LayoutKind::CreateIndex => render_index(statement),
        LayoutKind::AlterTable => render_alter(statement)?,
        LayoutKind::CreateTableAs => render_ctas(statement, dialect)?,
    };
    let output = output.trim().to_owned();
    if output.is_empty() { None } else { Some(output) }
}

/// 只看语句开头：`create [unique|fulltext|spatial] index`、`alter table`、
/// 带顶层 `as` 的 `create ... table`；其余返回 `None`。
fn detect_kind(statement: &str) -> Option<LayoutKind> {
    let all = tokens(statement);
    let flat: Vec<Token> = all.into_iter().filter(|token| token.kind != Kind::Whitespace).collect();
    if word_is(flat.first(), "create") {
        let mut index = 1;
        if word_is(flat.get(index), "unique") || word_is(flat.get(index), "fulltext") || word_is(flat.get(index), "spatial") {
            index += 1;
        }
        if word_is(flat.get(index), "index") {
            return Some(LayoutKind::CreateIndex);
        }
        return depth_zero_word(&flat, "as").map(|_| LayoutKind::CreateTableAs);
    }
    if word_is(flat.first(), "alter") && word_is(flat.get(1), "table") {
        return Some(LayoutKind::AlterTable);
    }
    None
}

/// D6：整条语句压成一行，关键字括号与访问方法括号补空格。
fn render_index(statement: &str) -> String {
    space_parens(&inline(statement))
}

/// D7：`alter table <name>` 与顶层 action 分离；1–2 个 action 同行，3 个以上逐行。
fn render_alter(statement: &str) -> Option<String> {
    let all = tokens(statement);
    let flat: Vec<Token> = all.into_iter().filter(|token| token.kind != Kind::Whitespace).collect();
    let header_end = alter_header_end(&flat)?;
    let header = space_parens(&inline(statement[..header_end].trim()));
    let rest = statement[header_end..].trim();
    if rest.is_empty() {
        return Some(header);
    }
    let actions: Vec<String> = crate::sql::splitter::split_elements("alter", rest).into_iter().map(|element| element.text).collect();
    if actions.is_empty() {
        return None;
    }
    let multiline = actions.len() >= 3;
    let base = if multiline { 4 } else { 0 };
    let formatted: Vec<String> = actions.iter().map(|action| format_action(action, base)).collect();
    let mut output = header;
    if multiline {
        for (index, action) in formatted.iter().enumerate() {
            output.push('\n');
            output.push_str("    ");
            output.push_str(action);
            if index + 1 < formatted.len() {
                output.push(',');
            }
        }
    } else {
        output.push(' ');
        output.push_str(&formatted.join(", "));
    }
    Some(output)
}

/// `alter table` 头部结束字节：可选 `if exists` / `only`、限定名与继承标记 `*`。
fn alter_header_end(tokens: &[Token]) -> Option<usize> {
    if !word_is(tokens.first(), "alter") || !word_is(tokens.get(1), "table") {
        return None;
    }
    let mut index = 2;
    if word_is(tokens.get(index), "if") && word_is(tokens.get(index + 1), "exists") {
        index += 2;
    }
    if word_is(tokens.get(index), "only") {
        index += 1;
    }
    let name = tokens.get(index)?;
    if !matches!(name.kind, Kind::Word | Kind::QuotedIdent) {
        return None;
    }
    index += 1;
    while let Some(dot) = tokens.get(index) {
        let Some(next) = tokens.get(index + 1) else {
            break;
        };
        if dot.kind == Kind::Punct && dot.text == "." && matches!(next.kind, Kind::Word | Kind::QuotedIdent) {
            index += 2;
            continue;
        }
        break;
    }
    if tokens.get(index).is_some_and(|token| token.kind == Kind::Punct && token.text == "*") {
        index += 1;
    }
    tokens.get(index.checked_sub(1)?).map(|token| token.start + token.text.len())
}

/// 一个顶层 action：`exclude` 列表按 D4 断行，其余保持单行。
fn format_action(action: &str, base: usize) -> String {
    format_exclude(action, base).unwrap_or_else(|| space_parens(&inline(action)))
}

/// `add [constraint <name>] exclude ...`：元素 ≥4 或含复杂表达式时按 D4 断行。
fn format_exclude(action: &str, base: usize) -> Option<String> {
    let flat_text = inline(action);
    let all = tokens(&flat_text);
    let flat: Vec<Token> = all.into_iter().filter(|token| token.kind != Kind::Whitespace).collect();
    if !word_is(flat.first(), "add") {
        return None;
    }
    let exclude = depth_zero_word(&flat, "exclude")?;
    let exclude_token = flat.get(exclude)?;
    let after_exclude = exclude + 1;
    let mut index = after_exclude;
    let has_method = word_is(flat.get(index), "using");
    if has_method {
        index += 1;
        let mut words = 0;
        while let Some(token) = flat.get(index) {
            if matches!(token.kind, Kind::Word | Kind::QuotedIdent) {
                words += 1;
                index += 1;
            } else if token.kind == Kind::Punct && token.text == "." {
                index += 1;
            } else {
                break;
            }
        }
        if words == 0 {
            return None;
        }
    }
    let open = flat.get(index)?;
    if open.kind != Kind::Punct || open.text != "(" {
        return None;
    }
    let close = matching_paren(&flat, index)?;
    let close_token = flat.get(close)?;
    let inner = &flat_text[open.start + 1..close_token.start];
    let elements: Vec<String> = crate::sql::splitter::split_elements("exclude", inner)
        .into_iter()
        .map(|element| space_parens(&inline(&element.text)))
        .collect();
    if elements.is_empty() {
        return None;
    }
    let complex = elements.len() >= 4 || elements.iter().any(|element| decision::is_complex(element, "exclude"));
    if !complex {
        return None;
    }
    let before = flat_text[..exclude_token.start].trim_end();
    let method = flat_text[exclude_token.start + exclude_token.text.len()..open.start].trim();
    let mut lines: Vec<String> = Vec::new();
    let (element_indent, close_indent) = if method.is_empty() {
        lines.push(format!("{before} exclude ("));
        (base + 4, base)
    } else {
        lines.push(format!("{before} exclude"));
        lines.push(format!("{}{} (", indent(base + 4), method));
        (base + 8, base + 4)
    };
    for (index, element) in elements.iter().enumerate() {
        let comma = if index + 1 < elements.len() { "," } else { "" };
        lines.push(format!("{}{element}{comma}", indent(element_indent)));
    }
    let tail_start = close_token.start + close_token.text.len();
    let (tail_head, segments) = exclude_tail(&flat_text[tail_start..]);
    let mut close_line = format!("{})", indent(close_indent));
    if !tail_head.is_empty() {
        close_line.push(' ');
        close_line.push_str(&tail_head);
    }
    lines.push(close_line);
    for segment in segments {
        lines.push(format!("{}{}", indent(base + 4), space_parens(&inline(&segment))));
    }
    Some(lines.join("\n"))
}

/// `exclude (...)` 之后的 `no inherit` / `include` / `with` / `using index` / `where` 分段。
fn exclude_tail(tail: &str) -> (String, Vec<String>) {
    let all = tokens(tail);
    let flat: Vec<Token> = all.into_iter().filter(|token| token.kind != Kind::Whitespace).collect();
    let mut marks: Vec<usize> = Vec::new();
    let mut depth: i32 = 0;
    for token in &flat {
        if token.kind == Kind::Punct && token.text == "(" {
            depth += 1;
            continue;
        }
        if token.kind == Kind::Punct && token.text == ")" {
            depth -= 1;
            continue;
        }
        if depth != 0 || token.kind != Kind::Word {
            continue;
        }
        let lower = token.text.to_ascii_lowercase();
        if matches!(lower.as_str(), "include" | "using" | "where" | "with") {
            marks.push(token.start);
        }
    }
    let Some(first) = marks.first().copied() else {
        return (tail.trim().to_owned(), Vec::new());
    };
    let head = tail[..first].trim().to_owned();
    let mut segments = Vec::with_capacity(marks.len());
    for (index, start) in marks.iter().enumerate() {
        let end = marks.get(index + 1).copied().unwrap_or(tail.len());
        segments.push(tail[*start..end].trim().to_owned());
    }
    (head, segments)
}

/// D8：header 单行；简单或非 `select` 查询体同行，复杂查询体 `as (` 换行、+4、`)` 回 0。
fn render_ctas(statement: &str, dialect: Dialect) -> Option<String> {
    let all = tokens(statement);
    let flat: Vec<Token> = all.into_iter().filter(|token| token.kind != Kind::Whitespace).collect();
    if !word_is(flat.first(), "create") {
        return None;
    }
    let as_index = depth_zero_word(&flat, "as")?;
    if !create_table_before(&flat, as_index) {
        return None;
    }
    let as_token = flat.get(as_index)?;
    let header = space_parens(&inline(statement[..as_token.start].trim_end()));
    let rest = statement[as_token.start + as_token.text.len()..].trim();
    if rest.is_empty() {
        return None;
    }
    let (inner, suffix) = ctas_body(rest)?;
    let inner = inner.as_str();
    let simple = first_word(inner).is_none_or(|word| !matches!(word.to_ascii_lowercase().as_str(), "select" | "with")) || decision::is_simple(inner);
    let mut output = format!("{header} as (");
    if simple {
        output.push_str(&space_parens(&inline(inner)));
        output.push(')');
    } else {
        let dedented = dedent_body(inner);
        let normalized = crate::sql::token::normalize(&dedented);
        let formatted = crate::sql::fix::format_with(&normalized, dialect).ok()?;
        let formatted = formatted.trim();
        if formatted.is_empty() {
            return None;
        }
        output.push('\n');
        for (index, line) in formatted.lines().enumerate() {
            if index > 0 {
                output.push('\n');
            }
            if !line.is_empty() {
                output.push_str("    ");
            }
            output.push_str(line);
        }
        output.push_str("\n)");
    }
    if !suffix.is_empty() {
        output.push(' ');
        output.push_str(&suffix);
    }
    Some(output)
}

/// 去掉查询体各行共有的缩进，再交给 DQL 修复器重新排版。
fn dedent_body(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut start = 0;
    while start < lines.len() && lines[start].trim().is_empty() {
        start += 1;
    }
    let mut end = lines.len();
    while end > start && lines[end - 1].trim().is_empty() {
        end -= 1;
    }
    if start >= end {
        return text.trim().to_owned();
    }
    let min = lines[start..end]
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start_matches(' ').len())
        .min()
        .unwrap_or(0);
    let mut output = String::new();
    for (index, line) in lines[start..end].iter().enumerate() {
        if index > 0 {
            output.push('\n');
        }
        if line.trim().is_empty() {
            continue;
        }
        output.push_str(line.get(min..).unwrap_or_else(|| line.trim_start_matches(' ')));
    }
    output
}

/// 拆出查询体与尾部；查询体统一去括号重排，默认 `with data` 省略。
fn ctas_body(rest: &str) -> Option<(String, String)> {
    if rest.starts_with('(') {
        let all = tokens(rest);
        let flat: Vec<Token> = all.into_iter().filter(|token| token.kind != Kind::Whitespace).collect();
        let close = matching_paren(&flat, 0)?;
        let close_token = flat.get(close)?;
        let inner = rest[1..close_token.start].to_owned();
        let suffix = rest[close_token.start + close_token.text.len()..].trim();
        let suffix = ctas_suffix(suffix)?;
        Some((inner, suffix))
    } else {
        let (inner, suffix) = split_trailing_data(rest);
        let suffix = ctas_suffix(&suffix)?;
        Some((inner, suffix))
    }
}

/// 把尾部规范成空或 `with no data`；无法识别的尾部视为不可渲染。
fn ctas_suffix(suffix: &str) -> Option<String> {
    let suffix = space_parens(&inline(suffix));
    if suffix.is_empty() {
        return Some(String::new());
    }
    if suffix.eq_ignore_ascii_case("with no data") {
        return Some("with no data".to_owned());
    }
    if suffix.eq_ignore_ascii_case("with data") {
        return Some(String::new());
    }
    None
}

/// 无括号查询体末尾的 `with [no] data` 单独拆出。
fn split_trailing_data(rest: &str) -> (String, String) {
    let all = tokens(rest);
    let flat: Vec<Token> = all.into_iter().filter(|token| token.kind != Kind::Whitespace).collect();
    let len = flat.len();
    if len >= 3 && word_is(flat.get(len - 1), "data") && word_is(flat.get(len - 2), "no") && word_is(flat.get(len - 3), "with") {
        let start = flat[len - 3].start;
        return (rest[..start].trim_end().to_owned(), rest[start..].trim().to_owned());
    }
    if len >= 2 && word_is(flat.get(len - 1), "data") && word_is(flat.get(len - 2), "with") {
        let start = flat[len - 2].start;
        return (rest[..start].trim_end().to_owned(), rest[start..].trim().to_owned());
    }
    (rest.trim_end().to_owned(), String::new())
}

/// `create` 与顶层 `as` 之间必须出现 `table`。
fn create_table_before(tokens: &[Token], as_index: usize) -> bool {
    let mut depth: i32 = 0;
    for token in &tokens[..as_index] {
        if token.kind == Kind::Punct && token.text == "(" {
            depth += 1;
            continue;
        }
        if token.kind == Kind::Punct && token.text == ")" {
            depth -= 1;
            continue;
        }
        if depth == 0 && token.kind == Kind::Word && token.text.eq_ignore_ascii_case("table") {
            return true;
        }
    }
    false
}

fn first_word(text: &str) -> Option<String> {
    tokens(text).into_iter().find(|token| token.kind == Kind::Word).map(|token| token.text.to_owned())
}

/// 把任意空白压成单行；是否存在空白与是否保留空格分开决策。
fn inline(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut seen: Vec<Token> = Vec::new();
    let mut gap = false;
    for token in tokens(text) {
        if token.kind == Kind::Whitespace {
            if !seen.is_empty() {
                gap = true;
            }
            continue;
        }
        if gap && keep_space(&seen, token) {
            output.push(' ');
        }
        gap = false;
        output.push_str(token.text);
        seen.push(token);
    }
    output
}

/// 跨行空白处是否需要保留一个空格。
fn keep_space(seen: &[Token], next: Token) -> bool {
    let Some(previous) = seen.last() else {
        return false;
    };
    if next.kind == Kind::Punct {
        return match next.text {
            ")" | "]" | "," | ";" | "." | "::" | "[" => false,
            "(" => {
                if previous.kind == Kind::Word {
                    space_before_paren(seen)
                } else {
                    previous.kind == Kind::Punct && previous.text != ")"
                }
            }
            _ => true,
        };
    }
    if previous.kind == Kind::Punct {
        return !matches!(previous.text, "(" | "[" | "." | "::");
    }
    true
}

/// R39：结构性关键字与访问方法/表名后的括号补一个空格；函数调用保持紧贴。
fn space_parens(text: &str) -> String {
    let mut output = String::with_capacity(text.len() + 8);
    let mut seen: Vec<Token> = Vec::new();
    for token in tokens(text) {
        if token.kind == Kind::Whitespace {
            output.push_str(token.text);
            continue;
        }
        if token.kind == Kind::Punct && token.text == "(" && space_before_paren(&seen) && !output.ends_with(' ') {
            output.push(' ');
        }
        output.push_str(token.text);
        seen.push(token);
    }
    output
}

/// 括号前是否是关键字、`primary key`/`foreign key`、`set null` 或限定名引导词。
fn space_before_paren(seen: &[Token]) -> bool {
    let Some(previous) = seen.last() else {
        return false;
    };
    if previous.kind != Kind::Word {
        return false;
    }
    let lower = previous.text.to_ascii_lowercase();
    if KEYWORD_PARENS.contains(&lower.as_str()) {
        return true;
    }
    let before = seen.len().checked_sub(2).and_then(|index| seen.get(index));
    if lower == "key"
        && let Some(before) = before
        && before.kind == Kind::Word
        && matches!(before.text.to_ascii_lowercase().as_str(), "primary" | "foreign")
    {
        return true;
    }
    if lower == "null"
        && let Some(before) = before
        && before.kind == Kind::Word
        && before.text.eq_ignore_ascii_case("set")
    {
        return true;
    }
    // 表达式后的 opclass 名（`myfunc(a, b) fop (opt=1)`）参数列表留空格。
    if let Some(before) = before
        && before.kind == Kind::Punct
        && before.text == ")"
    {
        return true;
    }
    if let Some(before) = word_before_qualified(seen)
        && before.kind == Kind::Word
    {
        let lower = before.text.to_ascii_lowercase();
        return QUALIFIED_KEYWORDS.contains(&lower.as_str());
    }
    false
}

/// `seen` 末尾的限定名（如 `schema.table`）之前的词。
fn word_before_qualified<'a>(seen: &'a [Token<'a>]) -> Option<&'a Token<'a>> {
    let mut index = seen.len().checked_sub(1)?;
    while index >= 2 {
        let dot = &seen[index - 1];
        if dot.kind != Kind::Punct || dot.text != "." {
            break;
        }
        index -= 2;
    }
    seen.get(index.checked_sub(1)?)
}

fn depth_zero_word(tokens: &[Token], word: &str) -> Option<usize> {
    let mut depth: i32 = 0;
    for (index, token) in tokens.iter().enumerate() {
        if token.kind == Kind::Punct && token.text == "(" {
            depth += 1;
            continue;
        }
        if token.kind == Kind::Punct && token.text == ")" {
            depth -= 1;
            continue;
        }
        if depth == 0 && token.kind == Kind::Word && token.text.eq_ignore_ascii_case(word) {
            return Some(index);
        }
    }
    None
}

fn matching_paren(tokens: &[Token], open: usize) -> Option<usize> {
    let mut depth: i32 = 0;
    for (index, token) in tokens.iter().enumerate().skip(open) {
        if token.kind == Kind::Punct && token.text == "(" {
            depth += 1;
        } else if token.kind == Kind::Punct && token.text == ")" {
            depth -= 1;
            if depth == 0 {
                return Some(index);
            }
        }
    }
    None
}

fn word_is(token: Option<&Token>, word: &str) -> bool {
    token.is_some_and(|token| token.kind == Kind::Word && token.text.eq_ignore_ascii_case(word))
}

fn indent(width: usize) -> String {
    " ".repeat(width)
}

#[cfg(test)]
mod tests {
    use crate::sql::parse::Dialect;

    fn layout(sql: &str) -> String {
        super::layout(sql, Dialect::Postgres).expect("layout")
    }

    fn assert_idempotent(output: &str) {
        assert_eq!(super::layout(output, Dialect::Postgres).as_deref(), Some(output));
    }

    #[test]
    fn create_index_is_one_line() {
        let sql = "create unique index title_idx\n    on films using btree (title, code)\n    include (director, rating)\n    with (fillfactor=70)\n    where title != ''";
        let expected = "create unique index title_idx on films using btree (title, code) include (director, rating) with (fillfactor=70) where title != ''";
        assert_eq!(layout(sql), expected);
        assert_idempotent(expected);
    }

    #[test]
    fn create_index_hugs_expression_parens() {
        let sql = "create index idx_expr on t using btree ((col::my_type(10)))";
        assert_eq!(layout(sql), sql);
        assert_idempotent(sql);
    }

    #[test]
    fn create_index_adds_access_method_space() {
        let sql = "create index pointloc on points using gist(box(location, location))";
        assert_eq!(layout(sql), "create index pointloc on points using gist (box(location, location))");
    }

    #[test]
    fn create_index_keeps_tablespace_inline() {
        let sql = "create index code_idx on films (code) tablespace indexspace";
        assert_eq!(layout(sql), sql);
        assert_idempotent(sql);
    }

    #[test]
    fn alter_one_or_two_actions_stay_inline() {
        let one = "alter table distributors add column address varchar(30)";
        assert_eq!(layout(one), one);
        assert_idempotent(one);
        let sql = "alter table distributors\n    add column address varchar(30),\n    alter column address set default 'old'";
        let expected = "alter table distributors add column address varchar(30), alter column address set default 'old'";
        assert_eq!(layout(sql), expected);
        assert_idempotent(expected);
    }

    #[test]
    fn alter_three_actions_break() {
        let sql = "alter table foo alter column foo_timestamp drop default, alter column foo_timestamp type timestamptz using 'epoch'::timestamp + foo_timestamp * '1 second'::interval, alter column foo_timestamp set default now()";
        let expected = "alter table foo\n    alter column foo_timestamp drop default,\n    alter column foo_timestamp type timestamptz using 'epoch'::timestamp + foo_timestamp * '1 second'::interval,\n    alter column foo_timestamp set default now()";
        assert_eq!(layout(sql), expected);
        assert_idempotent(expected);
    }

    #[test]
    fn alter_field_level_primary_key_is_not_split() {
        let sql = "alter table tbl add column id int not null primary key";
        let output = layout(sql);
        assert_eq!(output, sql);
        assert!(!output.contains('\n'), "{output}");
        assert_idempotent(&output);

        // sqlparser 0.64 解析不了的列级主键 + index 选项：版式层仍不得拆成多个 action。
        let sql = "alter table tbl add pk text default 'hello' primary key with (arg1=3) using index tablespace tblspace not null";
        let output = layout(sql);
        assert!(!output.contains('\n'), "{output}");
        assert!(!output.contains(", "), "{output}");
        assert!(output.ends_with("primary key with (arg1=3) using index tablespace tblspace not null"));
    }

    #[test]
    fn alter_keeps_set_storage_default() {
        let sql = "alter table mytable alter mycolumn set storage default";
        assert_eq!(layout(sql), sql);
        assert_idempotent(sql);
    }

    #[test]
    fn alter_exclude_list_breaks_per_d4() {
        let sql = "alter table reservation add exclude using gist (a with =, b with =, c with =, d with =)";
        let expected = "alter table reservation add exclude\n    using gist (\n        a with =,\n        b with =,\n        c with =,\n        d with =\n    )";
        assert_eq!(layout(sql), expected);
        assert_idempotent(expected);
    }

    #[test]
    fn ctas_simple_body_stays_inline() {
        let sql = "create table t1 as (select something from t2)";
        assert_eq!(layout(sql), sql);
        assert_idempotent(sql);
    }

    #[test]
    fn ctas_unparenthesized_select_gets_parentheses() {
        let sql = "create table t1 as select something from t2";
        assert_eq!(layout(sql), "create table t1 as (select something from t2)");
    }

    #[test]
    fn ctas_keeps_using_method_header() {
        let sql = "create table t1 using method as (select something from t2)";
        assert_eq!(layout(sql), sql);
        assert_idempotent(sql);
    }

    #[test]
    fn ctas_keeps_with_no_data() {
        let sql = "create table t1 as (select something from t2) with no data";
        assert_eq!(layout(sql), sql);
        assert_idempotent(sql);
        let unparenthesized = "create table t1 as select something from t2 with no data";
        assert_eq!(layout(unparenthesized), "create table t1 as (select something from t2) with no data");
    }

    #[test]
    fn ctas_omits_default_with_data() {
        let sql = "create table t1 as (select something from t2) with data";
        assert_eq!(layout(sql), "create table t1 as (select something from t2)");
    }

    #[test]
    fn ctas_complex_body_breaks_with_header_properties() {
        let sql = "create temp table a_new_table with (appendoptimized=true, compresstype=zstd) as (select column_1, column_2, column_3 from schema.tablename group by 1, 2, 3)";
        let expected = "create temp table a_new_table with (appendoptimized=true, compresstype=zstd) as (\n    select column_1, column_2, column_3\n    from schema.tablename\n    group by 1, 2, 3\n)";
        assert_eq!(layout(sql), expected);
        assert_idempotent(expected);
    }

    #[test]
    fn non_ctas_and_other_statements_are_not_handled() {
        assert_eq!(super::layout("create table t (a int)", Dialect::Postgres), None);
        assert_eq!(super::layout("insert into t values (1)", Dialect::Postgres), None);
    }
}
