//! Three meta-tools (ADR-027 §4 lazy 3-meta-tool default).
//!
//! - `mcp_servers` — list servers from config (no connection).
//! - `mcp_discover(server)` — connect + return server-native tool schemas.
//! - `mcp_call(server, tool, args)` — connect + invoke tool by raw name.
//!
//! Token cost is fixed at 3 tool descriptions regardless of server count.

use std::sync::Arc;

use async_trait::async_trait;
use hivecore_runtime_core::{
    AbortSignal, ContentBlock, RuntimeResult, Tool, ToolExecutionMode, ToolInvocation, ToolOutcome,
    UpdateSink,
};
use serde::Deserialize;

use crate::client::{call_raw_tool, list_raw_tools, McpClient};

/// `mcp_servers` — lists registered server names + transport + enabled state.
/// No connection is opened. Cheap; safe to call repeatedly.
#[derive(Debug, Clone)]
pub struct McpServersTool {
    client: McpClient,
}

impl McpServersTool {
    pub fn new(client: McpClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Tool for McpServersTool {
    fn name(&self) -> &str {
        "mcp_servers"
    }

    fn description(&self) -> &str {
        "List configured MCP servers (no connection opened). Returns name, transport, and enabled state for each server. Use `mcp_discover` to fetch a server's tool schemas."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        })
    }

    async fn execute(
        &self,
        _invocation: ToolInvocation,
        _signal: AbortSignal,
        _on_update: UpdateSink,
    ) -> RuntimeResult<ToolOutcome> {
        let mut entries: Vec<_> = self
            .client
            .config()
            .servers
            .iter()
            .map(|(name, cfg)| {
                serde_json::json!({
                    "name": name,
                    "transport": format!("{:?}", cfg.transport).to_lowercase(),
                    "enabled": cfg.enabled,
                })
            })
            .collect();
        entries.sort_by(|a, b| {
            a.get("name")
                .and_then(|v| v.as_str())
                .cmp(&b.get("name").and_then(|v| v.as_str()))
        });
        let body = serde_json::json!({ "servers": entries });
        Ok(ToolOutcome::ok(vec![ContentBlock::Text {
            text: body.to_string(),
        }]))
    }
}

/// `mcp_discover` — connect to a named server and list its native tools.
#[derive(Debug, Clone)]
pub struct McpDiscoverTool {
    client: McpClient,
}

impl McpDiscoverTool {
    pub fn new(client: McpClient) -> Self {
        Self { client }
    }
}

#[derive(Debug, Deserialize)]
struct DiscoverArgs {
    server: String,
}

#[async_trait]
impl Tool for McpDiscoverTool {
    fn name(&self) -> &str {
        "mcp_discover"
    }

    fn description(&self) -> &str {
        "Connect to a named MCP server (lazy on first call) and return its tool schemas. Use `mcp_servers` first to see available server names, then `mcp_call` to invoke a discovered tool."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "server": { "type": "string", "description": "configured server name" }
            },
            "required": ["server"],
            "additionalProperties": false
        })
    }

    async fn execute(
        &self,
        invocation: ToolInvocation,
        _signal: AbortSignal,
        _on_update: UpdateSink,
    ) -> RuntimeResult<ToolOutcome> {
        let args: DiscoverArgs = match serde_json::from_value(invocation.input) {
            Ok(a) => a,
            Err(e) => return Ok(ToolOutcome::error(format!("invalid args: {e}"))),
        };
        match list_raw_tools(&self.client, &args.server).await {
            Ok(v) => Ok(ToolOutcome::ok(vec![ContentBlock::Text {
                text: v.to_string(),
            }])),
            Err(e) => Ok(ToolOutcome::error(e.to_string())),
        }
    }
}

/// `mcp_call` — invoke a server-native tool. Allow / deny lists are enforced
/// before dispatch.
#[derive(Debug, Clone)]
pub struct McpCallTool {
    client: McpClient,
}

impl McpCallTool {
    pub fn new(client: McpClient) -> Self {
        Self { client }
    }
}

#[derive(Debug, Deserialize)]
struct CallArgs {
    server: String,
    tool: String,
    #[serde(default)]
    arguments: serde_json::Value,
}

#[async_trait]
impl Tool for McpCallTool {
    fn name(&self) -> &str {
        "mcp_call"
    }

    fn execution_mode(&self) -> ToolExecutionMode {
        // Default fallback when the per-invocation lookup can't classify
        // (unparseable args, unknown server, missing annotations). Safe
        // default per the MCP spec normative MUST.
        ToolExecutionMode::Sequential
    }

    fn execution_mode_for(&self, invocation: &ToolInvocation) -> ToolExecutionMode {
        // ADR-034 + ADR-029 A3 — dynamic dispatch. If the configured
        // server has `trust_annotations: true` (default) AND the cached
        // `ToolAnnotations.read_only_hint` is `Some(true)` for this
        // tool, dispatch in parallel. Anything else (untrusted server,
        // missing cache entry, `Some(false)`, parse failure) falls
        // through to Sequential — Goose convention preserved verbatim.
        let Ok(args) = serde_json::from_value::<CallArgs>(invocation.input.clone()) else {
            return ToolExecutionMode::Sequential;
        };
        let Ok(server_cfg) = self.client.server_config(&args.server) else {
            return ToolExecutionMode::Sequential;
        };
        if !server_cfg.trust_annotations {
            return ToolExecutionMode::Sequential;
        }
        let Some(ann) = self.client.annotations().get(&args.server, &args.tool) else {
            return ToolExecutionMode::Sequential;
        };
        if ann.read_only_hint == Some(true) {
            ToolExecutionMode::Parallel
        } else {
            ToolExecutionMode::Sequential
        }
    }

    fn description(&self) -> &str {
        "Invoke a tool on a named MCP server. `tool` is the server-native name as returned by `mcp_discover`. `arguments` is a JSON object matching the tool's input schema."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "server":    { "type": "string", "description": "configured server name" },
                "tool":      { "type": "string", "description": "server-native tool name (raw, not qualified)" },
                "arguments": { "type": "object", "description": "JSON object matching the tool's input schema" }
            },
            "required": ["server", "tool"],
            "additionalProperties": false
        })
    }

    async fn execute(
        &self,
        invocation: ToolInvocation,
        _signal: AbortSignal,
        _on_update: UpdateSink,
    ) -> RuntimeResult<ToolOutcome> {
        let args: CallArgs = match serde_json::from_value(invocation.input) {
            Ok(a) => a,
            Err(e) => return Ok(ToolOutcome::error(format!("invalid args: {e}"))),
        };
        match call_raw_tool(&self.client, &args.server, &args.tool, args.arguments).await {
            Ok(v) => Ok(ToolOutcome::ok(vec![ContentBlock::Text {
                text: v.to_string(),
            }])),
            Err(e) => Ok(ToolOutcome::error(e.to_string())),
        }
    }
}

/// Build the standard 3-tool registry from a shared client.
pub fn default_meta_tools(client: McpClient) -> Vec<Arc<dyn Tool>> {
    vec![
        Arc::new(McpServersTool::new(client.clone())),
        Arc::new(McpDiscoverTool::new(client.clone())),
        Arc::new(McpCallTool::new(client)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::McpUserConfig;
    use hivecore_runtime_core::ToolCallId;

    fn invoke(name: &str, input: serde_json::Value) -> ToolInvocation {
        ToolInvocation {
            id: ToolCallId(format!("test-{name}")),
            name: name.to_string(),
            input,
        }
    }

    #[tokio::test]
    async fn mcp_servers_lists_config() {
        let cfg = McpUserConfig::from_toml_str(
            r#"
            [mcp_servers.github]
            command = "npx"

            [mcp_servers.linear]
            command = "uvx"
            enabled = false
        "#,
        )
        .unwrap();
        let tool = McpServersTool::new(McpClient::new(cfg));
        let out = tool
            .execute(
                invoke("mcp_servers", serde_json::json!({})),
                AbortSignal::new().1,
                UpdateSink::noop(),
            )
            .await
            .unwrap();
        assert!(!out.is_error);
        let body = match &out.content[0] {
            ContentBlock::Text { text } => text.clone(),
            _ => panic!("expected text"),
        };
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        let servers = parsed["servers"].as_array().unwrap();
        assert_eq!(servers.len(), 2);
        // sorted by name
        assert_eq!(servers[0]["name"], "github");
        assert_eq!(servers[1]["name"], "linear");
        assert_eq!(servers[1]["enabled"], false);
    }

    #[tokio::test]
    async fn mcp_discover_unknown_server_errors_gracefully() {
        let tool = McpDiscoverTool::new(McpClient::new(McpUserConfig::default()));
        let out = tool
            .execute(
                invoke("mcp_discover", serde_json::json!({ "server": "ghost" })),
                AbortSignal::new().1,
                UpdateSink::noop(),
            )
            .await
            .unwrap();
        assert!(out.is_error);
    }

    #[tokio::test]
    async fn mcp_call_invalid_args_returns_tool_error_not_runtime_error() {
        let tool = McpCallTool::new(McpClient::new(McpUserConfig::default()));
        let out = tool
            .execute(
                invoke("mcp_call", serde_json::json!({ "server": "x" })), // missing `tool`
                AbortSignal::new().1,
                UpdateSink::noop(),
            )
            .await
            .unwrap();
        assert!(out.is_error);
    }

    // ADR-034 + ADR-029 A3 — dynamic execution-mode dispatch for mcp_call.

    fn read_only_annotations() -> rmcp::model::ToolAnnotations {
        rmcp::model::ToolAnnotations {
            read_only_hint: Some(true),
            ..Default::default()
        }
    }

    fn destructive_annotations() -> rmcp::model::ToolAnnotations {
        rmcp::model::ToolAnnotations {
            read_only_hint: Some(false),
            destructive_hint: Some(true),
            ..Default::default()
        }
    }

    fn cfg_with_server(name: &str) -> McpUserConfig {
        McpUserConfig::from_toml_str(&format!(
            r#"
            [mcp_servers.{name}]
            command = "noop"
        "#
        ))
        .unwrap()
    }

    fn call_invocation(server: &str, tool: &str) -> ToolInvocation {
        invoke(
            "mcp_call",
            serde_json::json!({"server": server, "tool": tool, "arguments": {}}),
        )
    }

    #[tokio::test]
    async fn mcp_call_read_only_annotation_dispatches_parallel() {
        let cfg = cfg_with_server("github");
        let client = McpClient::new(cfg);
        client
            .annotations()
            .insert("github", "search_issues", read_only_annotations());
        let tool = McpCallTool::new(client);
        assert_eq!(
            tool.execution_mode_for(&call_invocation("github", "search_issues")),
            ToolExecutionMode::Parallel
        );
    }

    #[tokio::test]
    async fn mcp_call_destructive_annotation_dispatches_sequential() {
        let cfg = cfg_with_server("github");
        let client = McpClient::new(cfg);
        client
            .annotations()
            .insert("github", "delete_issue", destructive_annotations());
        let tool = McpCallTool::new(client);
        assert_eq!(
            tool.execution_mode_for(&call_invocation("github", "delete_issue")),
            ToolExecutionMode::Sequential
        );
    }

    #[tokio::test]
    async fn mcp_call_missing_annotation_falls_back_sequential() {
        let cfg = cfg_with_server("github");
        let tool = McpCallTool::new(McpClient::new(cfg));
        assert_eq!(
            tool.execution_mode_for(&call_invocation("github", "uncached_tool")),
            ToolExecutionMode::Sequential
        );
    }

    #[tokio::test]
    async fn mcp_call_untrusted_server_falls_back_sequential() {
        let cfg = McpUserConfig::from_toml_str(
            r#"
            [mcp_servers.untrusted]
            command = "noop"
            trust_annotations = false
        "#,
        )
        .unwrap();
        let client = McpClient::new(cfg);
        client
            .annotations()
            .insert("untrusted", "tool", read_only_annotations());
        let tool = McpCallTool::new(client);
        // Even with read_only=true in cache, untrusted server ⇒ Sequential.
        assert_eq!(
            tool.execution_mode_for(&call_invocation("untrusted", "tool")),
            ToolExecutionMode::Sequential
        );
    }

    #[tokio::test]
    async fn mcp_call_unknown_server_falls_back_sequential() {
        let tool = McpCallTool::new(McpClient::new(McpUserConfig::default()));
        assert_eq!(
            tool.execution_mode_for(&call_invocation("ghost", "any")),
            ToolExecutionMode::Sequential
        );
    }

    #[test]
    fn default_meta_tools_returns_three_named() {
        let client = McpClient::new(McpUserConfig::default());
        let tools = default_meta_tools(client);
        let names: Vec<_> = tools.iter().map(|t| t.name().to_string()).collect();
        assert_eq!(names, vec!["mcp_servers", "mcp_discover", "mcp_call"]);
    }
}
