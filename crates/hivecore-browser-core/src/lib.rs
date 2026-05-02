//! `hivecore-browser-core` — Layer 2 browser-provider trait surface.
//!
//! ADR-028. Pure types, no I/O. Lifted from jcode's `Browser Provider Protocol`
//! (MIT) — see `.planning/intel/browser-harness.md` and ADR-024 vendoring rules.
//!
//! Concrete provider impls (chromiumoxide, WebDriver-BiDi, Playwright-MCP-as-
//! provider) live in sibling crates that depend on this one.

#![deny(missing_debug_implementations)]
#![warn(rust_2018_idioms, unreachable_pub)]

mod actions;
mod capability;
mod context;
mod error;
pub mod provider;
mod snapshot;

pub use actions::{ActionKind, ActionOutcome, AssertPredicate, WaitCondition};
pub use capability::{Capability, CapabilityStability, CapabilityTable};
pub use context::{BrowserContext, BrowserSessionId, TenantId};
pub use error::{BrowserError, BrowserResult};
pub use provider::BrowserProvider;
pub use snapshot::{ElementRef, Snapshot, SnapshotElement, SnapshotMeta, SnapshotVersion};
