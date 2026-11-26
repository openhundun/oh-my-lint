//! 词法与掩码：词法交给 sqlparser 的 tokenizer（按方言），这里只做种类映射与掩码。
//!
//! 方言是本次检查的上下文：入口用 [`with_dialect`] 设置，tokenizer 按它识别
//! `$$`、`E''`、`#` 注释、反引号等；线程局部保存，退出后恢复。

use std::cell::Cell;

use sqlparser::tokenizer::{
    Location, Span as AstSpan, Token as SqlToken, Tokenizer, Whitespace as Ws,
};

use crate::span::Span;
use crate::sql::parse::Dialect;

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

thread_local! {
    static DIALECT: Cell<Dialect> = const { Cell::new(Dialect::Postgres) };
}

/// 在指定方言下执行一段词法相关逻辑；退出（含 panic）时恢复原值。
pub(crate) fn with_dialect<R>(dialect: Dialect, f: impl FnOnce() -> R) -> R {
    struct Restore(Dialect);

    impl Drop for Restore {
        fn drop(&mut self) {
            DIALECT.with(|cell| cell.set(self.0));
        }
    }

    let previous = DIALECT.with(|cell| cell.replace(dialect));
    let _restore = Restore(previous);
    f()
}

/// 当前词法方言；R35 等只对 PostgreSQL 生效的规则据此收窄。
pub(crate) fn current_dialect() -> Dialect {
    DIALECT.with(Cell::get)
}

/// 按当前方言切出词法单元；tokenizer 报错（未闭合字符串/注释等）时整段按字面量处理，不改写。
pub fn tokens(text: &str) -> Vec<Token<'_>> {
    let dialect = current_dialect();
    let Ok(raw) = Tokenizer::new(dialect.parser().as_ref(), text).tokenize_with_location() else {
        return fallback(text);
    };
    let index = LineIndex::new(text);
    let mut result = Vec::with_capacity(raw.len());
    let mut cursor = 0usize;
    for token in &raw {
        if matches!(token.token, SqlToken::EOF) {
            continue;
        }
        let Some(span) = index.span(text, token.span) else {
            return fallback(text);
        };
        if span.start < cursor || span.end > text.len() {
            return fallback(text);
        }
        if span.start > cursor {
            push_gap(text, cursor, span.start, &mut result);
        }
        result.push(Token {
            kind: kind_of(&token.token),
            text: &text[span.start..span.end],
            start: span.start,
        });
        cursor = span.end;
    }
    if cursor < text.len() {
        push_gap(text, cursor, text.len(), &mut result);
    }
    result
}

/// 抹掉字符串、引号标识符与注释的字节，保留换行；输出与输入等长。
pub fn mask(text: &str) -> String {
    let mut masked = String::with_capacity(text.len());
    let mut cursor = 0usize;
    for token in tokens(text) {
        if token.start > cursor {
            masked.push_str(&text[cursor..token.start]);
        }
        if matches!(token.kind, Kind::String | Kind::QuotedIdent | Kind::Comment) {
            for byte in token.text.bytes() {
                masked.push(if byte == b'\n' { '\n' } else { ' ' });
            }
        } else {
            masked.push_str(token.text);
        }
        cursor = token.start + token.text.len();
    }
    if cursor < text.len() {
        masked.push_str(&text[cursor..]);
    }
    masked
}

/// 文本中是否存在注释。
pub fn has_comment(text: &str) -> bool {
    tokens(text).iter().any(|token| token.kind == Kind::Comment)
}

fn fallback(text: &str) -> Vec<Token<'_>> {
    if text.is_empty() {
        return Vec::new();
    }
    vec![Token {
        kind: Kind::String,
        text,
        start: 0,
    }]
}

fn push_gap<'a>(text: &'a str, start: usize, end: usize, result: &mut Vec<Token<'a>>) {
    let gap = &text[start..end];
    let kind = if gap.trim().is_empty() {
        Kind::Whitespace
    } else {
        Kind::Punct
    };
    result.push(Token {
        kind,
        text: gap,
        start,
    });
}

fn kind_of(token: &SqlToken) -> Kind {
    use SqlToken::*;
    match token {
        Word(word) if word.quote_style.is_some() => Kind::QuotedIdent,
        Word(_) => Kind::Word,
        Number(_, _) => Kind::Number,
        Whitespace(Ws::SingleLineComment { .. } | Ws::MultiLineComment(_)) => Kind::Comment,
        Whitespace(_) => Kind::Whitespace,
        SingleQuotedString(_)
        | DoubleQuotedString(_)
        | TripleSingleQuotedString(_)
        | TripleDoubleQuotedString(_)
        | DollarQuotedString(_)
        | SingleQuotedByteStringLiteral(_)
        | DoubleQuotedByteStringLiteral(_)
        | TripleSingleQuotedByteStringLiteral(_)
        | TripleDoubleQuotedByteStringLiteral(_)
        | SingleQuotedRawStringLiteral(_)
        | DoubleQuotedRawStringLiteral(_)
        | TripleSingleQuotedRawStringLiteral(_)
        | TripleDoubleQuotedRawStringLiteral(_)
        | NationalStringLiteral(_)
        | QuoteDelimitedStringLiteral(_)
        | NationalQuoteDelimitedStringLiteral(_)
        | EscapedStringLiteral(_)
        | UnicodeStringLiteral(_)
        | HexStringLiteral(_)
        | CopyData { .. } => Kind::String,
        EOF => Kind::Whitespace,
        _ => Kind::Punct,
    }
}

/// 行首字节偏移；把 `Location` 的行列位置换成字节偏移。
pub(crate) struct LineIndex {
    starts: Vec<usize>,
}

impl LineIndex {
    pub(crate) fn new(text: &str) -> Self {
        let mut starts = vec![0];
        starts.extend(text.match_indices('\n').map(|(index, _)| index + 1));
        Self { starts }
    }

    pub(crate) fn offset(&self, text: &str, location: Location) -> Option<usize> {
        let line = usize::try_from(location.line).ok()?.checked_sub(1)?;
        let start = *self.starts.get(line)?;
        let end = self
            .starts
            .get(line + 1)
            .map_or(text.len(), |next| next - 1);
        let column = usize::try_from(location.column).ok()?.checked_sub(1)?;
        let line_text = &text[start..end];
        let byte = line_text
            .char_indices()
            .nth(column)
            .map_or(line_text.len(), |(index, _)| index);
        Some(start + byte)
    }

    pub(crate) fn span(&self, text: &str, span: AstSpan) -> Option<Span> {
        if span.start.line == 0 {
            return None;
        }
        let start = self.offset(text, span.start)?;
        let end = self.offset(text, span.end)?;
        (start <= end && end <= text.len()).then(|| Span::new(start, end))
    }
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
        with_dialect(Dialect::Mysql, || {
            assert_eq!(kinds("`a``b`"), vec![(Kind::QuotedIdent, "`a``b`")]);
        });
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
    fn unterminated_input_is_not_edited() {
        for sql in ["select 'abc", "select /* abc", "select \"abc"] {
            let tokens = tokens(sql);
            assert_eq!(tokens.len(), 1, "{sql}");
            assert_eq!(tokens[0].kind, Kind::String, "{sql}");
            assert_eq!(tokens[0].text, sql);
            let masked = mask(sql);
            assert_eq!(masked.len(), sql.len());
            assert_eq!(masked.matches('\n').count(), sql.matches('\n').count());
            assert!(!masked.contains("select"));
        }
    }

    #[test]
    fn numbers_include_fraction_and_exponent() {
        assert_eq!(kinds("1"), vec![(Kind::Number, "1")]);
        assert_eq!(kinds("0.5"), vec![(Kind::Number, "0.5")]);
        assert_eq!(kinds("1.5e-3"), vec![(Kind::Number, "1.5e-3")]);
        assert_eq!(kinds("12E+7"), vec![(Kind::Number, "12E+7")]);
        assert_eq!(
            kinds("1.5e"),
            vec![(Kind::Number, "1.5"), (Kind::Word, "e")]
        );
    }

    #[test]
    fn words_follow_unicode_rules() {
        let sql = "select 中文名 as 名 from 表 where _x = 1";
        assert_covers(sql);
        assert!(kinds(sql).contains(&(Kind::Word, "中文名")));
        assert!(kinds(sql).contains(&(Kind::Word, "_x")));
    }

    #[test]
    fn dialect_specific_lexing() {
        with_dialect(Dialect::Postgres, || {
            let sql = "select $$a;--x$$, E'it\\'s', a # b from t";
            assert_covers(sql);
            let tokens = tokens(sql);
            assert!(
                tokens
                    .iter()
                    .any(|token| token.kind == Kind::String && token.text.starts_with("$$"))
            );
            assert!(
                tokens
                    .iter()
                    .any(|token| token.kind == Kind::String && token.text.starts_with("E'"))
            );
            assert!(!has_comment("a # b"));
        });
        with_dialect(Dialect::Mysql, || {
            assert_covers("select `a#b`, 1 # c\nfrom t");
            assert!(has_comment("select 1 # c\nfrom t"));
            assert!(
                tokens("select `a` from t")
                    .iter()
                    .any(|token| token.kind == Kind::QuotedIdent && token.text == "`a`")
            );
        });
        with_dialect(Dialect::Hive, || {
            assert!(
                tokens("select `a` from t")
                    .iter()
                    .any(|token| token.kind == Kind::QuotedIdent && token.text == "`a`")
            );
        });
        with_dialect(Dialect::DuckDb, || {
            assert!(
                tokens("select $$x; y$$ from t")
                    .iter()
                    .any(|token| token.kind == Kind::String && token.text == "$$x; y$$")
            );
        });
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
