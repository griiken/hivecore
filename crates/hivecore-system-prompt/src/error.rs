use thiserror::Error;

#[derive(Debug, Error)]
pub enum PromptError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("source `{source_name}` failed: {message}")]
    Source {
        source_name: String,
        message: String,
    },
}
