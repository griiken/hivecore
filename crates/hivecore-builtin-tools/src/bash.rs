//! `bash` — execute a shell command in the workspace root.
//!
//! Default timeout: 120s. Honours `AbortSignal` via tokio kill. Captures
//! stdout + stderr separately. **No sandbox** in v0.1 — Layer 3 wraps this
//! tool with a hook that consults a Codex-style `execpolicy`.

use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use hivecore_runtime_core::{
    AbortSignal, ContentBlock, RuntimeResult, Tool, ToolInvocation, ToolOutcome, UpdateSink,
};
use serde::Deserialize;
use tokio::process::Command;
use tokio::time::timeout;

use crate::error::ToolError;
use crate::safety::WorkspaceRoot;

const DEFAULT_TIMEOUT_MS: u64 = 120_000;
const MAX_OUTPUT_BYTES: usize = 32 * 1024;

#[derive(Debug, Clone)]
pub struct BashTool {
    root: WorkspaceRoot,
}

impl BashTool {
    pub fn new(root: WorkspaceRoot) -> Self {
        Self { root }
    }
}

#[derive(Debug, Deserialize)]
struct Args {
    command: String,
    #[serde(default)]
    timeout_ms: Option<u64>,
    #[serde(default)]
    cwd: Option<String>,
}

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }
    fn description(&self) -> &str {
        "Run a shell command. Returns combined stdout+stderr (truncated at 32 KiB). Default timeout 120s."
    }
    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {"type": "string"},
                "timeout_ms": {"type": "integer", "minimum": 100},
                "cwd": {"type": "string", "description": "workspace-relative working directory"}
            },
            "required": ["command"]
        })
    }
    async fn execute(
        &self,
        invocation: ToolInvocation,
        signal: AbortSignal,
        on_update: UpdateSink,
    ) -> RuntimeResult<ToolOutcome> {
        let args: Args = serde_json::from_value(invocation.input.clone())
            .map_err(|e| ToolError::InvalidArg(e.to_string()))?;
        let dur = Duration::from_millis(args.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS));

        let cwd = match args.cwd.as_deref() {
            Some(rel) => self.root.resolve(rel).map_err(rt)?,
            None => self.root.path().to_path_buf(),
        };

        let mut cmd = Command::new("bash");
        cmd.arg("-lc")
            .arg(&args.command)
            .current_dir(&cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let child = cmd.spawn().map_err(|e| rt(ToolError::Io(e)))?;
        on_update.send(serde_json::json!({"phase":"started","cwd":cwd.display().to_string()}));
        if signal.is_aborted() {
            return Err(ToolError::Aborted.into());
        }

        let output = match timeout(dur, child.wait_with_output()).await {
            Ok(Ok(o)) => o,
            Ok(Err(e)) => return Err(rt(ToolError::Io(e))),
            Err(_) => return Err(rt(ToolError::Timeout(dur.as_millis() as u64))),
        };

        let mut combined = String::new();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stdout.is_empty() {
            combined.push_str(&stdout);
        }
        if !stderr.is_empty() {
            if !combined.is_empty() && !combined.ends_with('\n') {
                combined.push('\n');
            }
            combined.push_str("--- stderr ---\n");
            combined.push_str(&stderr);
        }
        let truncated = combined.len() > MAX_OUTPUT_BYTES;
        if truncated {
            combined.truncate(MAX_OUTPUT_BYTES);
            combined.push_str("\n…[truncated]");
        }

        let exit = output.status.code().unwrap_or(-1);
        let is_error = !output.status.success();

        Ok(ToolOutcome {
            content: vec![ContentBlock::Text { text: combined }],
            details: Some(serde_json::json!({
                "exit_code": exit,
                "truncated": truncated,
                "cwd": cwd.display().to_string(),
            })),
            is_error,
        })
    }
}

fn rt(e: ToolError) -> hivecore_runtime_core::RuntimeError {
    e.into()
}

#[cfg(test)]
#[path = "bash_tests.rs"]
mod tests;
