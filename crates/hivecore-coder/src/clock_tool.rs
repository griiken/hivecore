//! `Tool` impl returning the current UTC time. Custom tools register
//! identically to builtins — proves the `Tool` trait is the only contract
//! a harness needs to extend the agent's capability surface.

use async_trait::async_trait;
use chrono::Utc;
use hivecore_runtime_core::{
    AbortSignal, ContentBlock, RuntimeResult, Tool, ToolInvocation, ToolOutcome, UpdateSink,
};
use serde_json::json;

#[derive(Debug, Default)]
pub struct ClockTool;

#[async_trait]
impl Tool for ClockTool {
    fn name(&self) -> &str {
        "clock"
    }

    fn description(&self) -> &str {
        "Return the current UTC time as an ISO-8601 string."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        })
    }

    async fn execute(
        &self,
        _invocation: ToolInvocation,
        _signal: AbortSignal,
        _on_update: UpdateSink,
    ) -> RuntimeResult<ToolOutcome> {
        let now = Utc::now().to_rfc3339();
        Ok(ToolOutcome::ok(vec![ContentBlock::Text { text: now }]))
    }
}
