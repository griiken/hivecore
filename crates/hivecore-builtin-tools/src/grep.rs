//! `grep` — regex search across the workspace. Returns matching lines with
//! filename + line number. Bounded by `max_matches`.

use async_trait::async_trait;
use regex::Regex;
use hivecore_runtime_core::{
    AbortSignal, ContentBlock, RuntimeResult, Tool, ToolInvocation, ToolOutcome, UpdateSink,
};
use serde::Deserialize;

use crate::error::ToolError;
use crate::safety::WorkspaceRoot;

const DEFAULT_MAX_MATCHES: usize = 200;

#[derive(Debug, Clone)]
pub struct GrepTool {
    root: WorkspaceRoot,
}

impl GrepTool {
    pub fn new(root: WorkspaceRoot) -> Self {
        Self { root }
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

        // Walking the tree is sync; offload to a blocking task to keep the
        // async runtime responsive on large workspaces.
        let root = self.root.clone();
        let search_root = search_root.clone();
        let task = tokio::task::spawn_blocking(move || {
            let mut hits = Vec::<String>::new();
            for entry in walkdir::WalkDir::new(&search_root)
                .follow_links(false)
                .into_iter()
                .filter_entry(|e| !is_hidden(e))
            {
                if signal.is_aborted() {
                    return Err(ToolError::Aborted);
                }
                let entry = match entry {
                    Ok(e) => e,
                    Err(_) => continue,
                };
                if !entry.file_type().is_file() {
                    continue;
                }
                let path = entry.path();
                if !path.starts_with(root.path()) {
                    continue;
                }
                let Ok(text) = std::fs::read_to_string(path) else {
                    continue;
                };
                for (lineno, line) in text.lines().enumerate() {
                    if re.is_match(line) {
                        let rel = path.strip_prefix(root.path()).unwrap_or(path);
                        hits.push(format!("{}:{}:{}", rel.display(), lineno + 1, line));
                        if hits.len() >= max {
                            return Ok((hits, true));
                        }
                    }
                }
            }
            Ok((hits, false))
        });

        let (hits, truncated) = task
            .await
            .map_err(|e| rt(ToolError::InvalidArg(format!("join error: {e}"))))?
            .map_err(rt)?;

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

fn is_hidden(entry: &walkdir::DirEntry) -> bool {
    if entry.depth() == 0 {
        return false; // never skip the search root itself
    }
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}

fn rt(e: ToolError) -> hivecore_runtime_core::RuntimeError {
    e.into()
}

#[cfg(test)]
#[path = "grep_tests.rs"]
mod tests;
