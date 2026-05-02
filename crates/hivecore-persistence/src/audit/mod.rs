//! Audit-plane writer. ADR-019 schema:
//!
//!   event_id / trace_id / turn_id / caused_by / timestamp / tenant_id / payload
//!
//! Append-only JSONL, projection layer (SQLite/Postgres) is out of scope for
//! the spike. Every event class is typed (orchestrator | model | tool | git
//! | test | policy | cost | kg).

mod schema;
mod writer;

pub use schema::{AuditClass, AuditEvent, EventId, TenantId};
pub use writer::AuditWriter;

#[cfg(test)]
#[path = "tests.rs"]
mod audit_tests;
