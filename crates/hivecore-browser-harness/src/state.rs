//! `BrowserHarness` — the harness-side glue between the agent's session
//! and a `BrowserProvider`. Maps the agent's `SessionId` (from
//! `hivecore-runtime-core`) onto a provider-issued `BrowserSessionId`,
//! enforces the tenant scope, and stores the most recent snapshot for
//! convenience lookups.

use std::collections::HashMap;
use std::sync::Arc;

use hivecore_browser_core::{
    BrowserContext, BrowserError, BrowserProvider, BrowserSessionId, Snapshot, TenantId,
};
use tokio::sync::Mutex;

#[derive(Debug, Default)]
pub struct HarnessState {
    /// Per-agent-session browser-session mapping. The agent calls
    /// `browser_session` w/ a name; we resolve to a `BrowserSessionId`
    /// and reuse it across subsequent turns.
    pub sessions: HashMap<String, BrowserSessionId>,
    /// Latest snapshot per session. Used by hooks + audit.
    pub latest: HashMap<BrowserSessionId, Snapshot>,
}

#[derive(Debug)]
pub struct BrowserHarness {
    pub provider: Arc<dyn BrowserProvider>,
    pub tenant: TenantId,
    state: Mutex<HarnessState>,
}

impl BrowserHarness {
    pub fn new(provider: Arc<dyn BrowserProvider>, tenant: TenantId) -> Arc<Self> {
        Arc::new(Self {
            provider,
            tenant,
            state: Mutex::new(HarnessState::default()),
        })
    }

    pub async fn ensure_named_session(&self, name: &str) -> Result<BrowserContext, BrowserError> {
        let mut state = self.state.lock().await;
        if let Some(sid) = state.sessions.get(name) {
            return Ok(BrowserContext::new(self.tenant.clone(), sid.clone()));
        }
        let sid = self
            .provider
            .ensure_session(
                &self.tenant,
                hivecore_browser_core::provider::SessionSpec {
                    session_name_hint: Some(name.to_string()),
                    ..Default::default()
                },
            )
            .await?;
        state.sessions.insert(name.to_string(), sid.clone());
        Ok(BrowserContext::new(self.tenant.clone(), sid))
    }

    pub async fn ctx_for(&self, name: &str) -> Result<BrowserContext, BrowserError> {
        let state = self.state.lock().await;
        state
            .sessions
            .get(name)
            .map(|sid| BrowserContext::new(self.tenant.clone(), sid.clone()))
            .ok_or_else(|| BrowserError::SessionNotFound(name.to_string()))
    }

    pub async fn record_snapshot(&self, ctx: &BrowserContext, snap: &Snapshot) {
        let mut state = self.state.lock().await;
        state.latest.insert(ctx.session.clone(), snap.clone());
    }
}
