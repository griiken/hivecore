//! ADR-019 audit event schema.

use chrono::{DateTime, Utc};
use hivecore_runtime_core::{SessionId, TurnId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EventId(pub Uuid);

impl EventId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for EventId {
    fn default() -> Self {
        Self::new()
    }
}

/// Tenant identifier. Hivecore is multi-tenant by design — every audit event
/// carries one. Single-tenant deployments use a fixed sentinel value.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TenantId(pub String);

impl TenantId {
    pub fn single_tenant() -> Self {
        Self("default".into())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditClass {
    Orchestrator,
    Model,
    Tool,
    Git,
    Test,
    Policy,
    Cost,
    /// Knowledge-graph mutation (hivecore addition over GSD-2).
    Kg,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub event_id: EventId,
    pub trace_id: SessionId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<TurnId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caused_by: Option<EventId>,
    pub timestamp: DateTime<Utc>,
    pub tenant_id: TenantId,
    pub class: AuditClass,
    pub payload: serde_json::Value,
}

impl AuditEvent {
    pub fn new(
        trace_id: SessionId,
        tenant_id: TenantId,
        class: AuditClass,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            event_id: EventId::new(),
            trace_id,
            turn_id: None,
            caused_by: None,
            timestamp: Utc::now(),
            tenant_id,
            class,
            payload,
        }
    }

    pub fn with_turn(mut self, turn: TurnId) -> Self {
        self.turn_id = Some(turn);
        self
    }

    pub fn caused_by(mut self, parent: EventId) -> Self {
        self.caused_by = Some(parent);
        self
    }
}
