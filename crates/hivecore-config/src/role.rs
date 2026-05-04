//! `Role` — call/session/agent overlay above `Agent` (ADR-033).
//!
//! A `Role` is a system-prompt fragment plus optional tool/model overrides.
//! It overlays an `Agent` at three scopes — **call**, **session**, or
//! **agent default** — with precedence `call > session > agent`. Roles are
//! TOML-declared, discovered the same way agents are, and applied as a
//! system-prompt overlay (never injected into persisted user history).
//!
//! An `Agent` is "what you are"; a `Role` is "how you're acting right now".
//! Roles compose across agents — one researcher role can apply to coder,
//! reviewer, designer.

use serde::{Deserialize, Serialize};

/// Validated role definition. `prompt` is the system-prompt fragment to
/// append after the agent's base prompt; `tools` (if present) replaces the
/// agent's tool list at this overlay's scope; `model` (if present) takes
/// precedence over the agent's model selection at this overlay's scope.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Role {
    pub name: String,
    pub description: String,
    pub prompt: String,
    /// Optional tool allowlist override. Absent ⇒ inherit agent's tools.
    /// Present (even empty) ⇒ replace agent's tools at this scope.
    #[serde(default)]
    pub tools: Option<Vec<String>>,
    /// Optional model override (`provider/id`).
    #[serde(default)]
    pub model: Option<String>,
}

impl Role {
    /// Resolve overlay precedence `call > session > agent`. The first
    /// `Some(_)` wins; agent default fills in for `None`s.
    pub fn resolve(
        call: Option<&Role>,
        session: Option<&Role>,
        agent_default: Option<&Role>,
    ) -> Option<Role> {
        call.or(session).or(agent_default).cloned()
    }
}

#[cfg(test)]
#[path = "role_tests.rs"]
mod tests;
