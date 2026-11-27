//! 集合运算：`union` / `intersect` / `except` 的运算符、分支与括号。

use crate::sql::postgresql::dql::checker::Violation;
use crate::sql::postgresql::dql::clause::Mode;
use crate::sql::postgresql::dql::util::block_indent;

/// 集合运算：运算符独占一行、分支按独立语句块判定、分支不加括号。
pub(super) fn check(statement_start: usize, statement_text: &str, block: crate::span::Span, clauses: &[crate::sql::splitter::Clause], found: &mut Vec<Violation>) {
    let text = &statement_text[block.start..block.end];
    let block_indent = block_indent(statement_text, block.start);
    let operators: Vec<&crate::sql::splitter::Clause> = clauses.iter().filter(|clause| crate::sql::splitter::SET_OPERATORS.contains(&clause.kind)).collect();
    for operator in &operators {
        let relative = block.start + operator.keyword_start;
        let line_start = statement_text[..relative].rfind('\n').map_or(0, |index| index + 1);
        let prefix = &statement_text[line_start..relative];
        if prefix.chars().all(|character| character == ' ' || character == '\t') {
            let mut start = line_start;
            let bytes = statement_text.as_bytes();
            while start > 0 && matches!(bytes[start - 1], b' ' | b'\t' | b'\r' | b'\n') {
                start -= 1;
            }
            let blank = statement_text[start..line_start].matches('\n').count() > 1;
            if prefix.len() == block_indent && !blank {
                continue;
            }
            // 运算符已经在独立一行；把前导空白一起替换，既修正缩进也清掉多余空行。
            found.push(Violation {
                kind: "set-op",
                element_count: 0,
                expected: Mode::Break,
                span: crate::span::Span::new(statement_start + start, statement_start + relative + operator.kind.len()),
                indent: block_indent,
                next_indent: None,
                message: format!("{} should start a new line", operator.kind),
            });
            continue;
        }
        let mut start = relative;
        let bytes = statement_text.as_bytes();
        while start > 0 && matches!(bytes[start - 1], b' ' | b'\t') {
            start -= 1;
        }
        found.push(Violation {
            kind: "set-op",
            element_count: 0,
            expected: Mode::Break,
            span: crate::span::Span::new(statement_start + start, statement_start + relative + operator.kind.len()),
            indent: block_indent,
            next_indent: None,
            message: format!("{} should start a new line", operator.kind),
        });
    }
    let mut bounds: Vec<(usize, usize)> = Vec::new();
    let mut start = 0;
    for operator in &operators {
        bounds.push((start, operator.keyword_start));
        start = operator.body_start;
    }
    bounds.push((start, text.len()));
    for (index, (branch_start, branch_end)) in bounds.into_iter().enumerate() {
        let branch = &text[branch_start..branch_end];
        let trimmed = branch.trim();
        if trimmed.is_empty() {
            continue;
        }
        let leading = branch.len() - branch.trim_start().len();
        let trailing = branch.trim_end().len();
        let absolute_start = block.start + branch_start + leading;
        if index > 0 {
            let line_start = statement_text[..absolute_start].rfind('\n').map_or(0, |position| position + 1);
            let prefix = &statement_text[line_start..absolute_start];
            if !(prefix.chars().all(|character| character == ' ' || character == '\t') && prefix.len() == block_indent) {
                let bytes = statement_text.as_bytes();
                let mut start = absolute_start;
                while start > 0 && matches!(bytes[start - 1], b' ' | b'\t') {
                    start -= 1;
                }
                found.push(Violation {
                    kind: "set-op-branch",
                    element_count: 0,
                    expected: Mode::Break,
                    span: crate::span::Span::new(statement_start + start, statement_start + absolute_start),
                    indent: block_indent,
                    next_indent: None,
                    message: "branch should start a new line".to_owned(),
                });
            }
        }
        if trimmed.starts_with('(') && trimmed.ends_with(')') {
            found.push(Violation {
                kind: "set-op-parens",
                element_count: 0,
                expected: Mode::Inline,
                span: crate::span::Span::new(statement_start + absolute_start, statement_start + block.start + branch_start + trailing),
                indent: block_indent,
                next_indent: None,
                message: "set operation branch should not be parenthesized".to_owned(),
            });
            continue;
        }
        super::checker::check_clauses(statement_start, statement_text, absolute_start, block.start + branch_start + trailing, found);
    }
}

/// 集合运算的三种渲染：运算符、分支起始、去分支括号。
pub(super) fn render(text: &str, violation: &Violation) -> Option<String> {
    if violation.kind == "set-op" {
        let operator = text[violation.span.start..violation.span.end].trim();
        return Some(format!("\n{}{operator}", " ".repeat(violation.indent)));
    }
    if violation.kind == "set-op-branch" {
        return Some(format!("\n{}", " ".repeat(violation.indent)));
    }
    if violation.kind == "set-op-parens" {
        let branch = text[violation.span.start..violation.span.end].trim();
        let masked = crate::sql::lexer::mask(branch);
        let mut depth = 0;
        let mut close = None;
        for (index, character) in masked.char_indices() {
            if character == '(' {
                depth += 1;
            } else if character == ')' {
                depth -= 1;
                if depth == 0 {
                    close = Some(index);
                    break;
                }
            }
        }
        if close != Some(branch.len() - 1) {
            return None;
        }
        return Some(branch[1..branch.len() - 1].trim().to_owned());
    }
    None
}
