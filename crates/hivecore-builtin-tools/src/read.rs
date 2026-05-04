//! `read_file` — read text from a workspace file with optional line slice.
//!
//! Schema chosen to match Claude Code / Codex `read_file` so the model can
//! reuse training-time familiarity:
//!     { path: string, offset?: u32, limit?: u32 }

use std::sync::Arc;

use async_trait::async_trait;
use hivecore_runtime_core::{
    AbortSignal, ContentBlock, ExecutionEnv, RuntimeError, RuntimeResult, Tool, ToolInvocation,
    ToolOutcome, UpdateSink,
};
use serde::Deserialize;

use crate::error::ToolError;
use crate::safety::WorkspaceRoot;

const DEFAULT_LIMIT: u32 = 2000;

#[derive(Debug, Clone)]
pub struct ReadTool {
    root: WorkspaceRoot,
    env: Arc<dyn ExecutionEnv>,
}

impl ReadTool {
    pub fn new(root: WorkspaceRoot, env: Arc<dyn ExecutionEnv>) -> Self {
        Self { root, env }
    }
}

#[derive(Debug, Deserialize)]
struct Args {
    path: String,
    #[serde(default)]
    offset: Option<u32>,
    #[serde(default)]
    limit: Option<u32>,
}

#[async_trait]
impl Tool for ReadTool {
    fn name(&self) -> &str {
        "read_file"
    }
    fn description(&self) -> &str {
        "Read text from a file in the workspace. Returns at most `limit` lines starting at `offset` (1-based, defaults to whole file up to 2000 lines)."
    }
    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "workspace-relative path"},
                "offset": {"type": "integer", "minimum": 1, "description": "1-based line to start at"},
                "limit": {"type": "integer", "minimum": 1, "description": "max lines to return"}
            },
            "required": ["path"]
        })
    }
    async fn execute(
        &self,
        invocation: ToolInvocation,
        signal: AbortSignal,
        _on_update: UpdateSink,
    ) -> RuntimeResult<ToolOutcome> {
        if signal.is_aborted() {
            return Err(ToolError::Aborted.into());
        }
        let args: Args = serde_json::from_value(invocation.input.clone())
            .map_err(|e| ToolError::InvalidArg(e.to_string()))?;
        let path = self.root.resolve(&args.path).map_err(runtime_err)?;
        let bytes = self.env.read_file(&path).await.map_err(|e| {
            if is_not_found(&e) {
                runtime_err(ToolError::NotFound(args.path.clone()))
            } else {
                e
            }
        })?;
        let text = String::from_utf8_lossy(&bytes);
        let offset = args.offset.unwrap_or(1).max(1) as usize - 1;
        let limit = args.limit.unwrap_or(DEFAULT_LIMIT) as usize;
        let mut out = String::new();
        let mut count = 0;
        for (i, line) in text.lines().enumerate().skip(offset).take(limit) {
            out.push_str(&format!("{:>6}\t{}\n", i + 1, line));
            count += 1;
        }
        let total = text.lines().count();
        let summary = format!(
            "{} ({} of {} lines, offset={})",
            args.path,
            count,
            total,
            offset + 1
        );

        Ok(ToolOutcome {
            content: vec![ContentBlock::Text { text: out }],
            details: Some(
                serde_json::json!({"summary": summary, "line_count": count, "total_lines": total}),
            ),
            is_error: false,
        })
    }
}

fn runtime_err(e: ToolError) -> hivecore_runtime_core::RuntimeError {
    e.into()
}

fn is_not_found(e: &RuntimeError) -> bool {
    matches!(e, RuntimeError::Other(msg) if msg.contains("No such file"))
}

#[cfg(test)]
#[path = "read_tests.rs"]
mod tests;
