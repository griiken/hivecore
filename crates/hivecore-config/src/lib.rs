//! Hivecore config (spike) — agents + global settings.
//!
//! Per CLAUDE.md and ADR-007: agents, workflows, and policies are CONFIG
//! (TOML), not code. This crate owns the schema, validation, and discovery
//! layer; downstream crates take fully-validated `Agent` values and never
//! parse TOML themselves.
//!
//! Industry alignment: Claude Code calls these "subagents" (relational
//! term) and stores them as `.claude/agents/<name>.md` with YAML
//! frontmatter. Codex / Zed / pi-mono / GSD-2 all call them "agents" too.
//! Hivecore picks the same noun. The TOML key is `[agent]` and the file
//! lives under `agents/`. Authors can still keep the system prompt in
//! markdown via `system_prompt.path = "swe.md"`.

#![deny(missing_debug_implementations)]
#![warn(rust_2018_idioms, unreachable_pub)]

pub mod agent;
pub mod error;
pub mod loader;
pub mod registry;
pub mod role;
pub mod role_loader;
pub mod role_registry;

pub use agent::{Agent, SkillPolicy, ToolMode};
pub use error::{ConfigError, ValidationError};
pub use loader::AgentLoader;
pub use registry::AgentRegistry;
pub use role::Role;
pub use role_loader::RoleLoader;
pub use role_registry::RoleRegistry;
