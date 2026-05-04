//! Hivecore persistence (spike).
//!
//! Two append-only JSONL sinks that subscribe to the `EventSink` contract
//! from `agent-loop` (re-exposed here as `hivecore_runtime_core::AgentEvent`):
//!
//!   - `sessions::SessionWriter` — pi-style conversation log. One file per
//!     session, replay reconstructs `AgentState`. See ADR-016.
//!   - `audit::AuditWriter`     — ADR-019 audit-plane schema with
//!     `event_id` / `trace_id` / `turn_id` / `caused_by` / `tenant_id` /
//!     `payload`.
//!
//! Both are pure subscribers — they never feed back into the loop. A broken
//! sink is logged via `tracing::warn` and otherwise swallowed.

#![deny(missing_debug_implementations)]
#![warn(rust_2018_idioms, unreachable_pub)]

pub mod audit;
pub mod error;
pub mod sessions;

pub use audit::{AuditClass, AuditEvent, AuditWriter, TenantId};
pub use error::PersistenceError;
pub use hivecore_runtime_core::EventSink;
pub use sessions::{
    acquire_lock, append_entry, find_by_id, find_by_name, find_latest_by_cwd, index_path,
    resolve_handle, session_path, tenant_dir, EntryId, LockError, SessionEntry, SessionHeader,
    SessionIndexEntry, SessionLock, SessionReader, SessionWriter,
};
