//! `edit_file` — exact-string replacement.
//!
//! Contract mirrors Claude Code's `Edit` tool: `old_string` must occur
//! exactly once in the file (unless `replace_all` is true). This forces the
//! model to provide enough surrounding context to disambiguate, and rejects
//! ambiguous edits before any I/O happens.

use async_trait::async_trait;
use hivecore_runtime_core::{
    AbortSignal, ContentBlock, RuntimeResult, Tool, ToolInvocation, ToolOutcome, UpdateSink,
};
use serde::Deserialize;

use crate::error::ToolError;
use crate::safety::WorkspaceRoot;

#[derive(Debug, Clone)]
pub struct EditTool {
    root: WorkspaceRoot,
}

impl EditTool {
    pub fn new(root: WorkspaceRoot) -> Self {
        Self { root }
    }
}

#[derive(Debug, Deserialize)]
struct Args {
    path: String,
    old_string: String,
    new_string: String,
    #[serde(default)]
    replace_all: bool,
}

#[async_trait]
impl Tool for EditTool {
    fn name(&self) -> &str {
        "edit_file"
    }
    fn description(&self) -> &str {
        "Replace exactly one occurrence of `old_string` with `new_string` in `path`. Set `replace_all=true` to replace every occurrence. Fails if `old_string` is missing or — without `replace_all` — occurs more than once."
    }
    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {"type": "string"},
                "old_string": {"type": "string"},
                "new_string": {"type": "string"},
                "replace_all": {"type": "boolean", "default": false}
            },
            "required": ["path", "old_string", "new_string"]
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
        if args.old_string == args.new_string {
            return Err(ToolError::InvalidArg("old_string == new_string".into()).into());
        }
        let path = self.root.resolve(&args.path).map_err(rt)?;
        let original = tokio::fs::read_to_string(&path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                rt(ToolError::NotFound(args.path.clone()))
            } else {
                rt(ToolError::Io(e))
            }
        })?;

        let count = original.matches(&args.old_string).count();
        if count == 0 {
            return Err(rt(ToolError::StringNotFound));
        }
        let new_contents = if args.replace_all {
            original.replace(&args.old_string, &args.new_string)
        } else {
            if count > 1 {
                return Err(rt(ToolError::StringNotUnique { count }));
            }
            original.replacen(&args.old_string, &args.new_string, 1)
        };

        tokio::fs::write(&path, new_contents.as_bytes())
            .await
            .map_err(|e| rt(ToolError::Io(e)))?;

        Ok(ToolOutcome {
            content: vec![ContentBlock::Text {
                text: format!(
                    "edited {} ({} occurrence{} replaced)",
                    args.path,
                    count,
                    if count == 1 { "" } else { "s" }
                ),
            }],
            details: Some(serde_json::json!({"path": args.path, "occurrences_replaced": count})),
            is_error: false,
        })
    }
}

fn rt(e: ToolError) -> hivecore_runtime_core::RuntimeError {
    e.into()
}

#[cfg(test)]
#[path = "edit_tests.rs"]
mod tests;
