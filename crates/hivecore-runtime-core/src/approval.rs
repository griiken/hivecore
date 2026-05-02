//! Human-in-the-loop approval primitive (ADR-029).
//!
//! Layer-1 trait surface — I/O-free. The driver in `hivecore-agent-loop`
//! awaits `Approval::request` between `pre_hooks` and `execute_tool`.
//! Layer-3 sinks (`hivecore-acp-server`, `hivecore-coder`, future web UI)
//! provide the impl.
//!
//! Conceptual model: Codex `ReviewDecision` (richer state machine).
//! Wire vocabulary: ACP `PermissionOptionKind` (4 standard options +
//! `optionId` round-trip for typed amendments).
//! Auto-decision input: Warp `RiskHint`-style metadata.

use std::path::PathBuf;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::ids::{SessionId, ToolCallId, TurnId};
use crate::tool::ToolInvocation;

/// What the runtime is asking the human about. Mirrors Codex's
/// `GuardianAssessmentAction` minus variants deferred to v0.2
/// (`Network`, `RequestPermissions`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ApprovalAction {
    /// A `Tool::execute` call. Identity = `(tool_name, invocation_id)`.
    Tool {
        tool_name: String,
        invocation_id: ToolCallId,
        input_preview: serde_json::Value,
        risk: RiskHint,
    },
    /// A workspace-mutating patch. Identity = `(root, files)`.
    ApplyPatch {
        root: PathBuf,
        files: Vec<PathBuf>,
        /// Serialized `FileChange` map; opaque to Layer 1.
        changes: serde_json::Value,
    },
    /// MCP tool call — kept distinct from `Tool` for routing + caching
    /// (identity = `(server, tool_name)`).
    Mcp {
        server: String,
        tool_name: String,
        input_preview: serde_json::Value,
    },
}

/// Coarse risk metadata set by the action emitter. Consumed by the
/// Layer-3 policy plane to short-circuit `AskUser` (e.g., `read_only=true`
/// auto-allows under `UnlessTrusted`). The user never sees these flags
/// directly.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct RiskHint {
    pub read_only: Option<bool>,
    pub risky: Option<bool>,
    pub network: Option<bool>,
}

/// What the user picked. Subset of Codex `ReviewDecision`.
///
/// `ApprovedAndPersist` carries a typed amendment payload — the v0.1
/// driver treats it identically to `Approved` (proceeds with the call);
/// v0.2 audit/policy plane will write the rule through to durable
/// storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum ApprovalDecision {
    Approved,
    ApprovedForSession,
    ApprovedAndPersist { rule: ApprovalRule },
    Denied,
    TimedOut,
    Abort,
    Cancelled,
}

/// A typed, human-readable amendment. Persisted by the audit/policy
/// plane (ADR-019) at v0.2.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ApprovalRule {
    ToolPrefix { name_prefix: String },
    McpToolAllow { server: String, tool: String },
    PathRoot { root: PathBuf, write: bool },
}

/// Scope axis (Codex `PermissionGrantScope` plus `Call` for one-shot).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalScope {
    #[default]
    Call,
    Turn,
    Session,
}

/// Borrowed request payload handed to the sink.
#[derive(Debug)]
pub struct ApprovalRequest<'a> {
    pub session_id: SessionId,
    pub turn_id: TurnId,
    pub action: &'a ApprovalAction,
    pub reason: Option<String>,
    /// Aider pattern (`io.py:812, 908`): destructive ops require literal `y`.
    /// Sinks should treat default-Enter as `Denied` when this is set.
    pub explicit_yes_required: bool,
}

/// The single trait Layer 3 implements; the `ApprovalHook` in
/// `hivecore-tool-policy` consumes it.
#[async_trait]
pub trait Approval: Send + Sync + std::fmt::Debug {
    async fn request(&self, req: ApprovalRequest<'_>) -> ApprovalDecision;
}

/// Augments a `ToolInvocation` with `RiskHint` metadata. Layer-3 implementors
/// (e.g. `McpRiskAugmenter` reading MCP `ToolAnnotations.read_only_hint`,
/// future built-in-tool emitter, OpenHands-style LLM-self-reported risk)
/// supply concrete instances. The `ApprovalHook` reads the augmented hint to
/// decide whether `UnlessTrusted` policy can short-circuit the human prompt.
///
/// Synchronous on purpose — risk lookup is expected to be a cache hit. If a
/// future augmenter needs I/O, return `RiskHint::default()` from `augment()`
/// and warm the cache out-of-band.
pub trait RiskAugmenter: Send + Sync + std::fmt::Debug {
    fn augment(&self, inv: &ToolInvocation) -> RiskHint;
}

#[cfg(test)]
#[path = "approval_tests.rs"]
mod tests;
