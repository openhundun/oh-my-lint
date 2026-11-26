//! 文档与块：`.sql` 整个文件是一块；`.py` 只认模块级 `SQL = """..."""` 常量。

/// 模块级 `SQL = """` 的起点；只匹配行首，避免抓函数里的局部量。
static PYTHON_SQL: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| regex::Regex::new(r#"(?m)^SQL\s*=\s*""""#).expect("valid SQL constant pattern"));

/// 一个源文件及其中的 SQL 块；块区间是文件字节坐标。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Document {
    pub path: std::path::PathBuf,
    pub text: String,
    pub blocks: Vec<crate::model::Block>,
}

impl Document {
    /// 按后缀加载：`.sql` 整文件，`.py` 抽取 SQL 常量。
    pub fn load(path: &std::path::Path) -> crate::error::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        Self::from_text(path.to_path_buf(), text)
    }

    /// 从已有文本构造，供 stdin 与测试使用。
    pub fn from_text(path: std::path::PathBuf, text: String) -> crate::error::Result<Self> {
        let blocks = match path.extension().and_then(|extension| extension.to_str()) {
            Some("sql") => sql_blocks(&text),
            Some("py") => python_blocks(&text),
            Some(other) => {
                return Err(crate::error::Error::Unsupported(format!("unsupported extension: {other}")));
            }
            None => {
                return Err(crate::error::Error::Unsupported("file has no extension".to_owned()));
            }
        };
        Ok(Self { path, text, blocks })
    }

    /// 字节偏移换算成 1 基行列；列按字符计。
    pub fn line_col(&self, offset: usize) -> (usize, usize) {
        let offset = offset.min(self.text.len());
        let mut line = 1;
        let mut column = 1;
        for (index, character) in self.text.char_indices() {
            if index >= offset {
                break;
            }
            if character == '\n' {
                line += 1;
                column = 1;
            } else {
                column += 1;
            }
        }
        (line, column)
    }

    /// 1 基行列换算成字节偏移；越界返回错误。
    pub fn offset(&self, line: usize, column: usize) -> crate::error::Result<usize> {
        if line == 0 || column == 0 {
            return Err(crate::error::Error::Rejected("line and column are 1-based".to_owned()));
        }
        let mut current_line = 1;
        let mut current_column = 1;
        for (index, character) in self.text.char_indices() {
            if current_line == line && current_column == column {
                return Ok(index);
            }
            if character == '\n' {
                current_line += 1;
                current_column = 1;
            } else {
                current_column += 1;
            }
        }
        if current_line == line && current_column == column {
            return Ok(self.text.len());
        }
        Err(crate::error::Error::Rejected(format!("offset out of range: {line}:{column}")))
    }

    /// 解析 `L:C-L:C` 形式的范围；结束列是开区间。
    pub fn parse_range(&self, spec: &str) -> crate::error::Result<crate::span::Span> {
        let (start, end) = spec.split_once('-').ok_or_else(|| crate::error::Error::Rejected(format!("invalid range: {spec}")))?;
        let (start_line, start_column) = parse_position(start)?;
        let (end_line, end_column) = parse_position(end)?;
        let start_offset = self.offset(start_line, start_column)?;
        let end_offset = self.offset(end_line, end_column)?;
        if start_offset > end_offset {
            return Err(crate::error::Error::Rejected(format!("range starts after it ends: {spec}")));
        }
        Ok(crate::span::Span::new(start_offset, end_offset))
    }
}

fn parse_position(spec: &str) -> crate::error::Result<(usize, usize)> {
    let (line, column) = spec.split_once(':').ok_or_else(|| crate::error::Error::Rejected(format!("invalid position: {spec}")))?;
    let line = line.parse::<usize>().map_err(|_| crate::error::Error::Rejected(format!("invalid line: {line}")))?;
    let column = column.parse::<usize>().map_err(|_| crate::error::Error::Rejected(format!("invalid column: {column}")))?;
    Ok((line, column))
}

fn sql_blocks(text: &str) -> Vec<crate::model::Block> {
    let trimmed = text.trim_matches('\n');
    if trimmed.trim().is_empty() {
        return Vec::new();
    }
    let start = text.len() - text.trim_start_matches('\n').len();
    let span = crate::span::Span::new(start, start + trimmed.len());
    vec![crate::model::Block { span, text: trimmed.to_owned(), language: crate::model::Language::Sql }]
}

fn python_blocks(text: &str) -> Vec<crate::model::Block> {
    let mut blocks = Vec::new();
    for matched in PYTHON_SQL.find_iter(text) {
        let body_start = matched.end();
        let Some(relative_end) = text[body_start..].find("\"\"\"") else {
            continue;
        };
        let body = &text[body_start..body_start + relative_end];
        let leading = body.len() - body.trim_start_matches('\n').len();
        let trailing = body.trim_end_matches('\n').len();
        let start = body_start + leading;
        let end = body_start + trailing;
        if text[start..end].trim().is_empty() {
            continue;
        }
        blocks.push(crate::model::Block {
            span: crate::span::Span::new(start, end),
            text: text[start..end].to_owned(),
            language: crate::model::Language::Sql,
        });
    }
    blocks
}

#[cfg(test)]
mod tests {
    #[test]
    fn extracts_python_sql_constant() {
        let text = "SQL = \"\"\"\nselect a from t\n\"\"\"\n";
        let document = crate::source::Document::from_text(std::path::PathBuf::from("q.py"), text.to_owned()).expect("load");
        assert_eq!(document.blocks.len(), 1);
        assert_eq!(document.blocks[0].text, "select a from t");
        assert_eq!(document.blocks[0].span, crate::span::Span::new(10, 25));
    }

    #[test]
    fn ignores_indented_local_sql() {
        let text = "def f():\n    SQL = \"\"\"\nselect 1\n\"\"\"\n";
        let document = crate::source::Document::from_text(std::path::PathBuf::from("q.py"), text.to_owned()).expect("load");
        assert!(document.blocks.is_empty());
    }

    #[test]
    fn trims_only_newlines_on_sql_files() {
        let text = "\nselect 1\n";
        let document = crate::source::Document::from_text(std::path::PathBuf::from("q.sql"), text.to_owned()).expect("load");
        assert_eq!(document.blocks[0].text, "select 1");
        assert_eq!(document.blocks[0].span, crate::span::Span::new(1, 9));
    }

    #[test]
    fn converts_line_and_column() {
        let text = "select 1\nfrom t\n";
        let document = crate::source::Document::from_text(std::path::PathBuf::from("q.sql"), text.to_owned()).expect("load");
        assert_eq!(document.line_col(9), (2, 1));
        assert_eq!(document.offset(2, 1).expect("offset"), 9);
        assert_eq!(document.parse_range("2:1-3:1").expect("range"), crate::span::Span::new(9, 16));
    }
}
