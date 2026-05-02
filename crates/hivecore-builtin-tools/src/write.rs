//! `write_file` — overwrite or create a file with the supplied content.

use async_trait::async_trait;
use hivecore_runtime_core::{
    AbortSignal, ContentBlock, RuntimeResult, Tool, ToolInvocation, ToolOutcome, UpdateSink,
};
use serde::Deserialize;

use crate::error::ToolError;
use crate::safety::WorkspaceRoot;

#[derive(Debug, Clone)]
pub struct WriteTool {
    root: WorkspaceRoot,
}

impl WriteTool {
    pub fn new(root: WorkspaceRoot) -> Self {
        Self { root }
    }
}

#[derive(Debug, Deserialize)]
struct Args {
    path: String,
    content: String,
}

#[async_trait]
impl Tool for WriteTool {
    fn name(&self) -> &str {
        "write_file"
    }
    fn description(&self) -> &str {
        "Overwrite or create a file in the workspace with the given content."
    }
    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "workspace-relative path"},
                "content": {"type": "string", "description": "full file contents to write"}
            },
            "required": ["path", "content"]
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
        let path = self.root.resolve(&args.path).map_err(rt)?;
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| rt(ToolError::Io(e)))?;
        }
        let bytes = args.content.len();
        tokio::fs::write(&path, &args.content)
            .await
            .map_err(|e| rt(ToolError::Io(e)))?;

        Ok(ToolOutcome {
            content: vec![ContentBlock::Text {
                text: format!("wrote {} bytes to {}", bytes, args.path),
            }],
            details: Some(serde_json::json!({"path": args.path, "bytes": bytes})),
            is_error: false,
        })
    }
}

fn rt(e: ToolError) -> hivecore_runtime_core::RuntimeError {
    e.into()
}

#[cfg(test)]
#[path = "write_tests.rs"]
mod tests;
