//! `McpRiskAugmenter` — Layer-3 `RiskAugmenter` that maps cached MCP
//! `ToolAnnotations` into a `RiskHint`. Consumed by `ApprovalHook` to
//! short-circuit `UnlessTrusted` policy when the server has declared
//! `read_only_hint = true` and the server is trusted.
//!
//! Spec authority (verbatim): "clients MUST consider tool annotations to
//! be untrusted unless they come from trusted servers." Per-server trust
//! flag lives in `McpServerConfig.trust_annotations`.
//!
//! Goose convention (`crates/goose/src/permission/permission_inspector.rs:32-44`):
//! `read_only_hint = Some(true)` is **policy input**, not unilateral auto-allow.
//! The hook only auto-allows when policy is `UnlessTrusted` AND
//! `read_only == Some(true)` AND the server is trusted.

use std::sync::Arc;

use hivecore_runtime_core::{RiskAugmenter, RiskHint, ToolInvocation};

use crate::client::McpClient;

#[derive(Debug, Clone)]
pub struct McpRiskAugmenter {
    client: McpClient,
}

impl McpRiskAugmenter {
    pub fn new(client: McpClient) -> Self {
        Self { client }
    }

    /// Convenience for callers who want an `Arc<dyn RiskAugmenter>` directly.
    pub fn into_arc(client: McpClient) -> Arc<dyn RiskAugmenter> {
        Arc::new(Self::new(client))
    }

    /// Parse `mcp_call` invocation input → (server, tool). Returns `None`
    /// for any other tool name or malformed input.
    fn parse_mcp_call_args(inv: &ToolInvocation) -> Option<(String, String)> {
        if inv.name != "mcp_call" {
            return None;
        }
        let server = inv.input.get("server")?.as_str()?.to_string();
        let tool = inv.input.get("tool")?.as_str()?.to_string();
        Some((server, tool))
    }
}

impl RiskAugmenter for McpRiskAugmenter {
    fn augment(&self, inv: &ToolInvocation) -> RiskHint {
        let Some((server, tool)) = Self::parse_mcp_call_args(inv) else {
            return RiskHint::default();
        };

        // Spec MUST: untrusted unless server is trusted.
        let trust = self
            .client
            .config()
            .servers
            .get(&server)
            .map(|c| c.trust_annotations)
            .unwrap_or(false);
        if !trust {
            return RiskHint::default();
        }

        let Some(ann) = self.client.annotations().get(&server, &tool) else {
            // No annotation cached. Spec defaults: read_only=false,
            // destructive=true (when not read-only). Surface as worst case.
            return RiskHint {
                read_only: None,
                risky: None,
                network: None,
            };
        };

        // `read_only_hint = Some(true)` opts INTO auto-allow under
        // `UnlessTrusted`. `Some(false)` and `None` both fall through to
        // ask. Note rmcp's `is_destructive()` defaults to true when absent —
        // we mirror by surfacing `Some(true)` as a positive risk signal.
        let risky = match (ann.read_only_hint, ann.destructive_hint) {
            (Some(true), _) => Some(false), // confirmed safe-ish
            (_, Some(true)) => Some(true),  // explicitly destructive
            (Some(false), _) => Some(true), // explicitly write
            (None, None) => None,           // no signal
            (None, Some(false)) => Some(false),
        };

        RiskHint {
            read_only: ann.read_only_hint,
            risky,
            network: ann.open_world_hint,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{McpServerConfig, McpUserConfig};
    use hivecore_runtime_core::ToolCallId;
    use rmcp::model::ToolAnnotations;

    fn client_with_server(name: &str, trust: bool) -> McpClient {
        let mut cfg = McpUserConfig::default();
        cfg.servers.insert(
            name.to_string(),
            McpServerConfig {
                transport: Default::default(),
                command: Some("x".into()),
                args: vec![],
                env: Default::default(),
                url: None,
                enabled: true,
                startup_timeout_sec: 30,
                tool_timeout_sec: 60,
                default_approval_mode: Default::default(),
                trust_annotations: trust,
                enabled_tools: vec![],
                disabled_tools: vec![],
                tools: Default::default(),
            },
        );
        McpClient::new(cfg)
    }

    fn invocation(name: &str, args: serde_json::Value) -> ToolInvocation {
        ToolInvocation {
            id: ToolCallId("c1".into()),
            name: name.into(),
            input: args,
        }
    }

    #[test]
    fn non_mcp_tool_returns_default() {
        let client = client_with_server("git", true);
        let aug = McpRiskAugmenter { client };
        let r = aug.augment(&invocation("bash", serde_json::json!({"command": "ls"})));
        assert!(r.read_only.is_none());
    }

    #[test]
    fn untrusted_server_returns_default_even_with_annotation() {
        let client = client_with_server("untrusted", false);
        client.annotations().insert(
            "untrusted",
            "read_x",
            ToolAnnotations {
                read_only_hint: Some(true),
                ..Default::default()
            },
        );
        let aug = McpRiskAugmenter { client };
        let r = aug.augment(&invocation(
            "mcp_call",
            serde_json::json!({"server": "untrusted", "tool": "read_x"}),
        ));
        assert!(
            r.read_only.is_none(),
            "untrusted server: annotations must be ignored"
        );
    }

    #[test]
    fn trusted_read_only_surfaces() {
        let client = client_with_server("git", true);
        client.annotations().insert(
            "git",
            "git_status",
            ToolAnnotations {
                read_only_hint: Some(true),
                ..Default::default()
            },
        );
        let aug = McpRiskAugmenter { client };
        let r = aug.augment(&invocation(
            "mcp_call",
            serde_json::json!({"server": "git", "tool": "git_status"}),
        ));
        assert_eq!(r.read_only, Some(true));
        assert_eq!(r.risky, Some(false));
    }

    #[test]
    fn trusted_destructive_surfaces_risky() {
        let client = client_with_server("fs", true);
        client.annotations().insert(
            "fs",
            "write_file",
            ToolAnnotations {
                read_only_hint: Some(false),
                destructive_hint: Some(true),
                idempotent_hint: Some(true),
                ..Default::default()
            },
        );
        let aug = McpRiskAugmenter { client };
        let r = aug.augment(&invocation(
            "mcp_call",
            serde_json::json!({"server": "fs", "tool": "write_file"}),
        ));
        assert_eq!(r.read_only, Some(false));
        assert_eq!(r.risky, Some(true));
    }

    #[test]
    fn missing_annotation_returns_unknown_signal() {
        let client = client_with_server("git", true);
        let aug = McpRiskAugmenter { client };
        let r = aug.augment(&invocation(
            "mcp_call",
            serde_json::json!({"server": "git", "tool": "never_listed"}),
        ));
        assert!(r.read_only.is_none(), "absent annotation = unknown");
    }

    #[test]
    fn malformed_args_returns_default() {
        let client = client_with_server("git", true);
        let aug = McpRiskAugmenter { client };
        let r = aug.augment(&invocation("mcp_call", serde_json::json!({"foo": "bar"})));
        assert!(r.read_only.is_none());
    }
}
