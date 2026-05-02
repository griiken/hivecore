use thiserror::Error;

#[derive(Debug, Error)]
pub enum OpenAiError {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("api error ({status}): {message}")]
    Api { status: u16, message: String },
    #[error("decode: {0}")]
    Decode(#[from] serde_json::Error),
    #[error("invalid sse frame: {0}")]
    Sse(String),
    #[error("unexpected end of stream")]
    UnexpectedEnd,
    #[error("unsupported: {0}")]
    Unsupported(String),
}
