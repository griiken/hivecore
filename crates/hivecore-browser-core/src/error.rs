use thiserror::Error;

pub type BrowserResult<T> = Result<T, BrowserError>;

#[derive(Debug, Error)]
pub enum BrowserError {
    #[error("session '{0}' not found")]
    SessionNotFound(String),

    #[error("cross-tenant access denied: caller tenant '{caller}' tried to use session belonging to '{owner}'")]
    CrossTenantDenied { caller: String, owner: String },

    /// Element ref does not match the latest snapshot for its session.
    /// Caller must re-snapshot before retrying. ADR-028 invariant.
    #[error("stale ref '{0}': re-snapshot required (snapshot version advanced)")]
    StaleRef(String),

    #[error("element '{0}' not actionable in current state")]
    NotActionable(String),

    #[error("navigation failed: {0}")]
    Navigation(String),

    #[error("transport: {0}")]
    Transport(String),

    #[error("capability '{0}' not supported by this provider")]
    CapabilityMissing(String),

    #[error("artifact path escapes tenant root: {0}")]
    ArtifactPathEscape(String),

    #[error("provider rejected: {0}")]
    Rejected(String),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("other: {0}")]
    Other(String),
}
