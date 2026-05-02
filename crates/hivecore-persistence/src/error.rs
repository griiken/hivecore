use thiserror::Error;

#[derive(Debug, Error)]
pub enum PersistenceError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("decode: {0}")]
    Decode(#[from] serde_json::Error),
    #[error("invalid session file: {0}")]
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, PersistenceError>;
