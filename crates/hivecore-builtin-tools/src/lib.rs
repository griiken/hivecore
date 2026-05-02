//! Hivecore builtin tools (spike).
//!
//! Five tools that cover ~80% of agent-driven SWE work:
//!   - `read_file` — read text with offset/limit (Claude Code style).
//!   - `write_file` — overwrite or create a file.
//!   - `edit_file` — exact-string replacement (requires prior read in the
//!     same session — gate enforced via the `read_required` hook).
//!   - `bash`     — run a shell command with timeout + abort.
//!   - `grep`     — regex search across the workspace.
//!
//! All tools are workspace-rooted: paths must resolve under `WorkspaceRoot`
//! after canonicalisation. Escapes (`..`, symlinks crossing the boundary,
//! absolute paths outside root) are rejected before any I/O happens.

#![deny(missing_debug_implementations)]
#![warn(rust_2018_idioms, unreachable_pub)]

pub mod bash;
pub mod edit;
pub mod error;
pub mod grep;
pub mod read;
pub mod safety;
pub mod write;

use std::sync::Arc;

use hivecore_runtime_core::Tool;

pub use bash::BashTool;
pub use edit::EditTool;
pub use error::ToolError;
pub use grep::GrepTool;
pub use read::ReadTool;
pub use safety::WorkspaceRoot;
pub use write::WriteTool;

/// Returns the canonical builtin set bound to a workspace root.
pub fn default_set(root: WorkspaceRoot) -> Vec<Arc<dyn Tool>> {
    vec![
        Arc::new(ReadTool::new(root.clone())),
        Arc::new(WriteTool::new(root.clone())),
        Arc::new(EditTool::new(root.clone())),
        Arc::new(BashTool::new(root.clone())),
        Arc::new(GrepTool::new(root)),
    ]
}
