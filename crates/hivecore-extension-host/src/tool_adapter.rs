//! Adapt a single extension-declared tool into `hivecore_runtime_core::Tool` so
//! it slots into the agent loop's `ToolRegistry` next to native builtins.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use hivecore_runtime_core::{
    AbortSignal, ContentBlock, RuntimeError, RuntimeResult, Tool, ToolInvocation, ToolOutcome,
    UpdateSink,
};

use crate::bindings::ToolSpec;
use crate::loader::Extension;

/// Wraps one tool from an extension. Multiple `ExtensionTool`s share the
/// underlying `Extension` via `Arc<Mutex<...>>` — the wasmtime store is
/// `!Send` across calls but `Mutex`-guarded sequential access works.
pub struct ExtensionTool {
    spec: ToolSpec,
    extension: Arc<Mutex<Extension>>,
}

impl std::fmt::Debug for ExtensionTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExtensionTool")
            .field("name", &self.spec.name)
            .finish()
    }
}

impl ExtensionTool {
    pub fn new(spec: ToolSpec, extension: Arc<Mutex<Extension>>) -> Self {
        Self { spec, extension }
    }

    pub fn from_extension(extension: Arc<Mutex<Extension>>) -> Vec<Self> {
        let specs = {
            let ext = extension.lock().expect("extension poisoned");
            ext.tools().to_vec()
        };
        specs
            .into_iter()
            .map(|s| Self::new(s, extension.clone()))
            .collect()
    }
}

#[async_trait]
impl Tool for ExtensionTool {
    fn name(&self) -> &str {
        &self.spec.name
    }

    fn description(&self) -> &str {
        &self.spec.description
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::from_str(&self.spec.parameters_json_schema)
            .unwrap_or_else(|_| serde_json::json!({"type":"object"}))
    }

    async fn execute(
        &self,
        invocation: ToolInvocation,
        signal: AbortSignal,
        _on_update: UpdateSink,
    ) -> RuntimeResult<ToolOutcome> {
        if signal.is_aborted() {
            return Err(RuntimeError::Aborted);
        }
        let args_json = invocation.input.to_string();
        let name = self.spec.name.clone();
        let ext = self.extension.clone();

        // wasmtime is sync; run on the blocking pool so we don't stall the
        // async runtime.
        let res = tokio::task::spawn_blocking(move || {
            let mut ext = ext.lock().expect("extension poisoned");
            ext.call_run_tool(&name, &args_json)
        })
        .await
        .map_err(|e| RuntimeError::ToolFailed(format!("join: {e}")))?
        .map_err(|e| RuntimeError::ToolFailed(e.to_string()))?;

        Ok(ToolOutcome {
            content: vec![ContentBlock::Text { text: res.content }],
            details: None,
            is_error: res.is_error,
        })
    }
}
