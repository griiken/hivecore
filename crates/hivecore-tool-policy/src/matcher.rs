//! `ToolMatcher` — classifier returning a 3-state outcome:
//!
//! - `Allow`        — matcher says safe; no approval needed.
//! - `Ask(reason)`  — needs human approval; reason surfaced to UI.
//! - `Deny(reason)` — hard-denied without prompting (Claude Code `deny:` /
//!   Goose `never_allow` pattern). Avoids alarm fatigue on tools that the
//!   policy author has already decided are off-limits.
//!
//! v0.1: simple per-tool-name classifier. v0.2 will add glob/regex on
//! arguments (Continue.dev / Claude Code shape) and `RiskHint`-driven rules.

use std::collections::HashMap;
use std::fmt;

use hivecore_runtime_core::ToolInvocation;

/// 3-state matcher result.
#[derive(Debug, Clone)]
pub enum MatchOutcome {
    /// No approval needed.
    Allow,
    /// Needs approval; `reason` surfaced to the user.
    Ask(String),
    /// Hard-denied without prompting; `reason` surfaced to the model.
    Deny(String),
}

pub trait ToolMatcher: Send + Sync + fmt::Debug {
    fn classify(&self, inv: &ToolInvocation) -> MatchOutcome;
}

/// Per-tool-name table: each tool maps to an explicit outcome
/// (`Ask(reason)` or `Deny(reason)`). Unmapped tools default to `Allow`.
#[derive(Debug, Default, Clone)]
pub struct ToolNameMatcher {
    rules: HashMap<String, MatchOutcome>,
}

impl ToolNameMatcher {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a rule: matched calls go to the human (`Ask`).
    pub fn ask(mut self, tool_name: impl Into<String>, reason: impl Into<String>) -> Self {
        self.rules
            .insert(tool_name.into(), MatchOutcome::Ask(reason.into()));
        self
    }

    /// Add a rule: matched calls are hard-denied without prompting (`Deny`).
    pub fn deny(mut self, tool_name: impl Into<String>, reason: impl Into<String>) -> Self {
        self.rules
            .insert(tool_name.into(), MatchOutcome::Deny(reason.into()));
        self
    }

    /// Default coding-agent risky set: shell exec + filesystem mutation
    /// surface a prompt; no hard-deny rules in the v0.1 default set.
    pub fn coder_defaults() -> Self {
        Self::new()
            .ask("bash", "shell command execution")
            .ask("write_file", "filesystem write")
            .ask("edit_file", "filesystem edit")
    }
}

impl ToolMatcher for ToolNameMatcher {
    fn classify(&self, inv: &ToolInvocation) -> MatchOutcome {
        self.rules
            .get(&inv.name)
            .cloned()
            .unwrap_or(MatchOutcome::Allow)
    }
}
