//! TOML config schema for the MCP client (ADR-027 §10).
//!
//! Read order at startup (highest precedence first — process per-key merge):
//!   1. `<workspace>/.hivecore/mcp.toml`
//!   2. `~/.hivecore/mcp.toml`
//!
//! JSON form (`~/.hivecore/mcp.json`) accepted as a Claude-Code-style import.
//! First-run import from `~/.codex/config.toml [mcp_servers.*]` and
//! `~/.claude.json` is best-effort (jcode pattern).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::error::{McpError, McpResult};

/// Top-level user config. Maps server-name → server settings.
#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct McpUserConfig {
    #[serde(default, rename = "mcp_servers")]
    pub servers: HashMap<String, McpServerConfig>,
}

/// Per-server config.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpServerConfig {
    #[serde(default)]
    pub transport: TransportKind,

    /// Stdio: command to spawn (e.g. `npx`, `uvx`, absolute path).
    pub command: Option<String>,

    /// Stdio: args passed to the command.
    #[serde(default)]
    pub args: Vec<String>,

    /// Stdio: extra env vars to set on the child. Values may include
    /// `${env:VAR}` placeholders — resolution happens at connect time.
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// HTTP / SSE / WS: base URL.
    pub url: Option<String>,

    /// Whether the server is wired at all. Defaults to `true`.
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Connect attempt timeout (seconds). Default 30.
    #[serde(default = "default_startup_timeout_sec")]
    pub startup_timeout_sec: u64,

    /// Per-tool-call timeout (seconds). Default 60.
    #[serde(default = "default_tool_timeout_sec")]
    pub tool_timeout_sec: u64,

    /// Default approval mode applied to every tool unless overridden.
    #[serde(default)]
    pub default_approval_mode: ApprovalMode,

    /// Whether to honour `ToolAnnotations.read_only_hint` from this server.
    /// Per MCP spec normative MUST: clients MUST consider tool annotations
    /// untrusted unless they come from trusted servers. Default `true` for
    /// v0.1; verticals deploying untrusted-third-party servers should flip
    /// to `false` per-server.
    #[serde(default = "default_trust_annotations")]
    pub trust_annotations: bool,

    /// Allowlist of raw server-native tool names. Empty = no allowlist.
    #[serde(default)]
    pub enabled_tools: Vec<String>,

    /// Denylist of raw server-native tool names.
    #[serde(default)]
    pub disabled_tools: Vec<String>,

    /// Per-tool overrides keyed by raw server-native name.
    #[serde(default)]
    pub tools: HashMap<String, McpToolConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpToolConfig {
    pub approval_mode: Option<ApprovalMode>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportKind {
    #[default]
    Stdio,
    Http,
    Sse,
    Ws,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalMode {
    /// Always require user approval before invocation.
    Always,
    /// Never require approval — silently invoke.
    Never,
    /// Require approval the first time `(tenant, server, tool)` is called;
    /// remember the user's choice (audit-logged, durable). Default.
    #[default]
    OnRequest,
}

fn default_enabled() -> bool {
    true
}
fn default_startup_timeout_sec() -> u64 {
    30
}
fn default_tool_timeout_sec() -> u64 {
    60
}
fn default_trust_annotations() -> bool {
    true
}

impl McpUserConfig {
    /// Parse a config from a TOML string.
    pub fn from_toml_str(s: &str) -> McpResult<Self> {
        toml::from_str(s).map_err(McpError::from)
    }

    /// Read a TOML file. Returns `Ok(default)` if the file does not exist.
    pub fn from_path(path: &Path) -> McpResult<Self> {
        match std::fs::read_to_string(path) {
            Ok(s) => Self::from_toml_str(&s),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(McpError::Io(e)),
        }
    }

    /// True if any server is registered.
    pub fn has_any(&self) -> bool {
        !self.servers.is_empty()
    }

    /// Number of enabled servers.
    pub fn enabled_count(&self) -> usize {
        self.servers.values().filter(|s| s.enabled).count()
    }

    /// Per-key merge: `other` values override `self` for matching server names;
    /// new server names are appended. Used to layer workspace-local config on
    /// top of user-global config.
    pub fn merge(&mut self, other: McpUserConfig) {
        for (name, cfg) in other.servers {
            self.servers.insert(name, cfg);
        }
    }
}

impl McpServerConfig {
    /// Apply the allow / deny lists to a raw server-native tool name.
    /// Returns `Ok(())` if the tool is permitted; otherwise the matching
    /// `McpError`.
    pub fn check_tool_allowed(&self, server: &str, tool: &str) -> McpResult<()> {
        if !self.disabled_tools.iter().any(|t| t == tool) {
            // not denied
        } else {
            return Err(McpError::DeniedByDenylist {
                server: server.to_string(),
                tool: tool.to_string(),
            });
        }
        if self.enabled_tools.is_empty() || self.enabled_tools.iter().any(|t| t == tool) {
            Ok(())
        } else {
            Err(McpError::DisabledByAllowlist {
                server: server.to_string(),
                tool: tool.to_string(),
            })
        }
    }

    /// Effective approval mode for a single tool — per-tool override beats
    /// the server default.
    pub fn effective_approval_mode(&self, tool: &str) -> ApprovalMode {
        self.tools
            .get(tool)
            .and_then(|t| t.approval_mode)
            .unwrap_or(self.default_approval_mode)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_stdio() {
        let toml_src = r#"
            [mcp_servers.github]
            command = "npx"
            args = ["-y", "@modelcontextprotocol/server-github"]
        "#;
        let cfg = McpUserConfig::from_toml_str(toml_src).unwrap();
        let s = cfg.servers.get("github").unwrap();
        assert_eq!(s.command.as_deref(), Some("npx"));
        assert_eq!(s.args, vec!["-y", "@modelcontextprotocol/server-github"]);
        assert!(s.enabled);
        assert_eq!(s.transport, TransportKind::Stdio);
        assert_eq!(s.default_approval_mode, ApprovalMode::OnRequest);
    }

    #[test]
    fn parse_per_tool_override() {
        let toml_src = r#"
            [mcp_servers.github]
            command = "npx"
            args = []

            [mcp_servers.github.tools.create_issue]
            approval_mode = "always"
        "#;
        let cfg = McpUserConfig::from_toml_str(toml_src).unwrap();
        let s = cfg.servers.get("github").unwrap();
        assert_eq!(
            s.effective_approval_mode("create_issue"),
            ApprovalMode::Always
        );
        assert_eq!(
            s.effective_approval_mode("list_repos"),
            ApprovalMode::OnRequest
        );
    }

    #[test]
    fn allowlist_and_denylist() {
        let toml_src = r#"
            [mcp_servers.github]
            command = "x"
            enabled_tools = ["create_issue", "list_repos"]
            disabled_tools = ["delete_repo"]
        "#;
        let cfg = McpUserConfig::from_toml_str(toml_src).unwrap();
        let s = cfg.servers.get("github").unwrap();
        assert!(s.check_tool_allowed("github", "create_issue").is_ok());
        assert!(matches!(
            s.check_tool_allowed("github", "merge_pr"),
            Err(McpError::DisabledByAllowlist { .. })
        ));
        assert!(matches!(
            s.check_tool_allowed("github", "delete_repo"),
            Err(McpError::DeniedByDenylist { .. })
        ));
    }

    #[test]
    fn merge_layers_workspace_over_user() {
        let mut user = McpUserConfig::from_toml_str(
            r#"
            [mcp_servers.github]
            command = "npx-user"
        "#,
        )
        .unwrap();
        let workspace = McpUserConfig::from_toml_str(
            r#"
            [mcp_servers.github]
            command = "npx-ws"

            [mcp_servers.linear]
            command = "uvx"
        "#,
        )
        .unwrap();
        user.merge(workspace);
        assert_eq!(
            user.servers.get("github").unwrap().command.as_deref(),
            Some("npx-ws")
        );
        assert!(user.servers.contains_key("linear"));
    }

    #[test]
    fn missing_path_returns_default() {
        let cfg = McpUserConfig::from_path(Path::new("/nonexistent/mcp.toml")).unwrap();
        assert!(!cfg.has_any());
    }
}
