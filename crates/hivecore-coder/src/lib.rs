//! Sample harness on top of hivecore Layer 1+2 — proves substrate composability.
//!
//! Different shape than `hivecore-acp-server`: no ACP, no skills, no agent
//! registry. Direct CLI loop showing the minimum needed to compose a working
//! agent harness from runtime-core + agent-loop + openai-adapter + builtin-tools.
//!
//! Adds three Layer-3-shaped pieces to demonstrate the extension surfaces:
//! - `TurnBudgetHook` — `LifecycleHook` that aborts after N model calls (gate-plane shape).
//! - `ClockTool` — custom `Tool` registered alongside builtins.
//! - `StderrSink` — `EventSink` that pretty-prints turn boundaries to stderr.

#![warn(rust_2018_idioms, unreachable_pub)]

pub mod cli_prompter;
pub mod clock_tool;
pub mod stderr_sink;
pub mod turn_budget;

pub use cli_prompter::CliPrompter;
pub use clock_tool::ClockTool;
pub use stderr_sink::StderrSink;
pub use turn_budget::TurnBudgetHook;
