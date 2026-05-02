//! Hivecore Layer-3 MCP client (ADR-027).
//!
//! Opt-in — bundled with default harnesses (`hivecore-coder`, `hivecore-acp-server`)
//! via Cargo dep, NOT in kernel, NOT a WASM extension. Default mode advertises three
//! meta-tools (`mcp_servers`, `mcp_discover`, `mcp_call`) and connects per-server lazily
//! on first use. Eager mode (Codex shape — every MCP tool flattened into the agent's
//! tool list at startup) is opt-in via builder.
//!
//! v0.2 surface — present scaffold ships pure pieces (name qualifier, config parser,
//! error enum). rmcp wiring lands in a follow-up commit.

#![deny(missing_debug_implementations)]
#![warn(rust_2018_idioms, unreachable_pub)]

pub mod client;
pub mod config;
pub mod error;
pub mod meta_tools;
pub mod name;
pub mod risk;

pub use client::{call_raw_tool, list_raw_tools, AnnotationCache, ConnectionCache, McpClient};
pub use meta_tools::{default_meta_tools, McpCallTool, McpDiscoverTool, McpServersTool};
pub use risk::McpRiskAugmenter;

pub use config::{ApprovalMode, McpServerConfig, McpToolConfig, McpUserConfig, TransportKind};
pub use error::{McpError, McpResult};
pub use name::{qualify_tool_name, sanitize, MAX_TOOL_NAME_LENGTH, NAMESPACE_PREFIX};
