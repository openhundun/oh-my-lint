//! DQL 布局共用的小工具：缩进、词边界与空白折叠。

/// 行首到 `offset` 的空格数。
pub(super) fn line_indent(text: &str, offset: usize) -> usize {
    let line_start = text[..offset].rfind('\n').map_or(0, |index| index + 1);
    text[line_start..offset].chars().take_while(|character| *character == ' ').count()
}

/// 语句块首列：括号后的语句体按 +4 计算。
pub(super) fn block_indent(text: &str, block_start: usize) -> usize {
    let line_start = text[..block_start].rfind('\n').map_or(0, |index| index + 1);
    let prefix = &text[line_start..block_start];
    let indent = prefix.chars().take_while(|character| *character == ' ').count();
    if prefix.chars().all(|character| character == ' ' || character == '\t') {
        indent
    } else if prefix.trim_end().ends_with('(') {
        indent + 4
    } else {
        indent
    }
}

/// 正文最后一行的缩进；正文不以“纯缩进尾行”结束则为 `None`。
pub(super) fn trailing_indent(body: &str) -> Option<usize> {
    let newline = body.rfind('\n')?;
    let tail = &body[newline + 1..];
    if tail.chars().all(|character| character == ' ' || character == '\t') {
        Some(tail.chars().count())
    } else {
        None
    }
}

/// 文本去掉前导空白后是否以某个词开头（按词边界判断）。
pub(super) fn starts_with_word(text: &str, word: &str) -> bool {
    let Some(rest) = text.trim_start().strip_prefix(word) else {
        return false;
    };
    rest.chars().next().is_none_or(|character| !(character.is_alphanumeric() || character == '_' || character == '$'))
}

/// 把任意空白折叠成一个空格；字符串与引号逐字节保留。
pub(super) fn collapse_whitespace(text: &str) -> String {
    let mut output = String::new();
    let mut pending = false;
    for token in crate::sql::lexer::tokens(text) {
        if token.kind == crate::sql::lexer::Kind::Whitespace {
            if !output.is_empty() {
                pending = true;
            }
            continue;
        }
        if pending {
            output.push(' ');
            pending = false;
        }
        output.push_str(token.text);
    }
    output
}
