//! Error types for the MCP client.

use thiserror::Error;

pub type McpResult<T> = Result<T, McpError>;

#[derive(Debug, Error)]
pub enum McpError {
    #[error("config: {0}")]
    Config(String),

    #[error("server `{server}` not found in config")]
    UnknownServer { server: String },

    #[error("server `{server}` connect failed: {reason}")]
    ConnectFailed { server: String, reason: String },

    #[error("tool `{tool}` not found on server `{server}`")]
    UnknownTool { server: String, tool: String },

    #[error("tool `{tool}` on server `{server}` is disabled by allowlist")]
    DisabledByAllowlist { server: String, tool: String },

    #[error("tool `{tool}` on server `{server}` is denied by denylist")]
    DeniedByDenylist { server: String, tool: String },

    #[error("approval required: server=`{server}` tool=`{tool}`")]
    ApprovalRequired { server: String, tool: String },

    #[error("transport: {0}")]
    Transport(String),

    #[error("timeout after {ms}ms calling `{tool}` on `{server}`")]
    Timeout {
        server: String,
        tool: String,
        ms: u64,
    },

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("toml: {0}")]
    Toml(#[from] toml::de::Error),
}
