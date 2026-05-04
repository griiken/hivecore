//! TOML loader for `Role`. Parallel to `AgentLoader`. ADR-033.
//!
//! Schema:
//! ```toml
//! # .hivecore/roles/researcher.toml
//! name = "researcher"
//! description = "Read-only investigation; no file edits, no shell."
//! prompt = """
//! You are operating in researcher mode. ...
//! """
//! # Optional: replace agent's tool list at this overlay's scope.
//! tools = ["read_file", "grep", "list_dir"]
//! # Optional: override the agent's model preference (provider/id).
//! # model = "openai/gpt-5.4-nano"
//! ```

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{ConfigError, ValidationError};
use crate::role::Role;

#[derive(Debug, Default, Clone)]
pub struct RoleLoader {
    base_dir: Option<PathBuf>,
}

impl RoleLoader {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.base_dir = Some(dir.into());
        self
    }

    /// Load and validate a single role TOML file.
    pub fn load_file(&self, path: impl AsRef<Path>) -> Result<Role, ConfigError> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path)?;
        let raw: raw::Role = toml::from_str(&text)?;
        Ok(self.validate(raw)?)
    }

    /// Load every `*.toml` file in `dir` (non-recursive). Empty dir is fine.
    pub fn load_dir(&self, dir: impl AsRef<Path>) -> Result<Vec<Role>, ConfigError> {
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

    fn validate(&self, raw: raw::Role) -> Result<Role, ValidationError> {
        if !valid_name(&raw.name) {
            return Err(ValidationError::InvalidId(raw.name));
        }
        if raw.prompt.trim().is_empty() {
            return Err(ValidationError::EmptyRolePrompt);
        }
        Ok(Role {
            name: raw.name,
            description: raw.description.unwrap_or_default(),
            prompt: raw.prompt,
            tools: raw.tools,
            model: raw.model,
        })
    }
}

fn valid_name(s: &str) -> bool {
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
    pub(super) struct Role {
        pub name: String,
        #[serde(default)]
        pub description: Option<String>,
        pub prompt: String,
        #[serde(default)]
        pub tools: Option<Vec<String>>,
        #[serde(default)]
        pub model: Option<String>,
    }
}

#[cfg(test)]
#[path = "role_loader_tests.rs"]
mod tests;
