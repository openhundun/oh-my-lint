//! SQL 词法与掩码：字符串、引号标识符、注释在这里被整体识别，供断行与判定复用。
//!
//! `tokens` 首尾相接覆盖输入全部字节，`start` 为字节偏移；`mask` 与输入等长，
//! 只把字面量与注释抹成空格并保留换行，因此掩码上的偏移、行号与原文一一对应。

/// 词法单元种类。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Word,
    QuotedIdent,
    String,
    Number,
    Punct,
    Comment,
    Whitespace,
}

/// 一个词法单元：种类、原文切片与起始字节偏移。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Token<'a> {
    pub kind: Kind,
    pub text: &'a str,
    pub start: usize,
}

/// 两字符运算符；先于单字符匹配，保证 `<=` 不被拆成 `<` 与 `=`。
const TWO_CHAR_PUNCT: [&str; 6] = ["<=", ">=", "<>", "!=", "||", "::"];

/// 切出全部词法单元，按顺序首尾相接覆盖输入。
pub fn tokens(text: &str) -> Vec<Token<'_>> {
    let bytes = text.as_bytes();
    let mut result = Vec::new();
    let mut cursor = 0;

    while cursor < bytes.len() {
        let start = cursor;
        let kind = match text[start..].chars().next().expect("cursor is inside text") {
            ch if ch.is_whitespace() => {
                cursor = scan_whitespace(text, start);
                Kind::Whitespace
            }
            '\'' => {
                cursor = scan_delimited(bytes, start, b'\'');
                Kind::String
            }
            '"' => {
                cursor = scan_delimited(bytes, start, b'"');
                Kind::QuotedIdent
            }
            '`' => {
                cursor = scan_delimited(bytes, start, b'`');
                Kind::QuotedIdent
            }
            '-' if bytes.get(start + 1) == Some(&b'-') => {
                cursor = scan_line_comment(bytes, start + 2);
                Kind::Comment
            }
            '/' if bytes.get(start + 1) == Some(&b'*') => {
                cursor = scan_block_comment(bytes, start + 2);
                Kind::Comment
            }
            ch if ch.is_ascii_digit() => {
                cursor = scan_number(bytes, start);
                Kind::Number
            }
            ch if ch.is_alphabetic() || ch == '_' => {
                cursor = scan_word(text, start);
                Kind::Word
            }
            ch => {
                cursor = scan_punct(text, start, ch.len_utf8());
                Kind::Punct
            }
        };
        result.push(Token {
            kind,
            text: &text[start..cursor],
            start,
        });
    }

    result
}

/// 抹掉字符串、引号标识符与注释的字节，保留换行；输出与输入等长。
pub fn mask(text: &str) -> String {
    let mut masked = String::with_capacity(text.len());
    for token in tokens(text) {
        match token.kind {
            Kind::String | Kind::QuotedIdent | Kind::Comment => {
                for byte in token.text.bytes() {
                    masked.push(if byte == b'\n' { '\n' } else { ' ' });
                }
            }
            _ => masked.push_str(token.text),
        }
    }
    masked
}

/// 文本中是否存在注释。
pub fn has_comment(text: &str) -> bool {
    tokens(text).iter().any(|token| token.kind == Kind::Comment)
}

/// 扫描连续空白，返回结束偏移。
fn scan_whitespace(text: &str, start: usize) -> usize {
    let mut end = start;
    for (offset, ch) in text[start..].char_indices() {
        if !ch.is_whitespace() {
            break;
        }
        end = start + offset + ch.len_utf8();
    }
    end
}

/// 扫描引号包围的片段：同引号双写为转义，未闭合时取到文本末尾。
fn scan_delimited(bytes: &[u8], start: usize, quote: u8) -> usize {
    let mut cursor = start + 1;
    while cursor < bytes.len() {
        if bytes[cursor] == quote {
            if bytes.get(cursor + 1) == Some(&quote) {
                cursor += 2;
                continue;
            }
            return cursor + 1;
        }
        cursor += 1;
    }
    bytes.len()
}

/// 扫描 `--` 注释正文，到行尾为止（不含换行）。
fn scan_line_comment(bytes: &[u8], mut cursor: usize) -> usize {
    while cursor < bytes.len() && bytes[cursor] != b'\n' {
        cursor += 1;
    }
    cursor
}

/// 扫描 `/* */` 注释正文；未闭合时取到文本末尾。
fn scan_block_comment(bytes: &[u8], mut cursor: usize) -> usize {
    while cursor < bytes.len() {
        if bytes[cursor] == b'*' && bytes.get(cursor + 1) == Some(&b'/') {
            return cursor + 2;
        }
        cursor += 1;
    }
    bytes.len()
}

/// 扫描数字：整数部分加可选小数与指数；指数必须后接数字才吞入。
fn scan_number(bytes: &[u8], start: usize) -> usize {
    let mut cursor = start;
    while cursor < bytes.len() && bytes[cursor].is_ascii_digit() {
        cursor += 1;
    }
    if bytes.get(cursor) == Some(&b'.') && bytes.get(cursor + 1).is_some_and(u8::is_ascii_digit) {
        cursor += 1;
        while cursor < bytes.len() && bytes[cursor].is_ascii_digit() {
            cursor += 1;
        }
    }
    if matches!(bytes.get(cursor), Some(b'e' | b'E')) {
        let mut probe = cursor + 1;
        if matches!(bytes.get(probe), Some(b'+' | b'-')) {
            probe += 1;
        }
        if bytes.get(probe).is_some_and(u8::is_ascii_digit) {
            cursor = probe;
            while cursor < bytes.len() && bytes[cursor].is_ascii_digit() {
                cursor += 1;
            }
        }
    }
    cursor
}

/// 扫描标识符：首字符为字母或 `_`，后续字母、数字、`_`、`$`。
fn scan_word(text: &str, start: usize) -> usize {
    let mut end = start;
    for (offset, ch) in text[start..].char_indices() {
        let accepted = if offset == 0 {
            ch.is_alphabetic() || ch == '_'
        } else {
            ch.is_alphanumeric() || ch == '_' || ch == '$'
        };
        if !accepted {
            break;
        }
        end = start + offset + ch.len_utf8();
    }
    end
}

/// 扫描标点：先尝试两字符运算符，否则只取当前字符。
fn scan_punct(text: &str, start: usize, width: usize) -> usize {
    let rest = &text[start..];
    if TWO_CHAR_PUNCT.iter().any(|op| rest.starts_with(*op)) {
        return start + 2;
    }
    start + width
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 断言词法结果首尾相接覆盖输入，且 `start` 为连续字节偏移。
    fn assert_covers(text: &str) {
        let tokens = tokens(text);
        let mut cursor = 0;
        for token in &tokens {
            assert_eq!(token.start, cursor, "token {token:?} start");
            assert!(!token.text.is_empty());
            cursor += token.text.len();
        }
        assert_eq!(cursor, text.len(), "tokens cover all bytes");
        let joined: String = tokens.iter().map(|token| token.text).collect();
        assert_eq!(joined, text);
    }

    fn kinds(text: &str) -> Vec<(Kind, &str)> {
        tokens(text)
            .into_iter()
            .map(|token| (token.kind, token.text))
            .collect()
    }

    #[test]
    fn string_hides_punctuation_and_keywords() {
        let sql = "select 'a, b and (c)' as x from t";
        assert_covers(sql);
        assert!(kinds(sql).contains(&(Kind::String, "'a, b and (c)'")));
        assert!(
            !tokens(sql)
                .iter()
                .any(|token| token.kind == Kind::Word && token.text == "and"),
            "字符串内的 and 不应成为关键字"
        );
        assert!(!tokens(sql).iter().any(|token| token.kind == Kind::Punct));
    }

    #[test]
    fn comments_swallow_quotes() {
        assert_eq!(
            kinds("-- don't\nselect 1"),
            vec![
                (Kind::Comment, "-- don't"),
                (Kind::Whitespace, "\n"),
                (Kind::Word, "select"),
                (Kind::Whitespace, " "),
                (Kind::Number, "1"),
            ]
        );
        assert_eq!(
            kinds("/* \"x\" */select"),
            vec![(Kind::Comment, "/* \"x\" */"), (Kind::Word, "select")]
        );
    }

    #[test]
    fn doubled_quotes_escape() {
        assert_eq!(kinds("'it''s'"), vec![(Kind::String, "'it''s'")]);
        assert_eq!(kinds("\"a\"\"b\""), vec![(Kind::QuotedIdent, "\"a\"\"b\"")]);
        assert_eq!(kinds("`a``b`"), vec![(Kind::QuotedIdent, "`a``b`")]);
    }

    #[test]
    fn two_char_operators_win() {
        let sql = "a<=b and c>=d and e<>f and g!=h and i||j and k::text";
        assert_covers(sql);
        let puncts: Vec<&str> = tokens(sql)
            .into_iter()
            .filter(|token| token.kind == Kind::Punct)
            .map(|token| token.text)
            .collect();
        assert_eq!(puncts, vec!["<=", ">=", "<>", "!=", "||", "::"]);
        assert_eq!(kinds(";"), vec![(Kind::Punct, ";")]);
    }

    #[test]
    fn block_comment_spans_lines() {
        let sql = "select 1 /* a\nb */, 2";
        assert_covers(sql);
        assert!(kinds(sql).contains(&(Kind::Comment, "/* a\nb */")));

        let masked = mask(sql);
        assert_eq!(masked.len(), sql.len());
        assert_eq!(masked.matches('\n').count(), sql.matches('\n').count());
        assert!(masked.contains('\n'));
        assert!(!masked.contains('a') && !masked.contains('b'));
    }

    #[test]
    fn mask_keeps_keywords_and_erases_literals() {
        let sql = "select 'x', \"y\" from t -- z";
        let masked = mask(sql);
        assert_eq!(masked.len(), sql.len());
        assert!(masked.contains("select"));
        assert!(masked.contains("from t"));
        assert!(!masked.contains('x'));
        assert!(!masked.contains('y'));
        assert!(!masked.contains('z'));
        assert_eq!(masked.replace(' ', ""), "select,fromt");
    }

    #[test]
    fn mask_is_line_stable_for_unclosed_constructs() {
        assert_covers("select 'abc");
        assert_eq!(kinds("select 'abc")[2], (Kind::String, "'abc"));
        assert_covers("select /* abc");
        assert_eq!(kinds("select /* abc")[2], (Kind::Comment, "/* abc"));
        assert_covers("select \"abc");
        assert_eq!(kinds("select \"abc")[2], (Kind::QuotedIdent, "\"abc"));
    }

    #[test]
    fn numbers_include_fraction_and_exponent() {
        assert_eq!(kinds("1"), vec![(Kind::Number, "1")]);
        assert_eq!(kinds("0.5"), vec![(Kind::Number, "0.5")]);
        assert_eq!(kinds("1.5e-3"), vec![(Kind::Number, "1.5e-3")]);
        assert_eq!(kinds("12E+7"), vec![(Kind::Number, "12E+7")]);
        // 指数后没有数字时不吞入 `e`。
        assert_eq!(
            kinds("1.5e"),
            vec![(Kind::Number, "1.5"), (Kind::Word, "e")]
        );
    }

    #[test]
    fn words_follow_unicode_rules() {
        let sql = "select 中文名 as 名 from 表 where _x$1 = 1";
        assert_covers(sql);
        assert!(kinds(sql).contains(&(Kind::Word, "中文名")));
        assert!(kinds(sql).contains(&(Kind::Word, "_x$1")));
    }

    #[test]
    fn has_comment_covers_three_inputs() {
        assert!(!has_comment("select a from t"));
        assert!(!has_comment("select 'a--b' from t"));
        assert!(has_comment("select a -- note\nfrom t"));
        assert!(has_comment("select a /* note */ from t"));
    }

    #[test]
    fn mask_preserves_byte_offsets() {
        let sql = "select \"列 名\", '值' /* 注\n释 */ from 表";
        let masked = mask(sql);
        assert_eq!(masked.len(), sql.len());
        assert_eq!(masked.lines().count(), sql.lines().count());
        // 掩码与原文字节对齐：未被抹掉的部分切片一致。
        for token in tokens(sql) {
            if !matches!(token.kind, Kind::String | Kind::QuotedIdent | Kind::Comment) {
                let end = token.start + token.text.len();
                assert_eq!(&masked[token.start..end], token.text);
            }
        }
        assert!(!masked.contains('列'));
        assert!(!masked.contains('注'));
        assert!(masked.contains("from 表"));
    }
}
