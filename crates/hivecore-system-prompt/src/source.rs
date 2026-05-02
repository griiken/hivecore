//! `ContextSource` — pluggable section producer. Each source returns one
//! `SourceFragment` (or none, if it has nothing to contribute this turn).
//!
//! Order in which sources run is the builder's call. Per Codex's
//! prefix-preservation invariant, the same source set must produce
//! identical bytes turn-after-turn unless input state genuinely changed.

use async_trait::async_trait;

use crate::error::PromptError;

/// Whether the resulting markdown is intended to render as system,
/// developer, or user role when handed to the model. Mirrors Codex's
/// three-role layering.
///
/// Hivecore's `ModelRequest::system` is a single string today, so all
/// fragments are concatenated into the system prompt. The `RoleHint` is
/// preserved on the output for harnesses that target a richer wire format
/// later (e.g. Responses API directly).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoleHint {
    System,
    Developer,
    User,
}

#[derive(Debug, Clone)]
pub struct SourceFragment {
    /// Stable source name (used for cache invalidation + debug output).
    pub source: String,
    pub role_hint: RoleHint,
    /// The markdown body. Builder concatenates these with `\n\n` separators.
    pub body: String,
}

#[async_trait]
pub trait ContextSource: Send + Sync + std::fmt::Debug {
    /// Stable identifier — must NOT change for the same logical source
    /// across calls. Used for ordering tiebreaks and debug.
    fn name(&self) -> &str;

    /// Produce this source's contribution. Return `None` to skip.
    async fn fragment(&self) -> Result<Option<SourceFragment>, PromptError>;
}

/// Static text, supplied at construction time. Most common building block.
#[derive(Debug, Clone)]
pub struct StaticSource {
    name: String,
    role_hint: RoleHint,
    body: String,
}

impl StaticSource {
    pub fn new(name: impl Into<String>, role_hint: RoleHint, body: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            role_hint,
            body: body.into(),
        }
    }
}

#[async_trait]
impl ContextSource for StaticSource {
    fn name(&self) -> &str {
        &self.name
    }
    async fn fragment(&self) -> Result<Option<SourceFragment>, PromptError> {
        if self.body.is_empty() {
            return Ok(None);
        }
        Ok(Some(SourceFragment {
            source: self.name.clone(),
            role_hint: self.role_hint,
            body: self.body.clone(),
        }))
    }
}
