use hivecore_runtime_core::RuntimeError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ToolError {
    #[error("path escapes workspace root: {0}")]
    PathEscape(String),
    #[error("path does not exist: {0}")]
    NotFound(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid argument: {0}")]
    InvalidArg(String),
    #[error("regex error: {0}")]
    Regex(#[from] regex::Error),
    #[error("timeout after {0}ms")]
    Timeout(u64),
    #[error("aborted")]
    Aborted,
    #[error("string not found in file")]
    StringNotFound,
    #[error("string occurs multiple times in file ({count} matches) — narrow the context")]
    StringNotUnique { count: usize },
    #[error("file not read in this session — call read_file first")]
    NotReadYet,
}

impl From<ToolError> for RuntimeError {
    fn from(e: ToolError) -> Self {
        match e {
            ToolError::Aborted => RuntimeError::Aborted,
            ToolError::InvalidArg(m) => RuntimeError::InvalidInput(m),
            other => RuntimeError::ToolFailed(other.to_string()),
        }
    }
}
