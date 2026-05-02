//! In-memory session-scoped approval cache.
//!
//! Keyed by `tool_name` alone. When the user picks `ApprovedForSession`,
//! the tool name lands here; every subsequent matched call to that tool
//! short-circuits to `Pass` regardless of arguments.
//!
//! This matches Codex MCP (`mcp_tool_call.rs:1196-1200` — `McpToolApprovalKey
//! { server, connector_id, tool_name }`, args NOT in key), Goose
//! (`permission_inspector.rs:32-44` — tool-name set), and Cline
//! (per-category toggle). Hashing the input was a v0.1 over-design —
//! verified against four prior systems on 2026-05-03.
//!
//! Argument-glob precision (Continue.dev / Claude Code shape —
//! `Bash(npm test:*)`) is the v0.2 B3 backlog item. Persistent
//! (`ApprovedAndPersist`) cache is v0.2 — needs the audit plane (ADR-019).
//!
//! `input` is still accepted in the API for forward-compat with B3.

use std::collections::HashSet;

use parking_lot::Mutex;

#[derive(Debug, Default)]
pub struct SessionCache {
    inner: Mutex<HashSet<String>>,
}

impl SessionCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn contains(&self, tool_name: &str, _input: &serde_json::Value) -> bool {
        self.inner.lock().contains(tool_name)
    }

    pub fn insert(&self, tool_name: &str, _input: &serde_json::Value) {
        self.inner.lock().insert(tool_name.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approved_tool_name_matches_any_input() {
        let c = SessionCache::new();
        c.insert("edit_file", &serde_json::json!({"path": "auth.rs"}));
        // Codex / Goose convention: different args, same tool → cache hit.
        assert!(c.contains("edit_file", &serde_json::json!({"path": "handlers.rs"})));
        assert!(c.contains(
            "edit_file",
            &serde_json::json!({"path": "lib.rs", "old": "x"})
        ));
    }

    #[test]
    fn unapproved_tool_does_not_match() {
        let c = SessionCache::new();
        c.insert("edit_file", &serde_json::json!({}));
        assert!(!c.contains("bash", &serde_json::json!({"command": "ls"})));
    }
}
