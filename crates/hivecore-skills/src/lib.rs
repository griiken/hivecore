//! Hivecore skills (spike).
//!
//! Per `.planning/intel/skills-subagents-survey.md`: skills converge on
//! markdown + YAML frontmatter + variables + bash injection + lazy-loaded
//! body. Hivecore's mapping: each `SKILL.md` becomes a `hivecore_runtime_core::Tool`.
//! The frontmatter `description` flows to the model via the tool list (the
//! same channel models already understand). Body is what the model gets back
//! when it invokes the tool — so body load *is* the tool call.
//!
//! Why-this-shape:
//!   - Description-always-loaded? The model already gets all tool descriptions
//!     in its prompt — same effect, no extra system-prompt bloat.
//!   - Body-lazy-loaded? It's the tool *result*, only sent on call.
//!   - Variables? Tool args feed the variable resolver.
//!   - `!`<cmd>`` bash injection? Tool execution time.
//!
//! Compared to Codex / Claude Code: those systems treat skills as a separate
//! prompt-injection mechanism. Hivecore reuses the existing tool channel.
//! Same author-facing format, simpler runtime.

#![deny(missing_debug_implementations)]
#![warn(rust_2018_idioms, unreachable_pub)]

pub mod error;
pub mod frontmatter;
pub mod loader;
pub mod registry;
pub mod render;
pub mod skill;
pub mod tool;
pub mod watcher;

pub use error::SkillError;
pub use loader::SkillLoader;
pub use registry::SkillRegistry;
pub use skill::{Skill, SkillFrontmatter};
pub use tool::{SkillTool, SubAgentRouter};
pub use watcher::{SharedSkillRegistry, SkillWatcher};
