//! `BrowserProvider` trait — the load-bearing Layer-2 contract.
//!
//! ADR-028 §1: pure trait, no I/O dependencies in this crate. Concrete
//! impls (chromiumoxide, WebDriver-BiDi, Playwright-MCP-as-provider) live
//! in sibling crates that depend on this one.

use async_trait::async_trait;

use crate::actions::{ActionKind, ActionOutcome, AssertPredicate, WaitCondition};
use crate::capability::CapabilityTable;
use crate::context::{BrowserContext, BrowserSessionId, TenantId};
use crate::error::BrowserResult;
use crate::snapshot::{ElementRef, Snapshot};

/// Provider-side hint for the kind of session to allocate.
#[derive(Debug, Clone, Default)]
pub struct SessionSpec {
    /// Pin a browser-side handle name. Optional; provider may generate one.
    pub session_name_hint: Option<String>,
    /// Initial URL — provider may navigate as part of session creation.
    pub initial_url: Option<String>,
    /// Storage-state JSON to restore (Playwright shape).
    pub storage_state: Option<serde_json::Value>,
    /// Pin viewport size for reproducibility.
    pub viewport: Option<(u32, u32)>,
}

/// Required + optional methods a browser provider must implement. v0.1
/// scope is the **required** half (annotated below); optional methods
/// have default impls returning `CapabilityMissing` so a minimal provider
/// compiles without them.
#[async_trait]
pub trait BrowserProvider: Send + Sync + std::fmt::Debug {
    // --- introspection -----------------------------------------------------

    /// Provider id (`chromiumoxide`, `firefox-bridge`, `playwright-mcp`, ...).
    fn id(&self) -> &str;

    /// Capabilities table — model + harness consult before invoking.
    fn capabilities(&self) -> &CapabilityTable;

    // --- session lifecycle (REQUIRED) -------------------------------------

    /// Create a session under `tenant`. Returns provider-issued session id
    /// the caller carries in subsequent `BrowserContext`s.
    async fn ensure_session(
        &self,
        tenant: &TenantId,
        spec: SessionSpec,
    ) -> BrowserResult<BrowserSessionId>;

    /// Close the session. Idempotent — closing a missing session is OK.
    async fn close_session(&self, ctx: &BrowserContext) -> BrowserResult<()>;

    /// List sessions belonging to this tenant. Cross-tenant calls return
    /// only the caller's sessions, never the union.
    async fn list_sessions(&self, tenant: &TenantId) -> BrowserResult<Vec<BrowserSessionId>>;

    // --- navigation (REQUIRED) --------------------------------------------

    async fn navigate(&self, ctx: &BrowserContext, url: &str) -> BrowserResult<()>;

    async fn back(&self, _ctx: &BrowserContext) -> BrowserResult<()> {
        Err(crate::error::BrowserError::CapabilityMissing("back".into()))
    }
    async fn forward(&self, _ctx: &BrowserContext) -> BrowserResult<()> {
        Err(crate::error::BrowserError::CapabilityMissing(
            "forward".into(),
        ))
    }
    async fn reload(&self, _ctx: &BrowserContext) -> BrowserResult<()> {
        Err(crate::error::BrowserError::CapabilityMissing(
            "reload".into(),
        ))
    }

    // --- snapshot (REQUIRED) ----------------------------------------------

    /// Bump the session's snapshot version, return the fresh snapshot.
    async fn snapshot(&self, ctx: &BrowserContext) -> BrowserResult<Snapshot>;

    // --- action (REQUIRED) ------------------------------------------------

    /// Apply `kind` to the element identified by `target`. Returns a
    /// fresh post-action snapshot inline so the agent's next turn can
    /// verify state without an extra round-trip.
    async fn act(
        &self,
        ctx: &BrowserContext,
        target: &ElementRef,
        kind: ActionKind,
    ) -> BrowserResult<(ActionOutcome, Snapshot)>;

    // --- wait (REQUIRED) --------------------------------------------------

    async fn wait_for(
        &self,
        ctx: &BrowserContext,
        condition: WaitCondition,
        timeout_ms: u64,
    ) -> BrowserResult<()>;

    // --- assert (REQUIRED) ------------------------------------------------

    async fn assert(&self, ctx: &BrowserContext, predicate: AssertPredicate)
        -> BrowserResult<bool>;

    // --- screenshot (optional) --------------------------------------------

    /// Capture a PNG. Default impl errors with `CapabilityMissing`.
    async fn screenshot(&self, _ctx: &BrowserContext) -> BrowserResult<Vec<u8>> {
        Err(crate::error::BrowserError::CapabilityMissing(
            "screenshot".into(),
        ))
    }

    // --- network / console / eval / trace (optional) ----------------------

    async fn console_messages(
        &self,
        _ctx: &BrowserContext,
    ) -> BrowserResult<Vec<serde_json::Value>> {
        Err(crate::error::BrowserError::CapabilityMissing(
            "console_messages".into(),
        ))
    }
    async fn network_list(&self, _ctx: &BrowserContext) -> BrowserResult<Vec<serde_json::Value>> {
        Err(crate::error::BrowserError::CapabilityMissing(
            "network_list".into(),
        ))
    }
    async fn eval(&self, _ctx: &BrowserContext, _expr: &str) -> BrowserResult<serde_json::Value> {
        // Capability-gated per ADR-028 §6 — default off.
        Err(crate::error::BrowserError::CapabilityMissing("eval".into()))
    }
    async fn trace_start(&self, _ctx: &BrowserContext) -> BrowserResult<()> {
        Err(crate::error::BrowserError::CapabilityMissing(
            "trace_start".into(),
        ))
    }
    async fn trace_stop(&self, _ctx: &BrowserContext) -> BrowserResult<String> {
        Err(crate::error::BrowserError::CapabilityMissing(
            "trace_stop".into(),
        ))
    }
}
