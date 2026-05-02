//! `Skill` — fully-loaded, validated record. Plain data; the engine that
//! turns a `Skill` into a `hivecore_runtime_core::Tool` lives in `tool.rs`.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Skill {
    pub name: String,
    pub description: String,
    /// Path of the source `.md` file. Used for `${SKILL_DIR}` expansion and
    /// helpful error messages.
    pub source_path: PathBuf,
    /// Raw markdown body, after frontmatter is stripped. Variables and
    /// `!`<cmd>`` placeholders are resolved at invocation time.
    pub body_template: String,
    pub frontmatter: SkillFrontmatter,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillFrontmatter {
    pub name: String,
    pub description: String,
    /// Named arguments. Order matters — index N also matches `$N` /
    /// `$ARGUMENTS[N]`. Mirrors Claude Code's `arguments: [issue, branch]`
    /// shape.
    #[serde(default)]
    pub arguments: Vec<String>,
    /// Block model invocation; only callable via explicit user `/skill-name`
    /// path. Hivecore implements this by hiding the skill's `Tool` from the
    /// agent's tool list when this is true.
    #[serde(default, rename = "disable-model-invocation")]
    pub disable_model_invocation: bool,
    /// Lock the user's invocation path off — only the model can pick this
    /// skill via its description. (Inverse of the above; Claude Code's
    /// `user-invocable: false`.)
    #[serde(default = "default_true", rename = "user-invocable")]
    pub user_invocable: bool,
    /// Tools the skill body assumes are available. Surfaced for harness UI
    /// and policy checks; not enforced inside the substrate.
    #[serde(default, rename = "allowed-tools")]
    pub allowed_tools: Vec<String>,
    /// Delegate the rendered skill body to a named sub-agent instead of
    /// returning it to the caller. Mirrors Claude Code's `agent:`
    /// frontmatter and Codex's `SubAgentSource` route. When set, the skill
    /// tool's result is the sub-agent's final assistant text, not the raw
    /// rendered body.
    #[serde(default)]
    pub agent: Option<String>,
    /// Fork into a fresh context even without a specific sub-agent name.
    /// Mirrors Claude Code's `context: fork`. v0.1: implemented by routing
    /// to a default sub-agent when no `agent` is set; harness picks the
    /// default. (Pure inline if neither is set.)
    #[serde(default)]
    pub context: Option<String>,
}

fn default_true() -> bool {
    true
}
