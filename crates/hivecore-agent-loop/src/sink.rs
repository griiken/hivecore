//! Re-exports of the shared `EventSink` trait + helpers from `runtime-core`.
//! Kept as a thin module here so callers can `use hivecore_agent_loop::sink::*`
//! without reaching into Layer 1 — at the same time, persistence layers can
//! use the same trait directly from `runtime-core`.

pub use hivecore_runtime_core::{EventSink, FanOutSink, NoopSink, VecSink};
