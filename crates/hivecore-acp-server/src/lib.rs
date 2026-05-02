//! Hivecore ACP server (spike).
//!
//! Exposes our `AgentLoop` over the Agent Client Protocol so any ACP-aware
//! editor (Zed, the official `agent-client-protocol` test clients, etc.)
//! can mount this binary as a subprocess and drive sessions.
//!
//! Wire format + framing handled by the official `agent-client-protocol`
//! crate (JSON-RPC 2.0 over stdio).

#![deny(missing_debug_implementations)]
#![warn(rust_2018_idioms, unreachable_pub)]

pub mod bridge;
pub mod server;
