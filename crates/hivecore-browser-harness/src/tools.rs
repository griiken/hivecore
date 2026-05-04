//! Typed `Tool` impls. Names align with Playwright MCP convention.
//!
//! v0.1 set:
//!   - `browser_session` (open / close / list)
//!   - `browser_navigate`
//!   - `browser_snapshot`
//!   - `browser_act` (one tool, typed `kind` enum)
//!   - `browser_wait`
//!   - `browser_assert`
//!   - `browser_screenshot`

use std::sync::Arc;

use async_trait::async_trait;
use hivecore_browser_core::{ActionKind, AssertPredicate, ElementRef, WaitCondition};
use hivecore_runtime_core::{
    AbortSignal, ContentBlock, RuntimeError, RuntimeResult, Tool, ToolExecutionMode,
    ToolInvocation, ToolOutcome, UpdateSink,
};
use serde::Deserialize;
use serde_json::json;

use crate::state::BrowserHarness;

fn json_outcome(value: serde_json::Value) -> ToolOutcome {
    let text = serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());
    ToolOutcome {
        content: vec![ContentBlock::Text { text }],
        details: Some(value),
        is_error: false,
    }
}

fn err_outcome(msg: impl Into<String>) -> ToolOutcome {
    ToolOutcome::error(msg)
}

// ===== browser_session ========================================================

#[derive(Debug)]
pub struct BrowserSessionTool {
    harness: Arc<BrowserHarness>,
}

impl BrowserSessionTool {
    pub fn new(harness: Arc<BrowserHarness>) -> Self {
        Self { harness }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum SessionOp {
    Open { name: String },
    Close { name: String },
    List,
}

#[async_trait]
impl Tool for BrowserSessionTool {
    fn name(&self) -> &str {
        "browser_session"
    }
    fn execution_mode(&self) -> ToolExecutionMode {
        // ADR-034 — single shared browser session; concurrent ops would race.
        ToolExecutionMode::Sequential
    }
    fn description(&self) -> &str {
        "Open / close / list browser sessions for this tenant. Each session is an isolated browser process."
    }
    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "op": { "type": "string", "enum": ["open", "close", "list"] },
                "name": { "type": "string", "description": "Session name. Required for open/close." }
            },
            "required": ["op"]
        })
    }
    async fn execute(
        &self,
        invocation: ToolInvocation,
        _signal: AbortSignal,
        _on_update: UpdateSink,
    ) -> RuntimeResult<ToolOutcome> {
        let op: SessionOp = serde_json::from_value(invocation.input)
            .map_err(|e| RuntimeError::InvalidInput(e.to_string()))?;
        match op {
            SessionOp::Open { name } => match self.harness.ensure_named_session(&name).await {
                Ok(ctx) => Ok(json_outcome(json!({
                    "ok": true,
                    "session_name": name,
                    "session_id": ctx.session.0,
                }))),
                Err(e) => Ok(err_outcome(e.to_string())),
            },
            SessionOp::Close { name } => {
                let ctx = match self.harness.ctx_for(&name).await {
                    Ok(c) => c,
                    Err(e) => return Ok(err_outcome(e.to_string())),
                };
                if let Err(e) = self.harness.provider.close_session(&ctx).await {
                    return Ok(err_outcome(e.to_string()));
                }
                Ok(json_outcome(json!({ "ok": true, "closed": name })))
            }
            SessionOp::List => {
                match self
                    .harness
                    .provider
                    .list_sessions(&self.harness.tenant)
                    .await
                {
                    Ok(list) => Ok(json_outcome(json!({
                        "ok": true,
                        "sessions": list.iter().map(|s| s.0.clone()).collect::<Vec<_>>(),
                    }))),
                    Err(e) => Ok(err_outcome(e.to_string())),
                }
            }
        }
    }
}

// ===== browser_navigate ======================================================

#[derive(Debug)]
pub struct BrowserNavigateTool {
    harness: Arc<BrowserHarness>,
}

impl BrowserNavigateTool {
    pub fn new(harness: Arc<BrowserHarness>) -> Self {
        Self { harness }
    }
}

#[derive(Debug, Deserialize)]
struct NavigateInput {
    session: String,
    url: String,
}

#[async_trait]
impl Tool for BrowserNavigateTool {
    fn name(&self) -> &str {
        "browser_navigate"
    }
    fn execution_mode(&self) -> ToolExecutionMode {
        // ADR-034 — single shared browser session; concurrent ops would race.
        ToolExecutionMode::Sequential
    }
    fn description(&self) -> &str {
        "Navigate the named session's active page to `url`. Returns when navigation completes."
    }
    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "session": { "type": "string" },
                "url": { "type": "string" }
            },
            "required": ["session", "url"]
        })
    }
    async fn execute(
        &self,
        invocation: ToolInvocation,
        _signal: AbortSignal,
        _on_update: UpdateSink,
    ) -> RuntimeResult<ToolOutcome> {
        let input: NavigateInput = serde_json::from_value(invocation.input)
            .map_err(|e| RuntimeError::InvalidInput(e.to_string()))?;
        let ctx = match self.harness.ensure_named_session(&input.session).await {
            Ok(c) => c,
            Err(e) => return Ok(err_outcome(e.to_string())),
        };
        match self.harness.provider.navigate(&ctx, &input.url).await {
            Ok(()) => Ok(json_outcome(
                json!({ "ok": true, "navigated_to": input.url }),
            )),
            Err(e) => Ok(err_outcome(e.to_string())),
        }
    }
}

// ===== browser_snapshot ======================================================

#[derive(Debug)]
pub struct BrowserSnapshotTool {
    harness: Arc<BrowserHarness>,
}

impl BrowserSnapshotTool {
    pub fn new(harness: Arc<BrowserHarness>) -> Self {
        Self { harness }
    }
}

#[derive(Debug, Deserialize)]
struct SnapshotInput {
    session: String,
}

#[async_trait]
impl Tool for BrowserSnapshotTool {
    fn name(&self) -> &str {
        "browser_snapshot"
    }
    fn execution_mode(&self) -> ToolExecutionMode {
        // ADR-034 — single shared browser session; concurrent ops would race.
        ToolExecutionMode::Sequential
    }
    fn description(&self) -> &str {
        "Capture a fresh accessibility-tree snapshot of the named session. Returns versioned element refs (`@v<N>:e<id>`) the agent must use for subsequent actions. Re-snapshot after every navigation or DOM change."
    }
    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": { "session": { "type": "string" } },
            "required": ["session"]
        })
    }
    async fn execute(
        &self,
        invocation: ToolInvocation,
        _signal: AbortSignal,
        _on_update: UpdateSink,
    ) -> RuntimeResult<ToolOutcome> {
        let input: SnapshotInput = serde_json::from_value(invocation.input)
            .map_err(|e| RuntimeError::InvalidInput(e.to_string()))?;
        let ctx = match self.harness.ensure_named_session(&input.session).await {
            Ok(c) => c,
            Err(e) => return Ok(err_outcome(e.to_string())),
        };
        match self.harness.provider.snapshot(&ctx).await {
            Ok(snap) => {
                self.harness.record_snapshot(&ctx, &snap).await;
                Ok(json_outcome(
                    serde_json::to_value(&snap).unwrap_or(json!({})),
                ))
            }
            Err(e) => Ok(err_outcome(e.to_string())),
        }
    }
}

// ===== browser_act ===========================================================

#[derive(Debug)]
pub struct BrowserActTool {
    harness: Arc<BrowserHarness>,
}

impl BrowserActTool {
    pub fn new(harness: Arc<BrowserHarness>) -> Self {
        Self { harness }
    }
}

#[derive(Debug, Deserialize)]
struct ActInput {
    session: String,
    /// `@v<N>:e<id>` — must match latest snapshot version.
    target: String,
    #[serde(flatten)]
    kind: ActionKind,
}

#[async_trait]
impl Tool for BrowserActTool {
    fn name(&self) -> &str {
        "browser_act"
    }
    fn execution_mode(&self) -> ToolExecutionMode {
        // ADR-034 — single shared browser session; concurrent ops would race.
        ToolExecutionMode::Sequential
    }
    fn description(&self) -> &str {
        "Apply an action (click, type, fill, press, hover, set_checked, select, drag, upload, scroll) to a versioned element ref. The ref MUST be from the latest snapshot — otherwise this returns a stale-ref error and you must re-snapshot."
    }
    fn parameters(&self) -> serde_json::Value {
        // One tool, kind-discriminated. Compact for the model's tool list
        // (open question #2 resolved: typed enum > 10 separate tools).
        json!({
            "type": "object",
            "properties": {
                "session": { "type": "string" },
                "target": { "type": "string", "description": "Versioned element ref like @v3:e12" },
                "kind": {
                    "type": "string",
                    "enum": ["click","type","fill","press","hover","set_checked","select","drag","upload","scroll"]
                },
                "text": { "type": "string", "description": "For type/fill" },
                "key": { "type": "string", "description": "For press" },
                "checked": { "type": "boolean", "description": "For set_checked" },
                "option": { "type": "string", "description": "For select" },
                "to": { "type": "string", "description": "For drag — destination ref" },
                "path": { "type": "string", "description": "For upload — local file path" }
            },
            "required": ["session", "target", "kind"]
        })
    }
    async fn execute(
        &self,
        invocation: ToolInvocation,
        _signal: AbortSignal,
        _on_update: UpdateSink,
    ) -> RuntimeResult<ToolOutcome> {
        let input: ActInput = serde_json::from_value(invocation.input)
            .map_err(|e| RuntimeError::InvalidInput(e.to_string()))?;
        let ctx = match self.harness.ctx_for(&input.session).await {
            Ok(c) => c,
            Err(e) => return Ok(err_outcome(e.to_string())),
        };
        let target = match ElementRef::parse(&input.target) {
            Some(t) => t,
            None => return Ok(err_outcome(format!("malformed ref: {}", input.target))),
        };
        match self.harness.provider.act(&ctx, &target, input.kind).await {
            Ok((outcome, snap)) => {
                self.harness.record_snapshot(&ctx, &snap).await;
                Ok(json_outcome(json!({
                    "ok": outcome.ok,
                    "note": outcome.note,
                    "snapshot": snap,
                })))
            }
            Err(e) => Ok(err_outcome(e.to_string())),
        }
    }
}

// ===== browser_wait ==========================================================

#[derive(Debug)]
pub struct BrowserWaitTool {
    harness: Arc<BrowserHarness>,
}
impl BrowserWaitTool {
    pub fn new(harness: Arc<BrowserHarness>) -> Self {
        Self { harness }
    }
}

#[derive(Debug, Deserialize)]
struct WaitInput {
    session: String,
    #[serde(flatten)]
    condition: WaitCondition,
    #[serde(default = "default_timeout")]
    timeout_ms: u64,
}

fn default_timeout() -> u64 {
    10_000
}

#[async_trait]
impl Tool for BrowserWaitTool {
    fn name(&self) -> &str {
        "browser_wait"
    }
    fn execution_mode(&self) -> ToolExecutionMode {
        // ADR-034 — single shared browser session; concurrent ops would race.
        ToolExecutionMode::Sequential
    }
    fn description(&self) -> &str {
        "Wait for a condition (load, dom_content_loaded, network_idle, selector_visible, ref_visible, url_contains, url_matches, text_visible, text_hidden, delay) up to `timeout_ms`."
    }
    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "session": { "type": "string" },
                "condition": {
                    "type": "string",
                    "enum": ["load","dom_content_loaded","network_idle","selector_visible","selector_hidden","ref_visible","url_contains","url_matches","text_visible","text_hidden","delay"]
                },
                "selector": { "type": "string" },
                "ref": { "type": "string" },
                "substring": { "type": "string" },
                "pattern": { "type": "string" },
                "ms": { "type": "integer", "description": "For delay" },
                "timeout_ms": { "type": "integer", "default": 10000 }
            },
            "required": ["session", "condition"]
        })
    }
    async fn execute(
        &self,
        invocation: ToolInvocation,
        _signal: AbortSignal,
        _on_update: UpdateSink,
    ) -> RuntimeResult<ToolOutcome> {
        let input: WaitInput = serde_json::from_value(invocation.input)
            .map_err(|e| RuntimeError::InvalidInput(e.to_string()))?;
        let ctx = match self.harness.ctx_for(&input.session).await {
            Ok(c) => c,
            Err(e) => return Ok(err_outcome(e.to_string())),
        };
        match self
            .harness
            .provider
            .wait_for(&ctx, input.condition, input.timeout_ms)
            .await
        {
            Ok(()) => Ok(json_outcome(json!({ "ok": true }))),
            Err(e) => Ok(err_outcome(e.to_string())),
        }
    }
}

// ===== browser_assert ========================================================

#[derive(Debug)]
pub struct BrowserAssertTool {
    harness: Arc<BrowserHarness>,
}
impl BrowserAssertTool {
    pub fn new(harness: Arc<BrowserHarness>) -> Self {
        Self { harness }
    }
}

#[derive(Debug, Deserialize)]
struct AssertInput {
    session: String,
    #[serde(flatten)]
    predicate: AssertPredicate,
}

#[async_trait]
impl Tool for BrowserAssertTool {
    fn name(&self) -> &str {
        "browser_assert"
    }
    fn execution_mode(&self) -> ToolExecutionMode {
        // ADR-034 — single shared browser session; concurrent ops would race.
        ToolExecutionMode::Sequential
    }
    fn description(&self) -> &str {
        "Run an assertion against the current snapshot. Returns true/false plus evidence."
    }
    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "session": { "type": "string" },
                "predicate": {
                    "type": "string",
                    "enum": ["exists","name_equals","url_equals","url_contains","text_visible","count"]
                },
                "ref": { "type": "string" },
                "expected": { "type": "string" },
                "substring": { "type": "string" },
                "role": { "type": "string" }
            },
            "required": ["session", "predicate"]
        })
    }
    async fn execute(
        &self,
        invocation: ToolInvocation,
        _signal: AbortSignal,
        _on_update: UpdateSink,
    ) -> RuntimeResult<ToolOutcome> {
        let input: AssertInput = serde_json::from_value(invocation.input)
            .map_err(|e| RuntimeError::InvalidInput(e.to_string()))?;
        let ctx = match self.harness.ctx_for(&input.session).await {
            Ok(c) => c,
            Err(e) => return Ok(err_outcome(e.to_string())),
        };
        match self.harness.provider.assert(&ctx, input.predicate).await {
            Ok(b) => Ok(json_outcome(json!({ "ok": true, "result": b }))),
            Err(e) => Ok(err_outcome(e.to_string())),
        }
    }
}

// ===== browser_screenshot ====================================================

#[derive(Debug)]
pub struct BrowserScreenshotTool {
    harness: Arc<BrowserHarness>,
}
impl BrowserScreenshotTool {
    pub fn new(harness: Arc<BrowserHarness>) -> Self {
        Self { harness }
    }
}

#[derive(Debug, Deserialize)]
struct ScreenshotInput {
    session: String,
}

#[async_trait]
impl Tool for BrowserScreenshotTool {
    fn name(&self) -> &str {
        "browser_screenshot"
    }
    fn execution_mode(&self) -> ToolExecutionMode {
        // ADR-034 — single shared browser session; concurrent ops would race.
        ToolExecutionMode::Sequential
    }
    fn description(&self) -> &str {
        "Capture a PNG screenshot of the page. Bytes returned in details; primary content reports artifact size only (visual confirmation, not interaction)."
    }
    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": { "session": { "type": "string" } },
            "required": ["session"]
        })
    }
    async fn execute(
        &self,
        invocation: ToolInvocation,
        _signal: AbortSignal,
        _on_update: UpdateSink,
    ) -> RuntimeResult<ToolOutcome> {
        let input: ScreenshotInput = serde_json::from_value(invocation.input)
            .map_err(|e| RuntimeError::InvalidInput(e.to_string()))?;
        let ctx = match self.harness.ctx_for(&input.session).await {
            Ok(c) => c,
            Err(e) => return Ok(err_outcome(e.to_string())),
        };
        match self.harness.provider.screenshot(&ctx).await {
            Ok(bytes) => Ok(ToolOutcome {
                content: vec![ContentBlock::Text {
                    text: format!("screenshot captured ({} bytes)", bytes.len()),
                }],
                details: Some(json!({ "ok": true, "bytes": bytes.len() })),
                is_error: false,
            }),
            Err(e) => Ok(err_outcome(e.to_string())),
        }
    }
}
