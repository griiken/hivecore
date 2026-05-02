//! `hivecore-browser-harness` — Layer 3 typed `Tool` impls over a
//! `BrowserProvider` (`hivecore-browser-core`).
//!
//! ADR-028. The model-facing tool surface. `browser_act` collapses
//! click/type/hover/etc. into one `kind`-enum tool to keep the tool list
//! compact (resolved open-question §2 in `crates/hivecore-browser-harness/AGENTS.md`).

#![warn(rust_2018_idioms, unreachable_pub)]

mod state;
mod tools;

pub use state::{BrowserHarness, HarnessState};
pub use tools::{
    BrowserActTool, BrowserAssertTool, BrowserNavigateTool, BrowserScreenshotTool,
    BrowserSessionTool, BrowserSnapshotTool, BrowserWaitTool,
};

use std::sync::Arc;

use hivecore_runtime_core::Tool;

/// Build the v0.1 default browser-tool set, all bound to `harness`.
/// Returns 7 typed `Tool` impls (Open / List / Snapshot / Act / Navigate /
/// Wait / Assert / Screenshot — `browser_act` collapses 10 primitives into
/// one tool).
pub fn default_tools(harness: Arc<BrowserHarness>) -> Vec<Arc<dyn Tool>> {
    vec![
        Arc::new(BrowserSessionTool::new(harness.clone())),
        Arc::new(BrowserNavigateTool::new(harness.clone())),
        Arc::new(BrowserSnapshotTool::new(harness.clone())),
        Arc::new(BrowserActTool::new(harness.clone())),
        Arc::new(BrowserWaitTool::new(harness.clone())),
        Arc::new(BrowserAssertTool::new(harness.clone())),
        Arc::new(BrowserScreenshotTool::new(harness)),
    ]
}
