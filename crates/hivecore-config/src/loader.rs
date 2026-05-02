//! TOML loader. Reads a single file or a directory of agent definitions,
//! validates each, and returns `Agent` values. The raw schema (`raw::*`)
//! is private: callers see only the validated shape.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::agent::{Agent, Limits, ModelSelection, ToolMode, ToolPolicy};
use crate::error::{ConfigError, ValidationError};

#[derive(Debug, Default, Clone)]
pub struct AgentLoader {
    base_dir: Option<PathBuf>,
}

impl AgentLoader {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.base_dir = Some(dir.into());
        self
    }

    /// Load and validate a single agent file.
    pub fn load_file(&self, path: impl AsRef<Path>) -> Result<Agent, ConfigError> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path)?;
        let raw: raw::Root = toml::from_str(&text)?;
        let resolve_dir = path.parent().unwrap_or_else(|| Path::new("."));
        Ok(self.validate(raw.agent, resolve_dir)?)
    }

    /// Load every `*.toml` file in `dir` (non-recursive). Empty dir is fine.
    pub fn load_dir(&self, dir: impl AsRef<Path>) -> Result<Vec<Agent>, ConfigError> {
        let dir = dir.as_ref();
        let mut out = Vec::new();
        if !dir.exists() {
            return Ok(out);
        }
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("toml") {
                continue;
            }
            out.push(self.load_file(&path)?);
        }
        Ok(out)
    }

    fn validate(&self, raw: raw::Agent, resolve_dir: &Path) -> Result<Agent, ValidationError> {
        if !valid_id(&raw.id) {
            return Err(ValidationError::InvalidId(raw.id));
        }

        let model = ModelSelection {
            provider: raw.model.provider.clone(),
            id: raw.model.id.clone(),
        };
        if model.provider != "openai" {
            return Err(ValidationError::UnsupportedProvider(model.provider));
        }
        if model.id.is_empty() {
            return Err(ValidationError::MissingModelId);
        }

        // system_prompt: exactly one of inline / path.
        let system_prompt = match (raw.system_prompt.inline, raw.system_prompt.path) {
            (Some(inline), None) => inline,
            (None, Some(rel)) => {
                let candidate = resolve_dir.join(&rel);
                std::fs::read_to_string(&candidate).map_err(|_| {
                    ValidationError::PromptPathMissing(candidate.display().to_string())
                })?
            }
            _ => return Err(ValidationError::PromptSourceAmbiguous),
        };

        let mode = match raw.tools.mode.as_str() {
            "allowlist" => ToolMode::Allowlist,
            "all" => ToolMode::All,
            "denylist" => ToolMode::Denylist,
            other => return Err(ValidationError::InvalidToolMode(other.to_string())),
        };
        if matches!(mode, ToolMode::All) && !raw.tools.list.is_empty() {
            return Err(ValidationError::ToolListWithAllMode);
        }
        let tools = ToolPolicy {
            mode,
            list: raw.tools.list,
        };

        if raw.limits.max_iterations < 1 {
            return Err(ValidationError::BadIterationLimit(
                raw.limits.max_iterations,
            ));
        }

        Ok(Agent {
            id: raw.id,
            name: raw.name,
            version: raw.version,
            description: raw.description.unwrap_or_default(),
            model,
            system_prompt,
            tools,
            limits: Limits {
                max_iterations: raw.limits.max_iterations,
            },
        })
    }
}

fn valid_id(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

mod raw {
    use super::*;

    #[derive(Debug, Deserialize)]
    pub(super) struct Root {
        pub agent: Agent,
    }

    #[derive(Debug, Deserialize)]
    pub(super) struct Agent {
        pub id: String,
        pub name: String,
        pub version: String,
        #[serde(default)]
        pub description: Option<String>,
        pub model: Model,
        pub system_prompt: SystemPrompt,
        pub tools: Tools,
        pub limits: Limits,
    }

    #[derive(Debug, Deserialize)]
    pub(super) struct Model {
        pub provider: String,
        pub id: String,
    }

    #[derive(Debug, Deserialize)]
    pub(super) struct SystemPrompt {
        #[serde(default)]
        pub inline: Option<String>,
        #[serde(default)]
        pub path: Option<String>,
    }

    #[derive(Debug, Deserialize)]
    pub(super) struct Tools {
        pub mode: String,
        #[serde(default)]
        pub list: Vec<String>,
    }

    #[derive(Debug, Deserialize)]
    pub(super) struct Limits {
        pub max_iterations: u32,
    }
}

#[cfg(test)]
#[path = "loader_tests.rs"]
mod tests;
