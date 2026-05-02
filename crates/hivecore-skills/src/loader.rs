//! Skill loader. Walks a directory tree, parses every `*.md` (and the
//! Claude-Code-style `<dir>/<skill>/SKILL.md` layout), and returns
//! `Vec<Skill>`.
//!
//! Discovery rules:
//!   - File ending in `.md` whose first non-BOM line is `---` is treated as
//!     a candidate skill.
//!   - Frontmatter must contain `name` and `description`.
//!   - `name` validated against `^[a-z][a-z0-9_-]*$`.
//!   - Hidden directories (`.git`, `.cache`, …) are skipped, except the
//!     search root itself.

use std::path::{Path, PathBuf};

use walkdir::{DirEntry, WalkDir};

use crate::error::SkillError;
use crate::frontmatter;
use crate::skill::{Skill, SkillFrontmatter};

#[derive(Debug, Default, Clone)]
pub struct SkillLoader {
    paths: Vec<PathBuf>,
}

impl SkillLoader {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.paths.push(path.into());
        self
    }

    pub fn load(&self) -> Result<Vec<Skill>, SkillError> {
        let mut out = Vec::new();
        for root in &self.paths {
            if !root.exists() {
                continue;
            }
            for entry in WalkDir::new(root)
                .follow_links(false)
                .into_iter()
                .filter_entry(|e| !is_hidden(e))
            {
                let entry = match entry {
                    Ok(e) => e,
                    Err(_) => continue,
                };
                if !entry.file_type().is_file() {
                    continue;
                }
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) != Some("md") {
                    continue;
                }
                if let Some(skill) = load_one(path)? {
                    out.push(skill);
                }
            }
        }
        Ok(out)
    }
}

fn is_hidden(entry: &DirEntry) -> bool {
    if entry.depth() == 0 {
        return false;
    }
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}

fn load_one(path: &Path) -> Result<Option<Skill>, SkillError> {
    let text = std::fs::read_to_string(path)?;
    let display = path.display().to_string();
    let (yaml, body) = frontmatter::split(&text, &display)?;
    let Some(yaml) = yaml else {
        return Ok(None); // not a skill, just a markdown file
    };

    let mut fm: SkillFrontmatter =
        serde_yaml::from_str(yaml).map_err(|e| SkillError::BadFrontmatter {
            path: display.clone(),
            message: e.to_string(),
        })?;

    if fm.name.is_empty() {
        return Err(SkillError::MissingField {
            path: display,
            field: "name",
        });
    }
    if fm.description.is_empty() {
        return Err(SkillError::MissingField {
            path: display,
            field: "description",
        });
    }
    if !valid_name(&fm.name) {
        return Err(SkillError::InvalidName(fm.name));
    }
    fm.name = fm.name.to_string();

    Ok(Some(Skill {
        name: fm.name.clone(),
        description: fm.description.clone(),
        source_path: path.to_path_buf(),
        body_template: body.to_string(),
        frontmatter: fm,
    }))
}

fn valid_name(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

#[cfg(test)]
#[path = "loader_tests.rs"]
mod tests;
