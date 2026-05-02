//! `AgentsMdSource` — Codex's AGENTS.md aggregation, re-implemented.
//!
//! Spec (verbatim from Codex's base instructions, see
//! `prompts/codex-base.md` § "AGENTS.md spec"):
//!
//! > The contents of the AGENTS.md file at the root of the repo and any
//! > directories from the CWD up to the root are included with the
//! > developer message and don't need to be re-read.
//!
//! Implementation mirrors that:
//!   - Walk from `repo_root` → ... → `cwd`, collecting every `AGENTS.md`
//!     (and `AGENTS.override.md` if present — overrides take precedence,
//!     mirroring Codex's behaviour).
//!   - Concatenate in walk order. Cap total bytes at 32 KiB (matches
//!     Codex's documented cap; configurable here).
//!   - Skip files that are missing or unreadable.
//!
//! Hivecore additions:
//!   - Files are wrapped in `<agents_md path="...">` blocks so the model
//!     knows where each instruction came from.
//!   - Tenant-scoped paths are honoured upstream — the source itself only
//!     reads what the caller hands it.

use std::path::{Path, PathBuf};

use async_trait::async_trait;

use crate::error::PromptError;
use crate::source::{ContextSource, RoleHint, SourceFragment};

const DEFAULT_BUDGET_BYTES: usize = 32 * 1024;

#[derive(Debug, Clone)]
pub struct AgentsMdSource {
    pub repo_root: PathBuf,
    pub cwd: PathBuf,
    pub budget_bytes: usize,
}

impl AgentsMdSource {
    pub fn new(repo_root: impl Into<PathBuf>, cwd: impl Into<PathBuf>) -> Self {
        Self {
            repo_root: repo_root.into(),
            cwd: cwd.into(),
            budget_bytes: DEFAULT_BUDGET_BYTES,
        }
    }

    pub fn with_budget(mut self, bytes: usize) -> Self {
        self.budget_bytes = bytes;
        self
    }

    /// Walk repo_root → cwd, collecting AGENTS.md (and AGENTS.override.md)
    /// at each level. Returns paths in walk order (root first).
    fn walk(&self) -> Vec<PathBuf> {
        let mut walked = Vec::<PathBuf>::new();

        // Build the directory chain root → ... → cwd.
        let chain = directory_chain(&self.repo_root, &self.cwd);

        for dir in chain {
            for name in ["AGENTS.override.md", "AGENTS.md"] {
                let candidate = dir.join(name);
                if candidate.is_file() {
                    walked.push(candidate);
                }
            }
        }
        walked
    }
}

fn directory_chain(root: &Path, cwd: &Path) -> Vec<PathBuf> {
    // Canonicalize what we can; fall back to literal paths when the FS
    // hasn't created them yet (used in tests).
    let root_canon = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let cwd_canon = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());

    if !cwd_canon.starts_with(&root_canon) {
        // cwd is outside the repo — only emit the root.
        return vec![root_canon];
    }

    let mut chain = vec![root_canon.clone()];
    let mut current = root_canon.clone();
    if let Ok(rel) = cwd_canon.strip_prefix(&root_canon) {
        for component in rel.components() {
            current = current.join(component);
            chain.push(current.clone());
        }
    }
    chain
}

#[async_trait]
impl ContextSource for AgentsMdSource {
    fn name(&self) -> &str {
        "agents_md"
    }
    async fn fragment(&self) -> Result<Option<SourceFragment>, PromptError> {
        let paths = self.walk();
        if paths.is_empty() {
            return Ok(None);
        }

        let mut body = String::new();
        let mut consumed = 0usize;
        for path in paths {
            let text = match std::fs::read_to_string(&path) {
                Ok(t) => t,
                Err(_) => continue,
            };
            let header = format!("<agents_md path=\"{}\">\n", path.display());
            let footer = "\n</agents_md>\n";
            let chunk_size = header.len() + text.len() + footer.len();
            if consumed + chunk_size > self.budget_bytes {
                // Truncate; emit a marker so the model knows.
                body.push_str("<!-- AGENTS.md aggregation truncated to budget -->\n");
                break;
            }
            body.push_str(&header);
            body.push_str(&text);
            body.push_str(footer);
            consumed += chunk_size;
        }

        if body.is_empty() {
            return Ok(None);
        }

        Ok(Some(SourceFragment {
            source: "agents_md".into(),
            role_hint: RoleHint::User,
            body,
        }))
    }
}

#[cfg(test)]
#[path = "agents_md_tests.rs"]
mod tests;
