use thiserror::Error;

#[derive(Debug, Error)]
pub enum SkillError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("malformed frontmatter at {path}: {message}")]
    BadFrontmatter { path: String, message: String },
    #[error("missing required field `{field}` at {path}")]
    MissingField { path: String, field: &'static str },
    #[error("invalid skill name `{0}` — must match ^[a-z][a-z0-9_-]*$")]
    InvalidName(String),
    #[error("duplicate skill name `{name}` (paths: {first}, {second})")]
    Duplicate {
        name: String,
        first: String,
        second: String,
    },
    #[error("variable resolution failed: {0}")]
    BadVariable(String),
    #[error("bash injection failed: {0}")]
    BashFailed(String),
    #[error("invalid args: {0}")]
    InvalidArgs(String),
}
