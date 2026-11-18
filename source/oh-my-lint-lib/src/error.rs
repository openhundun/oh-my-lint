//! 库错误：I/O、无法解析、规则拒绝修复。

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Unsupported(String),
    Rejected(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Unsupported(reason) => write!(formatter, "{reason}"),
            Self::Rejected(reason) => write!(formatter, "{reason}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Unsupported(_) | Self::Rejected(_) => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
