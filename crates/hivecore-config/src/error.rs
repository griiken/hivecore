use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("toml: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("validation: {0}")]
    Validation(#[from] ValidationError),
    #[error("agent not found: {0}")]
    NotFound(String),
    #[error("duplicate agent id: {0}")]
    Duplicate(String),
}

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("invalid id `{0}` — must match `^[a-z][a-z0-9_-]*$`")]
    InvalidId(String),
    #[error("system_prompt: must specify exactly one of `inline` or `path`")]
    PromptSourceAmbiguous,
    #[error("system_prompt path not found: {0}")]
    PromptPathMissing(String),
    #[error("tools.mode `{0}` invalid — expected one of allowlist | all | denylist")]
    InvalidToolMode(String),
    #[error("tools.list must be empty when mode = `all`")]
    ToolListWithAllMode,
    #[error("model.provider `{0}` unsupported in v0.1 — only `openai`")]
    UnsupportedProvider(String),
    #[error("model.id is required")]
    MissingModelId,
    #[error("limits.max_iterations must be >= 1 (got {0})")]
    BadIterationLimit(u32),
    #[error("role.prompt must be non-empty")]
    EmptyRolePrompt,
}
