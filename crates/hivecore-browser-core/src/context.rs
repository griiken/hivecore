//! Multi-tenant context that flows through every BPP call.
//!
//! Per ADR-028 §6: `tenant_id` is currently a `String`. When the Tenancy
//! plane lands (planned ADR-029) `TenantId` will be promoted to a Layer-1
//! newtype in `hivecore-runtime-core::ids`; we keep it as a string here so
//! Layer 1 stays stable in the interim.

use serde::{Deserialize, Serialize};

/// Per-call multi-tenant identity. The provider rejects cross-tenant page
/// handles by construction.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct TenantId(pub String);

impl TenantId {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
    pub fn default_tenant() -> Self {
        Self("default".to_string())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for TenantId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Provider-side handle for a single browser session. Opaque from the
/// outside.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct BrowserSessionId(pub String);

impl BrowserSessionId {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for BrowserSessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Carried with every BPP call. Couples the requesting tenant to the
/// provider-side session handle; provider must verify the session belongs
/// to `tenant` before doing any work.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserContext {
    pub tenant: TenantId,
    pub session: BrowserSessionId,
}

impl BrowserContext {
    pub fn new(tenant: TenantId, session: BrowserSessionId) -> Self {
        Self { tenant, session }
    }
}
