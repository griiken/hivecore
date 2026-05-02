//! Hivecore WASM extension host (spike).
//!
//! Loads `.wasm` Component-Model files implementing the
//! `hivecore:extension/extension@0.1.0` world (see `wit/since_v0.1.0/`).
//! Each tool the extension declares is exposed to the agent loop as a
//! standard `hivecore_runtime_core::Tool`, so extension-supplied tools are
//! indistinguishable from builtins at the loop level.
//!
//! Why we don't reuse Zed's `extension_host`:
//!   - Zed's host is GPL-3.0 + drags in 30+ Zed-internal crates (gpui,
//!     language, fs, dap, project, …). See `crates/extension_host/Cargo.toml`
//!     in zed-industries/zed.
//!   - Our boundary is multi-tenant + scoped to tools/audit/KG, not LSP/DAP.
//!
//! Borrowed from Zed (Apache-2.0 portions only):
//!   - The `since_vX.Y.Z/` versioned WIT directory pattern (ADR-017).
//!   - The "extension declares its tools, host adapts to native trait"
//!     architecture.

#![deny(missing_debug_implementations)]
#![warn(rust_2018_idioms, unreachable_pub)]

pub mod bindings;
pub mod error;
pub mod host;
pub mod loader;
pub mod tool_adapter;

pub use error::ExtensionError;
pub use host::{HostState, SettingsProvider};
pub use loader::{Extension, ExtensionLoader};
pub use tool_adapter::ExtensionTool;
