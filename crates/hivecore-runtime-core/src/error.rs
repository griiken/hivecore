//! Library error type. Per workspace convention (`CLAUDE.md` §critical-rules):
//! `thiserror` for libraries; binaries layer `anyhow` on top.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("tool not found: {0}")]
    ToolNotFound(String),
    #[error("tool execution failed: {0}")]
    ToolFailed(String),
    #[error("model adapter failed: {0}")]
    ModelFailed(String),
    #[error("aborted")]
    Aborted,
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("{0}")]
    Other(String),
}

pub type RuntimeResult<T> = Result<T, RuntimeError>;
