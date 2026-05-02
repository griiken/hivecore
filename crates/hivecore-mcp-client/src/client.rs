//! MCP client wiring (ADR-027 §4 default lazy mode).
//!
//! Single-tenant for v0.2 — all calls share `tenant_id = "default"`. Per-(tenant,
//! server) connection cache keyed by `(String, String)`. Stdio transport only;
//! HTTP / SSE / WS deferred until config schema demand surfaces.

use std::collections::HashMap;
use std::sync::Arc;

use rmcp::model::{CallToolRequestParam, ToolAnnotations};
use rmcp::service::{RoleClient, RunningService};
use rmcp::transport::child_process::{ConfigureCommandExt, TokioChildProcess};
use rmcp::ServiceExt;
use tokio::process::Command;
use tokio::sync::Mutex;

use crate::config::{McpServerConfig, McpUserConfig, TransportKind};
use crate::error::{McpError, McpResult};

/// `(server_name, raw_tool_name) → annotations` cache. Populated as a side
/// effect of `list_raw_tools`. Read by `McpRiskAugmenter` (sync) so the type
/// uses `parking_lot::RwLock` not tokio's async lock.
pub type AnnotationKey = (String, String);
#[derive(Default, Debug)]
pub struct AnnotationCache {
    inner: parking_lot::RwLock<HashMap<AnnotationKey, ToolAnnotations>>,
}

impl AnnotationCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, server: &str, tool: &str) -> Option<ToolAnnotations> {
        self.inner
            .read()
            .get(&(server.to_string(), tool.to_string()))
            .cloned()
    }

    pub fn insert(&self, server: &str, tool: &str, ann: ToolAnnotations) {
        self.inner
            .write()
            .insert((server.to_string(), tool.to_string()), ann);
    }

    pub fn len(&self) -> usize {
        self.inner.read().len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.read().is_empty()
    }
}

pub type Connection = Arc<RunningService<RoleClient, ()>>;

/// Connection cache keyed by `(tenant_id, server_name)`. v0.2 uses a single
/// `tenant_id = "default"`; the cache key carries it so v0.3 multi-tenant
/// adoption is a no-op for callers.
#[derive(Default)]
pub struct ConnectionCache {
    inner: Mutex<HashMap<(String, String), Connection>>,
}

impl ConnectionCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn get(&self, tenant: &str, server: &str) -> Option<Connection> {
        self.inner
            .lock()
            .await
            .get(&(tenant.to_string(), server.to_string()))
            .cloned()
    }

    pub async fn insert(&self, tenant: &str, server: &str, conn: Connection) {
        self.inner
            .lock()
            .await
            .insert((tenant.to_string(), server.to_string()), conn);
    }
}

impl std::fmt::Debug for ConnectionCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectionCache").finish_non_exhaustive()
    }
}

/// MCP client — owns config + connection cache. Cheap to clone (`Arc` inside).
#[derive(Clone, Debug)]
pub struct McpClient {
    config: Arc<McpUserConfig>,
    cache: Arc<ConnectionCache>,
    annotations: Arc<AnnotationCache>,
    tenant_id: String,
}

impl McpClient {
    pub fn new(config: McpUserConfig) -> Self {
        Self {
            config: Arc::new(config),
            cache: Arc::new(ConnectionCache::new()),
            annotations: Arc::new(AnnotationCache::new()),
            tenant_id: "default".to_string(),
        }
    }

    /// Annotation cache — populated as a side effect of `list_raw_tools`.
    /// Sync read; safe to call from non-async contexts (Layer-1 `RiskAugmenter`).
    pub fn annotations(&self) -> &Arc<AnnotationCache> {
        &self.annotations
    }

    pub fn with_tenant(mut self, tenant_id: impl Into<String>) -> Self {
        self.tenant_id = tenant_id.into();
        self
    }

    pub fn config(&self) -> &McpUserConfig {
        &self.config
    }

    pub fn tenant_id(&self) -> &str {
        &self.tenant_id
    }

    /// Look up server config or fail with `UnknownServer`.
    pub fn server_config(&self, server: &str) -> McpResult<&McpServerConfig> {
        self.config
            .servers
            .get(server)
            .ok_or_else(|| McpError::UnknownServer {
                server: server.to_string(),
            })
    }

    /// Get-or-connect for the named server. Subsequent calls return the cached
    /// connection. Stdio transport only in v0.2.
    pub async fn get_or_connect(&self, server: &str) -> McpResult<Connection> {
        if let Some(c) = self.cache.get(&self.tenant_id, server).await {
            return Ok(c);
        }
        let cfg = self.server_config(server)?.clone();
        if !cfg.enabled {
            return Err(McpError::ConnectFailed {
                server: server.to_string(),
                reason: "server is disabled in config".into(),
            });
        }
        let conn = match cfg.transport {
            TransportKind::Stdio => connect_stdio(server, &cfg).await?,
            TransportKind::Http | TransportKind::Sse | TransportKind::Ws => {
                return Err(McpError::ConnectFailed {
                    server: server.to_string(),
                    reason: format!(
                        "transport {:?} not supported in v0.2 (stdio only)",
                        cfg.transport
                    ),
                })
            }
        };
        let conn = Arc::new(conn);
        self.cache
            .insert(&self.tenant_id, server, conn.clone())
            .await;
        Ok(conn)
    }
}

async fn connect_stdio(
    server: &str,
    cfg: &McpServerConfig,
) -> McpResult<RunningService<RoleClient, ()>> {
    let command_path = cfg.command.as_deref().ok_or_else(|| {
        McpError::Config(format!(
            "server `{server}`: stdio transport requires `command`"
        ))
    })?;

    let resolved_env = resolve_env(&cfg.env)?;
    let args = cfg.args.clone();
    let cmd = Command::new(command_path).configure(|c| {
        c.args(&args);
        for (k, v) in &resolved_env {
            c.env(k, v);
        }
    });

    let transport = TokioChildProcess::new(cmd).map_err(|e| McpError::ConnectFailed {
        server: server.to_string(),
        reason: format!("spawn failed: {e}"),
    })?;

    let connect_fut = ().serve(transport);
    let timeout = std::time::Duration::from_secs(cfg.startup_timeout_sec);
    let svc = tokio::time::timeout(timeout, connect_fut)
        .await
        .map_err(|_| McpError::ConnectFailed {
            server: server.to_string(),
            reason: format!("timeout after {}s", cfg.startup_timeout_sec),
        })?
        .map_err(|e| McpError::ConnectFailed {
            server: server.to_string(),
            reason: format!("init failed: {e}"),
        })?;
    Ok(svc)
}

/// Resolve `${env:VAR}` placeholders in env values. Unresolved → error.
fn resolve_env(env: &HashMap<String, String>) -> McpResult<HashMap<String, String>> {
    let mut out = HashMap::with_capacity(env.len());
    for (k, v) in env {
        out.insert(k.clone(), expand_env_str(v)?);
    }
    Ok(out)
}

fn expand_env_str(s: &str) -> McpResult<String> {
    const PREFIX: &str = "${env:";
    const SUFFIX: &str = "}";
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find(PREFIX) {
        out.push_str(&rest[..start]);
        let after = &rest[start + PREFIX.len()..];
        let end = after
            .find(SUFFIX)
            .ok_or_else(|| McpError::Config(format!("unterminated env placeholder in `{s}`")))?;
        let var_name = &after[..end];
        let val = std::env::var(var_name).map_err(|_| {
            McpError::Config(format!(
                "env var `{var_name}` not set (referenced by `{s}`)"
            ))
        })?;
        out.push_str(&val);
        rest = &after[end + SUFFIX.len()..];
    }
    out.push_str(rest);
    Ok(out)
}

/// Call a server-native tool by raw name. Applies allow / deny check before
/// dispatch. Caller is expected to have applied the qualifier already (this
/// function deals with raw names because that's how the server speaks).
pub async fn call_raw_tool(
    client: &McpClient,
    server: &str,
    raw_tool: &str,
    arguments: serde_json::Value,
) -> McpResult<serde_json::Value> {
    let cfg = client.server_config(server)?;
    cfg.check_tool_allowed(server, raw_tool)?;
    let conn = client.get_or_connect(server).await?;

    let arg_map = match arguments {
        serde_json::Value::Object(m) => Some(m),
        serde_json::Value::Null => None,
        other => {
            return Err(McpError::Config(format!(
                "arguments must be a JSON object, got {other}"
            )))
        }
    };
    let params = CallToolRequestParam {
        name: raw_tool.to_string().into(),
        arguments: arg_map,
    };
    let timeout = std::time::Duration::from_secs(cfg.tool_timeout_sec);
    let result = tokio::time::timeout(timeout, conn.peer().call_tool(params))
        .await
        .map_err(|_| McpError::Timeout {
            server: server.to_string(),
            tool: raw_tool.to_string(),
            ms: cfg.tool_timeout_sec * 1000,
        })?
        .map_err(|e| McpError::Transport(e.to_string()))?;

    serde_json::to_value(&result).map_err(|e| McpError::Transport(format!("serialize result: {e}")))
}

/// List server-native tools. Side effect: populates the annotation cache so
/// `RiskAugmenter` lookups become free thereafter.
pub async fn list_raw_tools(client: &McpClient, server: &str) -> McpResult<serde_json::Value> {
    let conn = client.get_or_connect(server).await?;
    let result = conn
        .peer()
        .list_tools(Default::default())
        .await
        .map_err(|e| McpError::Transport(e.to_string()))?;
    // Populate annotation cache before serializing — every tool with
    // `annotations: Some(_)` gets an entry. Tools without annotations are
    // omitted (lookup returns `None` → augmenter falls back to defaults).
    for tool in &result.tools {
        if let Some(ann) = &tool.annotations {
            client.annotations().insert(server, &tool.name, ann.clone());
        }
    }
    serde_json::to_value(&result).map_err(|e| McpError::Transport(format!("serialize list: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_env_no_placeholder() {
        assert_eq!(expand_env_str("plain").unwrap(), "plain");
    }

    #[test]
    fn expand_env_resolves_set_var() {
        std::env::set_var("HC_MCP_TEST_X", "abc");
        let out = expand_env_str("v=${env:HC_MCP_TEST_X};").unwrap();
        assert_eq!(out, "v=abc;");
    }

    #[test]
    fn expand_env_unset_errors() {
        let r = expand_env_str("${env:HC_MCP_DEFINITELY_NOT_SET_99}");
        assert!(matches!(r, Err(McpError::Config(_))));
    }

    #[test]
    fn expand_env_unterminated_errors() {
        let r = expand_env_str("v=${env:OPEN");
        assert!(matches!(r, Err(McpError::Config(_))));
    }

    #[tokio::test]
    async fn unknown_server_errors() {
        let cfg = McpUserConfig::default();
        let client = McpClient::new(cfg);
        let r = client.get_or_connect("ghost").await;
        assert!(matches!(r, Err(McpError::UnknownServer { .. })));
    }
}
