//! 字节区间：所有规则、编辑与文档映射共用的一种坐标。

/// 源码里的半开字节区间 `[start, end)`。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd, serde::Deserialize, serde::Serialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    /// 构造一个区间；`start` 大于 `end` 时交换两端。
    pub fn new(start: usize, end: usize) -> Self {
        if start <= end { Self { start, end } } else { Self { start: end, end: start } }
    }

    /// 长度为 0 的区间，常用于插入点。
    pub fn empty(at: usize) -> Self {
        Self { start: at, end: at }
    }

    /// 区间长度。
    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// 区间是否为空。
    pub fn is_empty(&self) -> bool {
        self.start >= self.end
    }

    /// 偏移是否落在区间内（左闭右开）。
    pub fn contains(&self, offset: usize) -> bool {
        self.start <= offset && offset < self.end
    }

    /// 两个区间是否相交；空区间按包含判断。
    pub fn intersects(&self, other: Span) -> bool {
        if self.is_empty() {
            return other.contains(self.start) || other.end == self.start;
        }
        if other.is_empty() {
            return self.contains(other.start) || self.end == other.start;
        }
        self.start < other.end && other.start < self.end
    }

    /// 平移到另一段文本的坐标系。
    pub fn shift(&self, offset: usize) -> Self {
        Self { start: self.start + offset, end: self.end + offset }
    }

    /// 截断到 `limit` 之内。
    pub fn clamp(&self, limit: usize) -> Self {
        Self { start: self.start.min(limit), end: self.end.min(limit) }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn normalizes_reversed_bounds() {
        assert_eq!(crate::span::Span::new(9, 3), crate::span::Span::new(3, 9));
    }

    #[test]
    fn intersects_handles_empty_ranges() {
        let block = crate::span::Span::new(10, 20);
        assert!(block.intersects(crate::span::Span::empty(10)));
        assert!(block.intersects(crate::span::Span::empty(20)));
        assert!(!block.intersects(crate::span::Span::empty(9)));
        assert!(block.intersects(crate::span::Span::new(19, 25)));
        assert!(!block.intersects(crate::span::Span::new(20, 25)));
    }
}
