//! Static tool-list exclusion (ADR-029 A2).
//!
//! Removes tools from the registry before the agent loop starts. Defense-in-
//! depth: model can't call a tool it can't see. Cheaper than `Deny` matcher
//! + smaller tool-list tokens.
//!
//! Continue.dev `extensions/cli/spec/permissions.md:5-9` ships three states
//! (`allow` / `ask` / `exclude`). Hivecore implements `allow`/`ask` via
//! `ApprovalHook`; `exclude` is this module — pure list filter, no trait.

use std::collections::HashSet;
use std::sync::Arc;

use hivecore_runtime_core::Tool;

/// Drop every tool whose name appears in `excluded`. Order is preserved.
pub fn apply_exclude<I, S>(tools: Vec<Arc<dyn Tool>>, excluded: I) -> Vec<Arc<dyn Tool>>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let set: HashSet<String> = excluded.into_iter().map(Into::into).collect();
    if set.is_empty() {
        return tools;
    }
    tools
        .into_iter()
        .filter(|t| !set.contains(t.name()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use hivecore_runtime_core::{
        AbortSignal, RuntimeResult, ToolInvocation, ToolOutcome, UpdateSink,
    };

    #[derive(Debug)]
    struct StubTool(&'static str);

    #[async_trait]
    impl Tool for StubTool {
        fn name(&self) -> &str {
            self.0
        }
        fn description(&self) -> &str {
            ""
        }
        fn parameters(&self) -> serde_json::Value {
            serde_json::json!({})
        }
        async fn execute(
            &self,
            _inv: ToolInvocation,
            _sig: AbortSignal,
            _u: UpdateSink,
        ) -> RuntimeResult<ToolOutcome> {
            Ok(ToolOutcome::ok(vec![]))
        }
    }

    fn tools() -> Vec<Arc<dyn Tool>> {
        vec![
            Arc::new(StubTool("read_file")),
            Arc::new(StubTool("write_file")),
            Arc::new(StubTool("bash")),
            Arc::new(StubTool("browser_act")),
        ]
    }

    fn names(v: &[Arc<dyn Tool>]) -> Vec<&str> {
        v.iter().map(|t| t.name()).collect()
    }

    #[test]
    fn empty_exclude_returns_all() {
        let out = apply_exclude(tools(), Vec::<&str>::new());
        assert_eq!(
            names(&out),
            ["read_file", "write_file", "bash", "browser_act"]
        );
    }

    #[test]
    fn exclude_drops_named_tools() {
        let out = apply_exclude(tools(), ["bash", "write_file"]);
        assert_eq!(names(&out), ["read_file", "browser_act"]);
    }

    #[test]
    fn exclude_unknown_name_is_noop() {
        let out = apply_exclude(tools(), ["nonexistent"]);
        assert_eq!(names(&out).len(), 4);
    }

    #[test]
    fn plan_mode_excludes_all_write_tools() {
        let out = apply_exclude(tools(), ["write_file", "bash", "browser_act"]);
        assert_eq!(names(&out), ["read_file"]);
    }
}
