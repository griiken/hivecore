//! Hivecore Layer 1.5 — agent loop (spike).
//!
//! Drives a `ModelAdapter`, a `ToolRegistry`, and a chain of `ToolHook`s into
//! a working agent conversation. I/O-free except where the underlying model
//! adapter and tools choose to do work.
//!
//! Loop shape mirrors `badlogic/pi-mono/packages/agent/src/agent-loop.ts`
//! (nested outer/inner loops with steering + follow-up message hooks),
//! adapted to Rust idioms and Codex's hook trichotomy
//! (see `.planning/intel/codex-patterns.md` §3).

#![deny(missing_debug_implementations)]
#![warn(rust_2018_idioms, unreachable_pub)]

pub mod accumulator;
pub mod driver;
pub mod error;
pub mod registry;
pub mod sink;
pub mod spawn;
pub mod steering;

pub use driver::{AgentLoop, RunOutcome};
pub use error::LoopError;
pub use hivecore_runtime_core::{EventSink, FanOutSink, NoopSink, VecSink};
pub use registry::ToolRegistry;
pub use spawn::{ClosureSpec, SpawnAgentTool, SubAgentSpec};
pub use steering::{NoopSteering, SteeringSource};
