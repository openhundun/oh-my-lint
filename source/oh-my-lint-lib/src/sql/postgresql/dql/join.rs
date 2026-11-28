//! `from` 的 join 版式与隐式 cross join 改写。

use crate::sql::postgresql::dql::checker::Violation;
use crate::sql::postgresql::dql::clause::Mode;
use crate::sql::postgresql::dql::util::line_indent;

/// `from` 含 join：首表留在关键字行、每个 join 独占一行、显式 join 后不写逗号。
pub(super) fn check(statement_start: usize, statement_text: &str, base: usize, clause: &crate::sql::splitter::Clause, body: &str, found: &mut Vec<Violation>) {
    if !crate::sql::decision::has_join(body) {
        return;
    }
    let from_indent = line_indent(statement_text, base + clause.keyword_start);
    let body_base = base + clause.body_start;
    if body.trim_start_matches([' ', '\t']).starts_with('\n') {
        found.push(Violation {
            kind: "from-join",
            element_count: 0,
            expected: Mode::Break,
            span: crate::span::Span::new(statement_start + body_base, statement_start + body_base + body.len()),
            indent: from_indent,
            next_indent: None,
            message: "from should keep the first table on the keyword line".to_owned(),
            condition: None,
        });
    }
    let tokens = crate::sql::lexer::tokens(body);
    let mut depth = 0;
    for token in &tokens {
        match token.kind {
            crate::sql::lexer::Kind::Punct if token.text == "(" => depth += 1,
            crate::sql::lexer::Kind::Punct if token.text == ")" => depth -= 1,
            crate::sql::lexer::Kind::Punct if token.text == "," && depth == 0 => {
                found.push(Violation {
                    kind: "cross-join",
                    element_count: 0,
                    expected: Mode::Break,
                    span: crate::span::Span::new(statement_start + body_base + token.start, statement_start + body_base + token.start + 1),
                    indent: from_indent,
                    next_indent: None,
                    message: "implicit cross join should be written as cross join".to_owned(),
                    condition: None,
                });
            }
            crate::sql::lexer::Kind::Word if depth == 0 && token.text.eq_ignore_ascii_case("join") => {
                let absolute = body_base + token.start;
                let line_start = statement_text[..absolute].rfind('\n').map_or(0, |index| index + 1);
                let prefix = &statement_text[line_start..absolute];
                let leading = prefix.len() - prefix.trim_start().len();
                let mut clause_start = absolute;
                for (offset, word) in prefix_words(prefix) {
                    if crate::sql::splitter::is_join_modifier(word) {
                        clause_start = line_start + offset;
                    } else {
                        break;
                    }
                }
                let at_line_start = prefix[..clause_start - line_start].trim().is_empty();
                if at_line_start {
                    if leading == from_indent {
                        continue;
                    }
                    clause_start = line_start;
                }
                found.push(Violation {
                    kind: "from-join",
                    element_count: 0,
                    expected: Mode::Break,
                    span: crate::span::Span::new(statement_start + clause_start, statement_start + absolute + token.text.len()),
                    indent: from_indent,
                    next_indent: None,
                    message: "join should start a new line".to_owned(),
                    condition: None,
                });
            }
            _ => {}
        }
    }
}

/// 前缀中的词及字节偏移；从右往左用于识别 join 修饰词。
fn prefix_words(prefix: &str) -> Vec<(usize, &str)> {
    let mut words = Vec::new();
    let mut start = 0usize;
    for (index, ch) in prefix.char_indices() {
        if ch.is_whitespace() {
            if start < index {
                words.push((start, &prefix[start..index]));
            }
            start = index + ch.len_utf8();
        }
    }
    if start < prefix.len() {
        words.push((start, &prefix[start..]));
    }
    words.reverse();
    words
}

/// 显式 join 后的逗号表改写为 `cross join`。
pub(super) fn render_cross_join(violation: &Violation) -> Option<String> {
    let pad = " ".repeat(violation.indent);
    Some(format!("\n{pad}cross join"))
}

/// `from` 的 join 版式：首表留在 `from` 行，join 行回到 `from` 缩进。
pub(super) fn render_from_join(text: &str, violation: &Violation) -> Option<String> {
    let body = &text[violation.span.start..violation.span.end];
    if body.trim_start_matches([' ', '\t']).starts_with('\n') {
        return render_from_body(body, violation.indent);
    }
    let clause = body.trim();
    let pad = " ".repeat(violation.indent);
    let at_line_start = violation.span.start == 0 || text[..violation.span.start].ends_with('\n');
    Some(format!("{}{pad}{clause}", if at_line_start { "" } else { "\n" }))
}

/// 深度 0 的 join 起点；包含 `left`/`outer` 等修饰词。
pub(super) fn join_starts(body: &str) -> Vec<usize> {
    let tokens = crate::sql::lexer::tokens(body);
    let mut depth = 0i32;
    let mut starts = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        match token.kind {
            crate::sql::lexer::Kind::Punct if token.text == "(" => depth += 1,
            crate::sql::lexer::Kind::Punct if token.text == ")" => depth -= 1,
            crate::sql::lexer::Kind::Word if depth == 0 && token.text.eq_ignore_ascii_case("join") => {
                let mut start = token.start;
                let mut probe = index;
                while probe > 0 {
                    probe -= 1;
                    let previous = &tokens[probe];
                    if previous.kind == crate::sql::lexer::Kind::Whitespace {
                        continue;
                    }
                    if previous.kind == crate::sql::lexer::Kind::Word && crate::sql::splitter::is_join_modifier(previous.text) {
                        start = previous.start;
                        continue;
                    }
                    break;
                }
                starts.push(start);
            }
            _ => {}
        }
    }
    starts
}

/// 首表原在下一行时，重排整个 `from` 正文。
fn render_from_body(body: &str, indent: usize) -> Option<String> {
    let starts = join_starts(body);
    if starts.is_empty() {
        return None;
    }
    let first = body[..starts[0]].trim();
    let mut rendered = String::new();
    if !first.is_empty() {
        rendered.push(' ');
        rendered.push_str(first);
    }
    for (index, &start) in starts.iter().enumerate() {
        let end = starts.get(index + 1).copied().unwrap_or(body.len());
        rendered.push('\n');
        rendered.push_str(&" ".repeat(indent));
        rendered.push_str(body[start..end].trim());
    }
    let trailing = body.trim_end().len();
    rendered.push_str(&body[trailing..]);
    Some(rendered)
}
