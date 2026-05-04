//! `Agent` — the validated, in-memory shape downstream crates consume.
//! Mirrors Claude Code's subagent file format and Codex's agent definition:
//! a name, a system prompt, a model selection, a tool policy, and limits.
//! TOML-side counterparts (`raw::*`) live in `loader.rs`.

use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

/// Validated agent definition, ready to feed into `AgentLoop::builder()`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub model: ModelSelection,
    pub system_prompt: String,
    pub tools: ToolPolicy,
    #[serde(default)]
    pub skills: SkillPolicy,
    pub limits: Limits,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelSelection {
    pub provider: String,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolPolicy {
    pub mode: ToolMode,
    pub list: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ToolMode {
    /// Only the names in `list` are exposed to the model.
    Allowlist,
    /// Every registered tool is exposed.
    All,
    /// Every tool *except* those in `list` is exposed.
    Denylist,
}

impl ToolPolicy {
    /// Decide whether `name` is permitted under this policy.
    pub fn allows(&self, name: &str) -> bool {
        match self.mode {
            ToolMode::All => true,
            ToolMode::Allowlist => self.list.iter().any(|n| n == name),
            ToolMode::Denylist => !self.list.iter().any(|n| n == name),
        }
    }

    /// Filter a registered tool name list down to what this agent accepts,
    /// preserving the input order.
    pub fn filter<'a, I>(&self, names: I) -> Vec<String>
    where
        I: IntoIterator<Item = &'a str>,
    {
        names
            .into_iter()
            .filter(|n| self.allows(n))
            .map(|s| s.to_string())
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Limits {
    pub max_iterations: u32,
}

/// Per-agent skill exposure. Defaults to `All` so dropping a skill into the
/// configured skill dir surfaces it to every agent unless explicitly tightened.
///
/// TOML shapes:
/// ```toml
/// skills = "*"                       # all (default; same as omitting the field)
/// skills = ["pdf-tools", "review"]   # allowlist — only these
/// ```
///
/// A `Deny { deny: [...] }` variant can slot in later without breaking either
/// shape above (serde untagged falls through to it on inline-table input).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum SkillPolicy {
    #[default]
    All,
    Allow(Vec<String>),
}

impl SkillPolicy {
    pub fn allows(&self, name: &str) -> bool {
        match self {
            Self::All => true,
            Self::Allow(list) => list.iter().any(|n| n == name),
        }
    }

    pub fn filter<'a, I>(&self, names: I) -> Vec<String>
    where
        I: IntoIterator<Item = &'a str>,
    {
        names
            .into_iter()
            .filter(|n| self.allows(n))
            .map(|s| s.to_string())
            .collect()
    }
}

impl Serialize for SkillPolicy {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::All => ser.serialize_str("*"),
            Self::Allow(list) => list.serialize(ser),
        }
    }
}

impl<'de> Deserialize<'de> for SkillPolicy {
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Str(String),
            List(Vec<String>),
        }
        match Raw::deserialize(de)? {
            Raw::Str(s) if s == "*" => Ok(Self::All),
            Raw::Str(other) => Err(de::Error::custom(format!(
                "skills must be \"*\" or a list of names (got `{other}`)"
            ))),
            Raw::List(v) => Ok(Self::Allow(v)),
        }
    }
}

#[cfg(test)]
#[path = "agent_tests.rs"]
mod tests;
