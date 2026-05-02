use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExtensionError {
    #[error("wasmtime: {0}")]
    Wasmtime(#[from] anyhow::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid args: {0}")]
    InvalidArgs(String),
    #[error("tool returned error: {0}")]
    ToolFailed(String),
    #[error("tool not found in extension: {0}")]
    ToolNotFound(String),
}
