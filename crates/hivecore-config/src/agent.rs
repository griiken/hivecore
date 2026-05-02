//! `Agent` — the validated, in-memory shape downstream crates consume.
//! Mirrors Claude Code's subagent file format and Codex's agent definition:
//! a name, a system prompt, a model selection, a tool policy, and limits.
//! TOML-side counterparts (`raw::*`) live in `loader.rs`.

use serde::{Deserialize, Serialize};

/// Validated agent definition, ready to feed into `AgentLoop::builder()`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub model: ModelSelection,
    pub system_prompt: String,
    pub tools: ToolPolicy,
    pub limits: Limits,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelSelection {
    pub provider: String,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolPolicy {
    pub mode: ToolMode,
    pub list: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ToolMode {
    /// Only the names in `list` are exposed to the model.
    Allowlist,
    /// Every registered tool is exposed.
    All,
    /// Every tool *except* those in `list` is exposed.
    Denylist,
}

impl ToolPolicy {
    /// Decide whether `name` is permitted under this policy.
    pub fn allows(&self, name: &str) -> bool {
        match self.mode {
            ToolMode::All => true,
            ToolMode::Allowlist => self.list.iter().any(|n| n == name),
            ToolMode::Denylist => !self.list.iter().any(|n| n == name),
        }
    }

    /// Filter a registered tool name list down to what this agent accepts,
    /// preserving the input order.
    pub fn filter<'a, I>(&self, names: I) -> Vec<String>
    where
        I: IntoIterator<Item = &'a str>,
    {
        names
            .into_iter()
            .filter(|n| self.allows(n))
            .map(|s| s.to_string())
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Limits {
    pub max_iterations: u32,
}

#[cfg(test)]
#[path = "agent_tests.rs"]
mod tests;
