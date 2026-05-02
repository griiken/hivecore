//! ADR-026 — session compaction.
//!
//! Disk is append-only; compaction never mutates the on-disk log. Instead,
//! `SummarizingTransform` (`ContextTransform` impl) decides at each turn
//! whether the projected token cost exceeds threshold; if so, it calls a
//! summarizer model, builds a `compaction_marker` `Custom` message, and
//! returns it from `maybe_compact`. The driver pushes the marker into
//! `state.messages` (so disk records it via `MessageCommitted`), then the
//! same transform's `transform_outgoing` derives the actual model payload
//! by walking newest→oldest to the latest marker.
//!
//! Pi-mono shape (head-summary, keep tail) but with Codex's
//! "preserve real user messages" correction (see ADR-026 §Decisions).

#![warn(rust_2018_idioms, unreachable_pub)]

pub mod constants;
pub mod marker;
pub mod prompts;
pub mod token;
pub mod transform;

pub use constants::{CompactionConfig, DEFAULT_COMPACTION_CONFIG};
pub use marker::{CompactionMarker, CompactionTrigger, FileRef, MARKER_KIND};
pub use token::estimate_tokens;
pub use transform::SummarizingTransform;
