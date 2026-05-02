//! Hivecore layered system prompt builder.
//!
//! Composes the model's system prompt from N typed sources, in a
//! deterministic, cache-friendly order. Mirrors Codex's five-layer model
//! (see `.planning/intel/codex-agent-loop.md`):
//!
//! 1. **Base instructions** — the agent's core "who you are" text. We ship
//!    Codex's `default.md` (Apache-2.0) verbatim under `prompts/`.
//! 2. **Permissions** — sandbox + approval policy text.
//! 3. **Developer instructions** — operator-supplied (config.toml).
//! 4. **AGENTS.md aggregation** — files walked from project root → cwd.
//!    Codex spec re-implemented here (32 KiB cap, override files).
//! 5. **Environment context** — cwd, shell, date, etc.
//!
//! Per the prefix-preservation invariant (Codex blog), the builder always
//! emits sources in the same order so the prompt is a stable prefix across
//! turns. Mid-session changes (cwd flip, sandbox flip) are appended as new
//! sections rather than mutating earlier ones.
//!
//! ## Bundled prompts
//!
//! `prompts/codex-base.md` and `prompts/zed-template.hbs` are vendored
//! verbatim from upstream (see `prompts/README.md`). Hivecore reuses
//! Codex's text as the default `BASE_INSTRUCTIONS` source. Other harnesses
//! can supply their own.

#![deny(missing_debug_implementations)]
#![warn(rust_2018_idioms, unreachable_pub)]

pub mod agents_md;
pub mod builder;
pub mod environment;
pub mod error;
pub mod source;

pub use agents_md::AgentsMdSource;
pub use builder::{SystemPromptBuilder, SystemPromptOutput};
pub use environment::EnvironmentContextSource;
pub use error::PromptError;
pub use source::{ContextSource, RoleHint, SourceFragment, StaticSource};

/// Codex's `default.md` baseline, vendored verbatim. Source:
/// <https://github.com/openai/codex/blob/main/codex-rs/protocol/src/prompts/base_instructions/default.md>
/// License: Apache-2.0.
pub const CODEX_BASE_INSTRUCTIONS: &str = include_str!("../prompts/codex-base.md");

/// Zed's handlebars template, vendored for reference.
pub const ZED_SYSTEM_PROMPT_TEMPLATE: &str = include_str!("../prompts/zed-template.hbs");
