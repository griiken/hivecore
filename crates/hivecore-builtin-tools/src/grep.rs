//! `grep` — regex search across the workspace. Returns matching lines with
//! filename + line number. Bounded by `max_matches`.
//!
//! All filesystem traversal routes through `ExecutionEnv` (ADR-031), so
//! sandbox / remote envs work transparently. Hidden directories (`.` prefix,
//! depth ≥ 1) are skipped to avoid leaking VCS / cache state.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use hivecore_runtime_core::{
    AbortSignal, ContentBlock, ExecutionEnv, RuntimeResult, Tool, ToolInvocation, ToolOutcome,
    UpdateSink,
};
use regex::Regex;
use serde::Deserialize;

use crate::error::ToolError;
use crate::safety::WorkspaceRoot;

const DEFAULT_MAX_MATCHES: usize = 200;

#[derive(Debug, Clone)]
pub struct GrepTool {
    root: WorkspaceRoot,
    env: Arc<dyn ExecutionEnv>,
}

impl GrepTool {
    pub fn new(root: WorkspaceRoot, env: Arc<dyn ExecutionEnv>) -> Self {
        Self { root, env }
    }
}

#[derive(Debug, Deserialize)]
struct Args {
    pattern: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    case_insensitive: bool,
    #[serde(default)]
    max_matches: Option<usize>,
}

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }
    fn description(&self) -> &str {
        "Search files for a regex. Returns up to `max_matches` `path:line:text` rows."
    }
    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {"type": "string"},
                "path": {"type": "string", "description": "subdir to search; default = workspace root"},
                "case_insensitive": {"type": "boolean", "default": false},
                "max_matches": {"type": "integer", "minimum": 1, "default": 200}
            },
            "required": ["pattern"]
        })
    }
    async fn execute(
        &self,
        invocation: ToolInvocation,
        signal: AbortSignal,
        _on_update: UpdateSink,
    ) -> RuntimeResult<ToolOutcome> {
        let args: Args = serde_json::from_value(invocation.input.clone())
            .map_err(|e| ToolError::InvalidArg(e.to_string()))?;
        let pattern = if args.case_insensitive {
            format!("(?i){}", args.pattern)
        } else {
            args.pattern.clone()
        };
        let re = Regex::new(&pattern).map_err(|e| rt(ToolError::Regex(e)))?;
        let search_root = match args.path.as_deref() {
            Some(rel) => self.root.resolve(rel).map_err(rt)?,
            None => self.root.path().to_path_buf(),
        };
        let max = args.max_matches.unwrap_or(DEFAULT_MAX_MATCHES);

        let mut hits: Vec<String> = Vec::new();
        let mut truncated = false;
        let mut stack: Vec<PathBuf> = vec![search_root.clone()];

        while let Some(dir) = stack.pop() {
            if signal.is_aborted() {
                return Err(ToolError::Aborted.into());
            }
            if hits.len() >= max {
                truncated = true;
                break;
            }
            let entries = match self.env.list_dir(&dir).await {
                Ok(es) => es,
                Err(_) => continue,
            };
            for entry in entries {
                if hits.len() >= max {
                    truncated = true;
                    break;
                }
                if is_hidden(&entry) {
                    continue;
                }
                if !entry.starts_with(self.root.path()) {
                    continue;
                }
                let stat = match self.env.stat(&entry).await {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                if stat.is_symlink {
                    continue;
                }
                if stat.is_dir {
                    stack.push(entry);
                    continue;
                }
                if !stat.is_file {
                    continue;
                }
                let text = match self.env.read_text_file(&entry).await {
                    Ok(t) => t,
                    Err(_) => continue,
                };
                let rel = entry.strip_prefix(self.root.path()).unwrap_or(&entry);
                for (lineno, line) in text.lines().enumerate() {
                    if re.is_match(line) {
                        hits.push(format!("{}:{}:{}", rel.display(), lineno + 1, line));
                        if hits.len() >= max {
                            truncated = true;
                            break;
                        }
                    }
                }
            }
        }

        let summary = format!(
            "{} match{} ({})",
            hits.len(),
            if hits.len() == 1 { "" } else { "es" },
            if truncated { "truncated" } else { "complete" }
        );

        Ok(ToolOutcome {
            content: vec![ContentBlock::Text {
                text: hits.join("\n"),
            }],
            details: Some(
                serde_json::json!({"summary": summary, "matches": hits.len(), "truncated": truncated}),
            ),
            is_error: false,
        })
    }
}

fn is_hidden(p: &Path) -> bool {
    p.file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}

fn rt(e: ToolError) -> hivecore_runtime_core::RuntimeError {
    e.into()
}

#[cfg(test)]
#[path = "grep_tests.rs"]
mod tests;
