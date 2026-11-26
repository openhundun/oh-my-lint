//! `CREATE TABLE` 版式（D1–D5）；CTAS 交给 `ddl_misc`。

use crate::sql::lexer::{Kind, Token, tokens};
use crate::sql::parse::Dialect;
use crate::sql::token;

/// 渲染普通 `CREATE TABLE`；CTAS、无法渲染或输出不合法时返回 `None`。
pub(crate) fn layout(sql: &str, dialect: Dialect) -> Option<String> {
    let _ = dialect;
    let normalized = token::normalize(sql);
    let text = normalized.trim();
    if text.is_empty() || crate::sql::lexer::has_comment(text) {
        return None;
    }
    let toks = significant(text);
    let parsed = parse_statement(text, &toks)?;
    let Some(body) = parsed.body else {
        return Some(parsed.inline_header);
    };
    let body = parse_body(body, parsed.typed)?;
    let properties = parse_properties(parsed.tail)?;
    render(parsed, body, properties)
}

/// 一条语句的头部与尾部切片。
struct Parsed<'a> {
    /// `create table <name>`（typed table 含 `of <type>`）。
    base_header: String,
    /// 第一个 `(` 之前的完整头部，含 body 前的表属性和类型。
    inline_header: String,
    /// body 之前的表属性（`partition of ...`）。
    pre_property: Option<String>,
    /// `create table ... of <type>` 的 typed table。
    typed: bool,
    /// 括号内的主体；没有括号时为 `None`。
    body: Option<&'a str>,
    /// 闭合 `)` 之后的表属性。
    tail: &'a str,
}

/// 表主体的一个元素。
enum Element {
    Column(Box<Column>),
    Constraint(String),
}

/// 解析后的主体：按原顺序排列，列级主键单独提升。
struct Body {
    elements: Vec<Element>,
    promoted: Vec<String>,
}

#[derive(Default)]
struct Column {
    name: String,
    ty: String,
    typed_prefix: Option<String>,
    storage: Option<String>,
    compression: Option<String>,
    collate: Option<String>,
    not_null: Option<String>,
    default: Option<String>,
    generated: Option<String>,
    unique: Option<String>,
    check: Option<String>,
    references: Option<String>,
    pk: Option<(Option<String>, String)>,
    named: bool,
}

impl Column {
    /// D2 的其余约束，按固定顺序拼接。
    fn rest(&self) -> String {
        let mut parts = Vec::new();
        for value in [
            &self.default,
            &self.generated,
            &self.unique,
            &self.check,
            &self.references,
        ]
        .into_iter()
        .flatten()
        {
            parts.push(value.as_str());
        }
        parts.join(" ")
    }

    /// D1：除 `not null` 之外的额外修饰。
    fn extra(&self) -> bool {
        self.typed_prefix.is_some()
            || self.storage.is_some()
            || self.compression.is_some()
            || self.collate.is_some()
            || self.default.is_some()
            || self.generated.is_some()
            || self.unique.is_some()
            || self.check.is_some()
            || self.references.is_some()
            || self.named
    }

    /// 内联列文本；不补对齐空格。
    fn inline_text(&self) -> String {
        let mut parts = vec![self.name.clone()];
        if let Some(prefix) = &self.typed_prefix {
            parts.push(prefix.clone());
        }
        if !self.ty.is_empty() {
            parts.push(self.ty.clone());
        }
        for value in [
            &self.storage,
            &self.compression,
            &self.collate,
            &self.not_null,
        ]
        .into_iter()
        .flatten()
        {
            parts.push(value.clone());
        }
        let rest = self.rest();
        if !rest.is_empty() {
            parts.push(rest);
        }
        parts.join(" ")
    }

    /// D3 的七个对齐单元格：name、type、storage、compression、collate、not null、rest。
    fn cells(&self) -> [String; 7] {
        [
            self.name.clone(),
            self.ty.clone(),
            self.storage.clone().unwrap_or_default(),
            self.compression.clone().unwrap_or_default(),
            self.collate.clone().unwrap_or_default(),
            self.not_null.clone().unwrap_or_default(),
            self.rest(),
        ]
    }
}

fn render(parsed: Parsed<'_>, body: Body, tail_properties: Vec<String>) -> Option<String> {
    let Body { elements, promoted } = body;
    let broken = body_breaks(&elements, &promoted);
    let mut all_properties = Vec::new();
    if let Some(pre) = &parsed.pre_property {
        all_properties.push(pre.clone());
    }
    all_properties.extend(tail_properties.iter().cloned());

    if !broken {
        let inline_body = inline_body(&elements);
        if properties_inline(&all_properties) {
            let mut output = format!("{} ({})", parsed.inline_header, inline_body);
            for property in &tail_properties {
                output.push(' ');
                output.push_str(property);
            }
            return Some(output);
        }

        let mut lines = Vec::new();
        if let Some(pre) = &parsed.pre_property {
            lines.push(parsed.base_header.clone());
            lines.push(format!("{pre} ({inline_body})"));
        } else {
            lines.push(format!("{} ({inline_body})", parsed.base_header));
        }
        lines.extend(tail_properties);
        return Some(lines.join("\n"));
    }

    let mut lines = Vec::new();
    if let Some(pre) = &parsed.pre_property {
        lines.push(parsed.base_header.clone());
        lines.push(format!("{pre} ("));
    } else {
        lines.push(format!("{} (", parsed.base_header));
    }
    lines.extend(broken_body(&elements, &promoted));
    lines.push(")".to_owned());
    lines.extend(tail_properties);
    Some(lines.join("\n"))
}

fn body_breaks(elements: &[Element], promoted: &[String]) -> bool {
    if !promoted.is_empty()
        || elements
            .iter()
            .any(|element| matches!(element, Element::Constraint(_)))
    {
        return true;
    }
    let columns: Vec<&Column> = elements
        .iter()
        .filter_map(|element| match element {
            Element::Column(column) => Some(column.as_ref()),
            Element::Constraint(_) => None,
        })
        .collect();
    if columns.len() <= 1 {
        return false;
    }
    if columns.iter().any(|column| column.extra()) {
        return columns.len() > 2;
    }
    columns.len() > 3
}

fn inline_body(elements: &[Element]) -> String {
    elements
        .iter()
        .filter_map(|element| match element {
            Element::Column(column) => Some(column.inline_text()),
            Element::Constraint(_) => None,
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn broken_body(elements: &[Element], promoted: &[String]) -> Vec<String> {
    let columns: Vec<&Column> = elements
        .iter()
        .filter_map(|element| match element {
            Element::Column(column) => Some(column.as_ref()),
            Element::Constraint(_) => None,
        })
        .collect();
    let alignment = Alignment::new(&columns);
    let mut groups: Vec<Vec<String>> = Vec::new();
    for element in elements {
        match element {
            Element::Column(column) => groups.push(vec![alignment.line(column)]),
            Element::Constraint(text) => groups.push(constraint_lines(text)),
        }
    }
    for line in promoted {
        groups.push(vec![line.clone()]);
    }
    let last_group = groups.len().saturating_sub(1);
    for (index, group) in groups.iter_mut().enumerate() {
        if index < last_group
            && let Some(last) = group.last_mut()
        {
            last.push(',');
        }
    }
    let mut lines = Vec::new();
    for group in groups {
        for line in group {
            lines.push(format!("    {line}"));
        }
    }
    lines
}

/// D3：name、type、storage、compression、collate、not null 的宽度。
///
/// 只有"该字段后面还有非空字段"的行才参与宽度计算；因此行尾字段不会把
/// 前面的列撑宽。空单元只有在同列有宽度时才占位对齐。
struct Alignment {
    widths: [usize; 7],
    active: [bool; 7],
}

impl Alignment {
    fn new(columns: &[&Column]) -> Self {
        let mut widths = [0usize; 7];
        for column in columns {
            let cells = column.cells();
            let last = last_non_empty(&cells);
            for (index, cell) in cells.iter().enumerate().take(last) {
                widths[index] = widths[index].max(cell.chars().count());
            }
        }
        let active = std::array::from_fn(|index| widths[index] > 0);
        Self { widths, active }
    }

    fn line(&self, column: &Column) -> String {
        // typed table 的列没有类型列，按 D2 的固定单空格写法渲染。
        if column.typed_prefix.is_some() {
            return column.inline_text();
        }
        let cells = column.cells();
        let last = last_non_empty(&cells);
        let indices: Vec<usize> = (0..=last)
            .filter(|index| self.active[*index] || !cells[*index].is_empty())
            .collect();
        let mut output = String::new();
        for (position, index) in indices.iter().enumerate() {
            let cell = &cells[*index];
            output.push_str(cell);
            if position + 1 < indices.len() {
                let padding = self.widths[*index].saturating_sub(cell.chars().count()) + 1;
                for _ in 0..padding {
                    output.push(' ');
                }
            }
        }
        output
    }
}

/// 行内最后一个非空字段的下标。
fn last_non_empty(cells: &[String; 7]) -> usize {
    cells.iter().rposition(|cell| !cell.is_empty()).unwrap_or(0)
}

fn constraint_lines(text: &str) -> Vec<String> {
    if !is_exclude(text) {
        return vec![text.to_owned()];
    }
    let Some(lines) = exclude_lines(text) else {
        return vec![text.to_owned()];
    };
    lines
}

fn is_exclude(text: &str) -> bool {
    let toks = significant(text);
    let mut index = 0;
    if word_at(&toks, index, "constraint") {
        index += 2;
    }
    word_at(&toks, index, "exclude")
}

fn exclude_lines(text: &str) -> Option<Vec<String>> {
    let toks = significant(text);
    if toks.is_empty() {
        return None;
    }
    let depth = depths(&toks);
    let mut open = None;
    for (index, token) in toks.iter().enumerate() {
        if depth[index] == 0 && punct(token, "(") {
            open = Some(index);
            break;
        }
    }
    let open = open?;
    let close = matching_paren(&toks, open)?;
    let inner = &text[token_end(&toks[open])..toks[close].start];
    let elements = split_top_level(inner);
    if elements.is_empty() {
        return None;
    }
    let simple = elements.len() <= 3
        && elements
            .iter()
            .all(|element| exclude_element_simple(element));
    if simple {
        return Some(vec![text.to_owned()]);
    }

    let mut lines = Vec::new();
    lines.push(collapse(&text[..token_end(&toks[open])]));
    for (index, element) in elements.iter().enumerate() {
        let mut line = format!("    {}", element.trim());
        if index + 1 < elements.len() {
            line.push(',');
        }
        lines.push(line);
    }
    lines.push(")".to_owned());
    let rest = &text[token_end(&toks[close])..];
    lines.extend(exclude_options(rest)?);
    Some(lines)
}

fn exclude_element_simple(element: &str) -> bool {
    let toks = significant(element);
    !toks.iter().any(|token| {
        if token.kind == Kind::Word {
            matches!(
                token.text.to_ascii_lowercase().as_str(),
                "select" | "case" | "and" | "or"
            )
        } else {
            token.kind == Kind::Punct && matches!(token.text, "(" | ")" | "+" | "-" | "*" | "/")
        }
    })
}

fn exclude_options(text: &str) -> Option<Vec<String>> {
    let toks = significant(text);
    let mut options = Vec::new();
    let mut index = 0;
    while index < toks.len() {
        let start = index;
        let lower = toks[index].text.to_ascii_lowercase();
        match lower.as_str() {
            "include" | "with" => {
                index += 1;
                index = consume_paren_group(&toks, index)?;
            }
            "where" => {
                index += 1;
                index = consume_paren_group(&toks, index)?;
            }
            "using" => {
                index += 1;
                while index < toks.len() && !is_exclude_option_start(&toks, index) {
                    index += 1;
                }
            }
            "deferrable" | "initially" | "valid" | "not" => {
                index = toks.len();
            }
            _ => return None,
        }
        if index <= start {
            return None;
        }
        options.push(collapse(
            &text[toks[start].start..token_end(&toks[index - 1])],
        ));
    }
    Some(options)
}

fn is_exclude_option_start(toks: &[Token<'_>], index: usize) -> bool {
    toks.get(index).is_some_and(|token| {
        token.kind == Kind::Word
            && matches!(
                token.text.to_ascii_lowercase().as_str(),
                "include"
                    | "with"
                    | "using"
                    | "where"
                    | "deferrable"
                    | "initially"
                    | "valid"
                    | "not"
            )
    })
}

fn parse_body(body: &str, typed: bool) -> Option<Body> {
    let mut elements = Vec::new();
    for part in split_top_level(body) {
        let toks = significant(part);
        if toks.is_empty() {
            continue;
        }
        if is_word(&toks[0], "like") {
            return None;
        }
        if is_table_constraint(&toks) {
            elements.push(Element::Constraint(collapse(part)));
        } else {
            elements.push(Element::Column(Box::new(parse_column(part, typed)?)));
        }
    }

    let mut primary_keys = Vec::new();
    for element in &elements {
        if let Element::Constraint(text) = element
            && let Some(names) = primary_key_columns(text)
        {
            primary_keys.extend(names);
        }
    }
    let mut promoted = Vec::new();
    for element in &elements {
        if let Element::Column(column) = element
            && let Some((prefix, suffix)) = &column.pk
        {
            primary_keys.push(column.name.clone());
            promoted.push(promoted_key(&column.name, prefix, suffix));
        }
    }
    for element in &mut elements {
        if let Element::Column(column) = element
            && primary_keys.iter().any(|name| name == &column.name)
            && column.not_null.is_none()
        {
            column.not_null = Some("not null".to_owned());
        }
    }
    Some(Body { elements, promoted })
}

fn promoted_key(name: &str, prefix: &Option<String>, suffix: &str) -> String {
    let mut output = String::new();
    if let Some(prefix) = prefix {
        output.push_str(prefix);
        output.push(' ');
    }
    output.push_str("primary key (");
    output.push_str(name);
    output.push(')');
    if !suffix.is_empty() {
        output.push(' ');
        output.push_str(suffix);
    }
    output
}

/// 表级 `primary key (...)` 的字段名。
fn primary_key_columns(text: &str) -> Option<Vec<String>> {
    let toks = significant(text);
    let mut index = 0;
    if word_at(&toks, index, "constraint") {
        index += 2;
    }
    if !(word_at(&toks, index, "primary") && word_at(&toks, index + 1, "key")) {
        return None;
    }
    index += 2;
    if !punct_at(&toks, index, "(") {
        return None;
    }
    let close = matching_paren(&toks, index)?;
    let inner = &text[token_end(&toks[index])..toks[close].start];
    let mut names = Vec::new();
    for part in split_top_level(inner) {
        if !part.is_empty() {
            names.push(collapse(part));
        }
    }
    if names.is_empty() { None } else { Some(names) }
}

fn is_table_constraint(toks: &[Token<'_>]) -> bool {
    let mut index = 0;
    if word_at(toks, index, "constraint") {
        index += 2;
    }
    let Some(token) = toks.get(index) else {
        return false;
    };
    if token.kind != Kind::Word {
        return false;
    }
    match token.text.to_ascii_lowercase().as_str() {
        "primary" => word_at(toks, index + 1, "key"),
        "foreign" => word_at(toks, index + 1, "key"),
        "unique" => punct_at(toks, index + 1, "(") || word_at(toks, index + 1, "nulls"),
        "check" => punct_at(toks, index + 1, "("),
        "exclude" => punct_at(toks, index + 1, "(") || word_at(toks, index + 1, "using"),
        _ => false,
    }
}

fn parse_column(element: &str, typed: bool) -> Option<Column> {
    let toks = significant(element);
    let name = toks.first()?;
    if !is_ident(name) {
        return None;
    }
    let depth = depths(&toks);
    let mut column = Column {
        name: name.text.to_owned(),
        ..Column::default()
    };
    let mut index = 1;

    if typed {
        if !(word_at(&toks, index, "with") && word_at(&toks, index + 1, "options")) {
            return None;
        }
        column.typed_prefix = Some("with options".to_owned());
        index += 2;
    } else {
        let start = index;
        while index < toks.len() {
            if depth[index] == 0 && is_type_boundary(&toks, index) {
                break;
            }
            index += 1;
        }
        if index == start {
            return None;
        }
        column.ty = canonical_type(slice(element, &toks, start, index));
    }

    while index < toks.len() {
        if depth[index] != 0 {
            return None;
        }
        let (prefix, at) = parse_constraint_prefix(&toks, index)?;
        if at >= toks.len() {
            return None;
        }
        let lower = toks[at].text.to_ascii_lowercase();
        match lower.as_str() {
            "not" if word_at(&toks, at + 1, "null") => {
                if column.not_null.is_some() {
                    return None;
                }
                column.not_null = Some(attach(&prefix, "not null"));
                column.named |= prefix.is_some();
                index = at + 2;
            }
            "null" => {
                // R38：显式 null 省略。
                index = at + 1;
            }
            "storage" | "compression" => {
                let value = toks.get(at + 1)?;
                if !is_ident(value) {
                    return None;
                }
                let omit = value.text.eq_ignore_ascii_case("default");
                index = at + 2;
                if !omit {
                    let body = slice(element, &toks, at, index);
                    let value = attach(&prefix, body);
                    if lower == "storage" {
                        if column.storage.is_some() {
                            return None;
                        }
                        column.storage = Some(value);
                    } else {
                        if column.compression.is_some() {
                            return None;
                        }
                        column.compression = Some(value);
                    }
                    column.named |= prefix.is_some();
                }
            }
            "collate" => {
                let mut end = at + 1;
                consume_qualified(&toks, &mut end)?;
                index = end;
                if column.collate.is_some() {
                    return None;
                }
                column.collate = Some(attach(&prefix, slice(element, &toks, at, index)));
                column.named |= prefix.is_some();
            }
            "default" => {
                let mut end = at + 1;
                if end >= toks.len() {
                    return None;
                }
                while end < toks.len() && !(end > at + 1 && is_constraint_boundary(&toks, end)) {
                    end += 1;
                }
                index = end;
                if column.default.is_some() {
                    return None;
                }
                column.default = Some(attach(&prefix, slice(element, &toks, at, index)));
                column.named |= prefix.is_some();
            }
            "generated" => {
                let start = at;
                let mut end = at + 1;
                if word_at(&toks, end, "always") {
                    end += 1;
                } else if word_at(&toks, end, "by") && word_at(&toks, end + 1, "default") {
                    end += 2;
                } else {
                    return None;
                }
                if !word_at(&toks, end, "as") {
                    return None;
                }
                end += 1;
                if word_at(&toks, end, "identity") {
                    end += 1;
                } else {
                    end = consume_paren_group(&toks, end)?;
                    if word_at(&toks, end, "stored") || word_at(&toks, end, "virtual") {
                        end += 1;
                    }
                }
                index = end;
                if column.generated.is_some() {
                    return None;
                }
                column.generated = Some(attach(&prefix, slice(element, &toks, start, index)));
                column.named |= prefix.is_some();
            }
            "unique" => {
                let start = at;
                let mut end = at + 1;
                let mut nulls_not_distinct = false;
                if word_at(&toks, end, "nulls") {
                    end += 1;
                    let not = word_at(&toks, end, "not");
                    if not {
                        end += 1;
                    }
                    if !word_at(&toks, end, "distinct") {
                        return None;
                    }
                    end += 1;
                    nulls_not_distinct = not;
                }
                let mut extras = Vec::new();
                if word_at(&toks, end, "with") {
                    let extra_start = end;
                    end += 1;
                    end = consume_paren_group(&toks, end)?;
                    extras.push(collapse(slice(element, &toks, extra_start, end)));
                }
                if word_at(&toks, end, "using") {
                    let extra_start = end;
                    end += 1;
                    if !word_at(&toks, end, "index") {
                        return None;
                    }
                    end += 1;
                    if word_at(&toks, end, "tablespace") {
                        end += 1;
                        end = consume_ident(&toks, end)?;
                    }
                    extras.push(collapse(slice(element, &toks, extra_start, end)));
                }
                index = end;
                if column.unique.is_some() {
                    return None;
                }
                let mut body = String::from("unique");
                if nulls_not_distinct {
                    body.push_str(" nulls not distinct");
                }
                for extra in extras {
                    body.push(' ');
                    body.push_str(&extra);
                }
                column.unique = Some(attach(&prefix, &body));
                column.named |= prefix.is_some();
                let _ = start;
            }
            "check" => {
                let start = at;
                let mut end = at + 1;
                end = consume_paren_group(&toks, end)?;
                if word_at(&toks, end, "no") && word_at(&toks, end + 1, "inherit") {
                    end += 2;
                }
                index = end;
                if column.check.is_some() {
                    return None;
                }
                column.check = Some(attach(&prefix, slice(element, &toks, start, index)));
                column.named |= prefix.is_some();
            }
            "references" => {
                let start = at;
                let mut end = at + 1;
                consume_qualified(&toks, &mut end)?;
                while end < toks.len() && !is_reference_boundary(&toks, end) {
                    end += 1;
                }
                index = end;
                if column.references.is_some() {
                    return None;
                }
                column.references = Some(attach(&prefix, slice(element, &toks, start, index)));
                column.named |= prefix.is_some();
            }
            "primary" => {
                if !word_at(&toks, at + 1, "key") {
                    return None;
                }
                if column.pk.is_some() {
                    return None;
                }
                let mut end = at + 2;
                while end < toks.len() && !is_constraint_boundary(&toks, end) {
                    end += 1;
                }
                let suffix = if end > at + 2 {
                    collapse(slice(element, &toks, at + 2, end))
                } else {
                    String::new()
                };
                column.pk = Some((prefix, suffix));
                index = end;
            }
            _ => return None,
        }
    }
    Some(column)
}

fn parse_constraint_prefix(toks: &[Token<'_>], index: usize) -> Option<(Option<String>, usize)> {
    if !word_at(toks, index, "constraint") {
        return Some((None, index));
    }
    let name = toks.get(index + 1)?;
    if !is_ident(name) {
        return None;
    }
    Some((Some(format!("constraint {}", name.text)), index + 2))
}

fn is_type_boundary(toks: &[Token<'_>], index: usize) -> bool {
    let Some(token) = toks.get(index) else {
        return false;
    };
    if token.kind != Kind::Word {
        return false;
    }
    match token.text.to_ascii_lowercase().as_str() {
        "storage" | "compression" | "collate" | "default" | "generated" | "unique" | "check"
        | "references" | "primary" | "constraint" | "null" => true,
        "not" => word_at(toks, index + 1, "null"),
        _ => false,
    }
}

fn is_constraint_boundary(toks: &[Token<'_>], index: usize) -> bool {
    is_type_boundary(toks, index)
}

fn is_reference_boundary(toks: &[Token<'_>], index: usize) -> bool {
    let Some(token) = toks.get(index) else {
        return false;
    };
    if token.kind != Kind::Word {
        return false;
    }
    match token.text.to_ascii_lowercase().as_str() {
        "default" => !word_at(toks, index.wrapping_sub(1), "set"),
        "storage" | "compression" | "collate" | "generated" | "unique" | "check" | "references"
        | "primary" | "constraint" => true,
        "not" => word_at(toks, index + 1, "null"),
        _ => false,
    }
}

fn attach(prefix: &Option<String>, body: &str) -> String {
    match prefix {
        Some(prefix) => format!("{prefix} {body}"),
        None => body.to_owned(),
    }
}

/// R36：类型位置的名字归一；`collate numeric` 不在类型位置，故不受影响。
fn canonical_type(text: &str) -> String {
    let toks = significant(text);
    if toks.is_empty() {
        return collapse(text);
    }
    let first = &toks[0];
    if first.kind != Kind::Word {
        return collapse(text);
    }
    if toks.get(1).is_some_and(|token| punct(token, ".")) {
        return collapse(text);
    }
    let lower = first.text.to_ascii_lowercase();

    if lower == "timestamp" || lower == "time" {
        let depth = depths(&toks);
        let mut suffix = None;
        for (index, level) in depth.iter().enumerate().skip(1) {
            if *level != 0 {
                continue;
            }
            if (word_at(&toks, index, "with") || word_at(&toks, index, "without"))
                && word_at(&toks, index + 1, "time")
                && word_at(&toks, index + 2, "zone")
            {
                suffix = Some((index, word_at(&toks, index, "with")));
                break;
            }
        }
        if let Some((with_index, with_zone)) = suffix {
            let replacement = match (lower.as_str(), with_zone) {
                ("timestamp", true) => "timestamptz",
                ("timestamp", false) => "timestamp",
                ("time", true) => "timetz",
                _ => "time",
            };
            let mut output = String::new();
            output.push_str(&text[..first.start]);
            output.push_str(replacement);
            output.push_str(&text[token_end(first)..toks[with_index].start]);
            let output = output.trim_end();
            let after = &text[token_end(&toks[with_index + 2])..];
            return collapse(&format!("{output}{after}"));
        }
        return collapse(text);
    }

    let mut replacement: Option<(&str, usize)> = None;
    match lower.as_str() {
        "double" if word_at(&toks, 1, "precision") => {
            replacement = Some(("float8", 2));
        }
        "character" | "char" if word_at(&toks, 1, "varying") => {
            replacement = Some(("varchar", 2));
        }
        "character" => replacement = Some(("char", 1)),
        "integer" | "int4" => replacement = Some(("int", 1)),
        "int2" => replacement = Some(("smallint", 1)),
        "int8" => replacement = Some(("bigint", 1)),
        "bool" => replacement = Some(("boolean", 1)),
        "numeric" => replacement = Some(("decimal", 1)),
        "real" => replacement = Some(("float4", 1)),
        _ => {}
    }
    let Some((name, count)) = replacement else {
        return collapse(text);
    };
    let mut output = String::new();
    output.push_str(&text[..first.start]);
    output.push_str(name);
    output.push_str(&text[token_end(&toks[count - 1])..]);
    collapse(&output)
}

fn parse_statement<'a>(text: &'a str, toks: &[Token<'a>]) -> Option<Parsed<'a>> {
    if toks.is_empty() || !is_word(&toks[0], "create") {
        return None;
    }
    let depth = depths(toks);
    // CTAS 的顶层 `as` 不归本模块。
    if toks
        .iter()
        .enumerate()
        .any(|(index, token)| depth[index] == 0 && is_word(token, "as"))
    {
        return None;
    }

    let mut index = 1;
    let mut found_table = false;
    while index < toks.len() {
        if is_word(&toks[index], "table") {
            found_table = true;
            index += 1;
            break;
        }
        let lower = toks[index].text.to_ascii_lowercase();
        if matches!(
            lower.as_str(),
            "global" | "local" | "temporary" | "temp" | "unlogged" | "or" | "replace"
        ) {
            index += 1;
            continue;
        }
        return None;
    }
    if !found_table {
        return None;
    }
    if word_at(toks, index, "if")
        && word_at(toks, index + 1, "not")
        && word_at(toks, index + 2, "exists")
    {
        index += 3;
    }
    consume_qualified(toks, &mut index)?;
    let mut base_end = token_end(&toks[index - 1]);
    let mut typed = false;
    let mut pre_property = None;

    if word_at(toks, index, "of") {
        typed = true;
        index += 1;
        consume_qualified(toks, &mut index)?;
        base_end = token_end(&toks[index - 1]);
    } else if word_at(toks, index, "partition") && word_at(toks, index + 1, "of") {
        let start = toks[index].start;
        index += 2;
        consume_qualified(toks, &mut index)?;
        let end = token_end(&toks[index - 1]);
        pre_property = Some(collapse(&text[start..end]));
    }

    if index >= toks.len() || !punct(&toks[index], "(") {
        let inline = collapse(text);
        return Some(Parsed {
            base_header: inline.clone(),
            inline_header: inline,
            pre_property: None,
            typed,
            body: None,
            tail: "",
        });
    }

    let open = &toks[index];
    let close = matching_paren(toks, index)?;
    let close_token = &toks[close];
    Some(Parsed {
        base_header: collapse(&text[..base_end]),
        inline_header: collapse(&text[..open.start]),
        pre_property,
        typed,
        body: Some(&text[token_end(open)..close_token.start]),
        tail: &text[token_end(close_token)..],
    })
}

fn parse_properties(text: &str) -> Option<Vec<String>> {
    let toks = significant(text);
    let mut properties = Vec::new();
    let mut index = 0;
    while index < toks.len() {
        let start = index;
        if toks[index].kind != Kind::Word {
            return None;
        }
        let lower = toks[index].text.to_ascii_lowercase();
        match lower.as_str() {
            "inherits" => {
                index += 1;
                index = consume_paren_group(&toks, index)?;
            }
            "partition" => {
                index += 1;
                if word_at(&toks, index, "of") {
                    index += 1;
                    consume_qualified(&toks, &mut index)?;
                } else if word_at(&toks, index, "by") {
                    index += 1;
                    index = consume_ident(&toks, index)?;
                    index = consume_paren_group(&toks, index)?;
                } else {
                    return None;
                }
            }
            "for" => {
                index += 1;
                if !word_at(&toks, index, "values") {
                    return None;
                }
                index += 1;
                if word_at(&toks, index, "in") {
                    index += 1;
                    index = consume_paren_group(&toks, index)?;
                } else if word_at(&toks, index, "from") {
                    index += 1;
                    index = consume_paren_group(&toks, index)?;
                    if !word_at(&toks, index, "to") {
                        return None;
                    }
                    index += 1;
                    index = consume_paren_group(&toks, index)?;
                } else if word_at(&toks, index, "with") {
                    index += 1;
                    index = consume_paren_group(&toks, index)?;
                } else {
                    return None;
                }
            }
            "using" => {
                index += 1;
                index = consume_ident(&toks, index)?;
            }
            "with" => {
                index += 1;
                if punct_at(&toks, index, "(") {
                    index = consume_paren_group(&toks, index)?;
                } else {
                    index = consume_ident(&toks, index)?;
                }
            }
            "without" => {
                if word_at(&toks, index + 1, "oids") {
                    index += 2;
                    continue;
                }
                return None;
            }
            "on" => {
                index += 1;
                if !word_at(&toks, index, "commit") {
                    return None;
                }
                index += 1;
                if word_at(&toks, index, "preserve") && word_at(&toks, index + 1, "rows") {
                    index += 2;
                    continue;
                } else if word_at(&toks, index, "delete") && word_at(&toks, index + 1, "rows") {
                    index += 2;
                } else if word_at(&toks, index, "drop") {
                    index += 1;
                } else {
                    return None;
                }
            }
            "tablespace" => {
                index += 1;
                index = consume_ident(&toks, index)?;
            }
            "default" => {
                index += 1;
            }
            _ => return None,
        }
        if index <= start {
            return None;
        }
        properties.push(collapse(
            &text[toks[start].start..token_end(&toks[index - 1])],
        ));
    }
    Some(properties)
}

fn properties_inline(properties: &[String]) -> bool {
    properties.len() <= 2
        && properties
            .iter()
            .all(|property| property_simple(property) && property_max_params(property) <= 3)
}

fn property_simple(text: &str) -> bool {
    significant(text).iter().all(|token| {
        token.kind != Kind::Word
            || !matches!(
                token.text.to_ascii_lowercase().as_str(),
                "select" | "case" | "and" | "or"
            )
    })
}

fn property_max_params(text: &str) -> usize {
    let toks = significant(text);
    let depth = depths(&toks);
    let mut maximum = 0usize;
    let mut index = 0;
    while index < toks.len() {
        if depth[index] == 0
            && punct(&toks[index], "(")
            && let Some(close) = matching_paren(&toks, index)
        {
            let inner = &text[token_end(&toks[index])..toks[close].start];
            maximum = maximum.max(split_top_level(inner).len());
            index = close + 1;
            continue;
        }
        index += 1;
    }
    maximum
}

fn split_top_level(text: &str) -> Vec<&str> {
    let toks = significant(text);
    if toks.is_empty() {
        return Vec::new();
    }
    let depth = depths(&toks);
    let mut parts = Vec::new();
    let mut start = 0usize;
    for (index, token) in toks.iter().enumerate() {
        if depth[index] == 0 && punct(token, ",") {
            let part = text[start..token.start].trim();
            if !part.is_empty() {
                parts.push(part);
            }
            start = token.start + 1;
        }
    }
    let part = text[start..].trim();
    if !part.is_empty() {
        parts.push(part);
    }
    parts
}

fn depths(tokens: &[Token<'_>]) -> Vec<usize> {
    let mut depth = 0usize;
    let mut result = Vec::with_capacity(tokens.len());
    for token in tokens {
        result.push(depth);
        if token.kind == Kind::Punct {
            match token.text {
                "(" | "[" => depth += 1,
                ")" | "]" => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
    }
    result
}

fn matching_paren(tokens: &[Token<'_>], open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate().skip(open) {
        if token.kind != Kind::Punct {
            continue;
        }
        match token.text {
            "(" => depth += 1,
            ")" => {
                if depth == 0 {
                    return None;
                }
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

fn consume_paren_group(tokens: &[Token<'_>], index: usize) -> Option<usize> {
    if !punct_at(tokens, index, "(") {
        return None;
    }
    matching_paren(tokens, index).map(|close| close + 1)
}

fn consume_ident(tokens: &[Token<'_>], index: usize) -> Option<usize> {
    tokens.get(index).is_some_and(is_ident).then_some(index + 1)
}

fn consume_qualified(tokens: &[Token<'_>], index: &mut usize) -> Option<()> {
    if !tokens.get(*index).is_some_and(is_ident) {
        return None;
    }
    *index += 1;
    while *index + 1 < tokens.len() && punct(&tokens[*index], ".") && is_ident(&tokens[*index + 1])
    {
        *index += 2;
    }
    Some(())
}

fn significant(text: &str) -> Vec<Token<'_>> {
    tokens(text)
        .into_iter()
        .filter(|token| !matches!(token.kind, Kind::Whitespace | Kind::Comment))
        .collect()
}

/// 把 segment 内的空白折叠成一个空格；字符串与引号标识符原样保留。
fn collapse(text: &str) -> String {
    let mut output = String::new();
    for token in tokens(text) {
        match token.kind {
            Kind::Whitespace => {
                if !output.is_empty() {
                    output.push(' ');
                }
            }
            Kind::Comment => {}
            _ => output.push_str(token.text),
        }
    }
    output.trim().to_owned()
}

fn slice<'a>(text: &'a str, toks: &[Token<'a>], start: usize, end: usize) -> &'a str {
    &text[toks[start].start..token_end(&toks[end - 1])]
}

fn token_end(token: &Token<'_>) -> usize {
    token.start + token.text.len()
}

fn is_word(token: &Token<'_>, word: &str) -> bool {
    token.kind == Kind::Word && token.text.eq_ignore_ascii_case(word)
}

fn is_ident(token: &Token<'_>) -> bool {
    matches!(token.kind, Kind::Word | Kind::QuotedIdent)
}

fn punct(token: &Token<'_>, text: &str) -> bool {
    token.kind == Kind::Punct && token.text == text
}

fn word_at(toks: &[Token<'_>], index: usize, word: &str) -> bool {
    toks.get(index).is_some_and(|token| is_word(token, word))
}

fn punct_at(toks: &[Token<'_>], index: usize, text: &str) -> bool {
    toks.get(index).is_some_and(|token| punct(token, text))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rendered(sql: &str) -> String {
        layout(sql, Dialect::Postgres).expect(sql)
    }

    fn check(sql: &str, expected: &str) {
        let output = rendered(sql);
        assert_eq!(output, expected, "input: {sql}");
        assert_eq!(rendered(&output), output, "not idempotent: {sql}");
    }

    #[test]
    fn three_columns_stay_inline() {
        check(
            "create table t (a int, b text, c boolean);",
            "create table t (a int, b text, c boolean)",
        );
    }

    #[test]
    fn four_columns_break() {
        check(
            "create table t (a int, b int, c int, d int);",
            "create table t (\n    a int,\n    b int,\n    c int,\n    d int\n)",
        );
    }

    #[test]
    fn table_constraint_breaks_body() {
        check(
            "create table t (a int, b int, constraint uq unique (b));",
            "create table t (\n    a int,\n    b int,\n    constraint uq unique (b)\n)",
        );
    }

    #[test]
    fn column_primary_key_is_promoted_with_not_null() {
        check(
            "create table t (id int primary key, name text);",
            "create table t (\n    id   int not null,\n    name text,\n    primary key (id)\n)",
        );
    }

    #[test]
    fn typed_table_primary_key_keeps_options() {
        check(
            "create table employees of employee_type (name with options primary key, salary with options default 1000);",
            "create table employees of employee_type (\n    name with options not null,\n    salary with options default 1000,\n    primary key (name)\n)",
        );
    }

    #[test]
    fn table_primary_key_adds_not_null() {
        check(
            "create table t (a int, b int, primary key (a, b));",
            "create table t (\n    a int not null,\n    b int not null,\n    primary key (a, b)\n)",
        );
    }

    #[test]
    fn named_primary_key_keeps_constraint_name() {
        check(
            "create table t (id int constraint pk primary key);",
            "create table t (\n    id int not null,\n    constraint pk primary key (id)\n)",
        );
    }

    #[test]
    fn broken_columns_align_name_type_and_not_null() {
        check(
            "create table t (a int, longer varchar(10), flag boolean not null, d date);",
            "create table t (\n    a      int,\n    longer varchar(10),\n    flag   boolean not null,\n    d      date\n)",
        );
    }

    #[test]
    fn last_type_does_not_widen_not_null() {
        check(
            "create table t (code char(5) not null, title varchar(40) not null, date_prod date, kind varchar(10), len interval hour to minute, primary key (code));",
            "create table t (\n    code      char(5)     not null,\n    title     varchar(40) not null,\n    date_prod date,\n    kind      varchar(10),\n    len       interval hour to minute,\n    primary key (code)\n)",
        );
    }

    #[test]
    fn last_type_does_not_widen_not_null_in_user_table() {
        check(
            "create table myschema.user (user_id bigint not null, name varchar(40), primary key (user_id));",
            "create table myschema.user (\n    user_id bigint not null,\n    name    varchar(40),\n    primary key (user_id)\n)",
        );
    }

    #[test]
    fn absent_optional_fields_reserve_columns_before_rest() {
        check(
            "create table t (id bigint not null default 1, a text collate numeric not null unique, b text unique);",
            "create table t (\n    id bigint                 not null default 1,\n    a  text   collate numeric not null unique,\n    b  text                            unique\n)",
        );
    }

    #[test]
    fn extra_modifier_limits_inline_to_two_columns() {
        check(
            "create table t (a int default 1, b int default 2);",
            "create table t (a int default 1, b int default 2)",
        );
        check(
            "create table t (a int default 1, b int default 2, c int default 3);",
            "create table t (\n    a int default 1,\n    b int default 2,\n    c int default 3\n)",
        );
    }

    #[test]
    fn column_field_order_is_d2_order() {
        check(
            "create table t (a int default 1 unique collate \"C\" not null);",
            "create table t (a int collate \"C\" not null default 1 unique)",
        );
    }

    #[test]
    fn constraint_name_stays_attached() {
        check(
            "create table t (a int constraint nn not null, b int);",
            "create table t (a int constraint nn not null, b int)",
        );
    }

    #[test]
    fn explicit_null_and_default_storage_are_omitted() {
        check("create table t (a int null);", "create table t (a int)");
        check(
            "create table t (a text storage default, b text compression default);",
            "create table t (a text, b text)",
        );
    }

    #[test]
    fn type_names_are_canonical() {
        check(
            "create table t (a integer, b int8, c bool);",
            "create table t (a int, b bigint, c boolean)",
        );
        check(
            "create table t (a numeric(10, 2), b real, c double precision);",
            "create table t (a decimal(10, 2), b float4, c float8)",
        );
        check(
            "create table t (a timestamp with time zone, b time with time zone, c character varying(40));",
            "create table t (a timestamptz, b timetz, c varchar(40))",
        );
        check(
            "create table t (a character(5), b int4, c character varying);",
            "create table t (a char(5), b int, c varchar)",
        );
        check(
            "create table t (a timestamp(3) with time zone, b timestamp without time zone);",
            "create table t (a timestamptz(3), b timestamp)",
        );
    }

    #[test]
    fn collate_numeric_is_not_a_type_name() {
        check(
            "create table t (a text collate numeric not null);",
            "create table t (a text collate numeric not null)",
        );
        check(
            "create table t (a numeric(10, 2) collate numeric);",
            "create table t (a decimal(10, 2) collate numeric)",
        );
    }

    #[test]
    fn quoted_identifiers_and_strings_are_preserved() {
        check(
            "create table \"My Table\" (\"Col\" character varying(10) default 'Hello, World' not null);",
            "create table \"My Table\" (\"Col\" varchar(10) not null default 'Hello, World')",
        );
    }

    #[test]
    fn empty_body_stays_inline() {
        check("create table t ();", "create table t ()");
        check(
            "create table t () inherits (parent_table);",
            "create table t () inherits (parent_table)",
        );
    }

    #[test]
    fn properties_inline_when_body_and_property_are_simple() {
        check(
            "create table t (a int) partition by range (a);",
            "create table t (a int) partition by range (a)",
        );
        check(
            "create table t (a int) partition by range (extract(year from a), extract(month from a));",
            "create table t (a int) partition by range (extract(year from a), extract(month from a))",
        );
        check(
            "create table t (a int) with (fillfactor=70) tablespace ts;",
            "create table t (a int) with (fillfactor=70) tablespace ts",
        );
    }

    #[test]
    fn properties_break_with_broken_body_or_too_many_clauses() {
        check(
            "create table t (a int, b int, c int, d int) partition by range (a);",
            "create table t (\n    a int,\n    b int,\n    c int,\n    d int\n)\npartition by range (a)",
        );
        check(
            "create table t (a int) with (fillfactor=70) tablespace ts on commit delete rows;",
            "create table t (a int)\nwith (fillfactor=70)\ntablespace ts\non commit delete rows",
        );
    }

    #[test]
    fn partition_of_keeps_its_line_before_a_broken_body() {
        check(
            "create table cities_ab partition of cities (constraint city_id_nonzero check (city_id != 0)) for values in ('a', 'b');",
            "create table cities_ab\npartition of cities (\n    constraint city_id_nonzero check (city_id != 0)\n)\nfor values in ('a', 'b')",
        );
    }

    #[test]
    fn default_properties_are_omitted() {
        check(
            "create table t (a int) without oids;",
            "create table t (a int)",
        );
        check(
            "create table t (a int) on commit preserve rows;",
            "create table t (a int)",
        );
    }

    #[test]
    fn simple_exclude_list_stays_inline() {
        check(
            "create table t (a text, b text, exclude using gist (a with &&));",
            "create table t (\n    a text,\n    b text,\n    exclude using gist (a with &&)\n)",
        );
    }

    #[test]
    fn long_exclude_list_breaks() {
        check(
            "create table t (a text, b text, c text, d text, exclude using gist (a with =, b with =, c with =, d with =));",
            "create table t (\n    a text,\n    b text,\n    c text,\n    d text,\n    exclude using gist (\n        a with =,\n        b with =,\n        c with =,\n        d with =\n    )\n)",
        );
    }

    #[test]
    fn ctas_is_not_owned() {
        assert_eq!(
            layout("create table t as select 1", Dialect::Postgres),
            None
        );
        assert_eq!(
            layout("create table t (a int) as select 1", Dialect::Postgres),
            None
        );
    }

    #[test]
    fn no_paren_partition_table_stays_one_line() {
        check(
            "create table cities_partdef partition of cities default;",
            "create table cities_partdef partition of cities default",
        );
    }
}
