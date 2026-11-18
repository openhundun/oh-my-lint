//! 统一 diff 输出：CLI 与人读的补丁都从这里生成。

/// 生成 `a/<path>` 到 `b/<path>` 的统一 diff；没有差异时返回空串。
pub fn unified(path: &str, original: &str, fixed: &str) -> String {
    if original == fixed {
        return String::new();
    }
    let diff = similar::TextDiff::from_lines(original, fixed);
    let display = path.trim_start_matches('/');
    diff.unified_diff()
        .header(&format!("a/{display}"), &format!("b/{display}"))
        .context_radius(3)
        .to_string()
}

#[cfg(test)]
mod tests {
    #[test]
    fn keeps_outer_lines_untouched() {
        let before = "select a\nfrom t\n";
        let after = "select a, b\nfrom t\n";
        let patch = crate::diff::unified("q.sql", before, after);
        assert!(patch.contains("--- a/q.sql"));
        assert!(patch.contains("-select a"));
        assert!(patch.contains("+select a, b"));
        assert!(patch.contains(" from t"));
    }

    #[test]
    fn empty_when_equal() {
        assert!(crate::diff::unified("q.sql", "select 1\n", "select 1\n").is_empty());
    }
}
