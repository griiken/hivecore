//! Hivecore Layer 1 — runtime core (spike).
//!
//! I/O-free trait surface mirroring `pi-agent-core`'s behavioral contract
//! (see `.planning/intel/pi-anatomy.md`) and Codex's hook trichotomy
//! (see `.planning/intel/codex-patterns.md` §3). No concrete adapters here.
//!
//! Module layout follows the Codex / Warp convention: one file per concern,
//! tests live in a sibling `*_tests.rs` included via `#[cfg(test)] mod`.

#![deny(missing_debug_implementations)]
#![warn(rust_2018_idioms, unreachable_pub)]

pub mod abort;
pub mod approval;
pub mod error;
pub mod event;
pub mod hook;
pub mod ids;
pub mod lifecycle;
pub mod message;
pub mod model;
pub mod sink;
pub mod state;
pub mod tool;
pub mod transform;

pub use abort::{AbortHandle, AbortSignal};
pub use approval::{
    Approval, ApprovalAction, ApprovalDecision, ApprovalRequest, ApprovalRule, ApprovalScope,
    RiskAugmenter, RiskHint,
};
pub use error::{RuntimeError, RuntimeResult};
pub use event::AgentEvent;
pub use hook::{HookOutcome, PostHookOutcome, ToolHook, ToolHookContext, ToolPostContext};
pub use ids::{MessageId, SessionId, ToolCallId, TurnId};
pub use lifecycle::{LifecycleEvent, LifecycleHook, LifecycleOutcome};
pub use message::{AgentMessage, ContentBlock, StopReason};
pub use model::{ModelAdapter, ModelChunk, ModelRequest, ModelStream, TokenUsage};
pub use sink::{EventSink, FanOutSink, NoopSink, VecSink};
pub use state::{AgentState, ThinkingLevel, ToolDescriptor};
pub use tool::{Tool, ToolInvocation, ToolOutcome, UpdateSink};
pub use transform::ContextTransform;
