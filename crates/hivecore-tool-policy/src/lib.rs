//! ADR-029 — Layer-3 HITL approval policy.
//!
//! Bridges the runtime-core `Approval` trait (the sink that asks the human)
//! into the agent loop via a `ToolHook` impl. The driver is not modified;
//! `ApprovalHook` returns existing `HookOutcome` variants:
//!
//! | User decision         | `HookOutcome`               | Effect                              |
//! |-----------------------|------------------------------|-------------------------------------|
//! | Approved              | `Pass`                       | tool executes                       |
//! | ApprovedForSession    | `Pass` + cache               | future identical calls auto-approve |
//! | ApprovedAndPersist    | `Pass` (v0.2 also writes)    | tool executes                       |
//! | Denied / TimedOut     | `FailedContinue { reason }`  | model sees synthetic tool error     |
//! | Abort / Cancelled     | `ManualAttention { reason }` | bubbles to harness                  |
//!
//! Pi-mono inspiration: `pi-coding-agent/src/core/agent-session.ts:540-565`.
//! Codex inspiration: `codex-rs/core/src/mcp_tool_call.rs:948-1058`.

#![deny(missing_debug_implementations)]
#![warn(rust_2018_idioms, unreachable_pub)]

pub mod cache;
pub mod exclude;
pub mod hook;
pub mod matcher;
pub mod policy;

pub use cache::SessionCache;
pub use exclude::apply_exclude;
pub use hook::ApprovalHook;
pub use matcher::{MatchOutcome, ToolMatcher, ToolNameMatcher};
pub use policy::ApprovalPolicy;

#[cfg(test)]
mod tests;
