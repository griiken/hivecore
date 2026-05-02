//! End-to-end-ish: a skill with `agent: <name>` frontmatter routes through
//! a `SubAgentRouter`, returning the sub-agent's reply instead of the
//! rendered body. The router here is a stub — real wiring lives in the
//! ACP server, which uses `agent-loop`'s `SpawnAgentTool`.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use hivecore_runtime_core::{
    AbortSignal, ContentBlock, RuntimeResult, ToolCallId, ToolInvocation, UpdateSink,
};
use hivecore_skills::{render::RenderCtx, SkillLoader, SkillRegistry, SubAgentRouter};
use tempfile::TempDir;

#[derive(Debug, Default)]
struct StubRouter {
    received: Mutex<Vec<(String, String)>>,
}

#[async_trait]
impl SubAgentRouter for StubRouter {
    async fn run(
        &self,
        agent_name: &str,
        prompt: String,
        _signal: AbortSignal,
    ) -> RuntimeResult<String> {
        self.received
            .lock()
            .unwrap()
            .push((agent_name.into(), prompt.clone()));
        // Pretend the sub-agent reads the prompt and reports back.
        Ok(format!(
            "subagent[{}] saw {} chars",
            agent_name,
            prompt.len()
        ))
    }
}

const SKILL_WITH_AGENT: &str = "---
name: pr-summary
description: Summarise a PR using the explore subagent.
agent: explore
context: fork
---
This is the rendered prompt body.
";

const SKILL_INLINE: &str = "---
name: greet
description: Inline greeting.
---
Hello $0.
";

#[tokio::test]
async fn skill_with_agent_routes_to_subagent() {
    let d = TempDir::new().unwrap();
    std::fs::write(d.path().join("pr-summary.md"), SKILL_WITH_AGENT).unwrap();
    std::fs::write(d.path().join("greet.md"), SKILL_INLINE).unwrap();
    let skills = SkillLoader::new().add_dir(d.path()).load().unwrap();
    let reg = SkillRegistry::from_skills(skills).unwrap();

    let stub = Arc::new(StubRouter::default());
    let router: Arc<dyn SubAgentRouter> = stub.clone();
    let tools = reg.model_invokable_tools_with_router(RenderCtx::default, Some(router));
    assert_eq!(tools.len(), 2);

    let pr_summary = tools.iter().find(|t| t.name() == "pr-summary").unwrap();
    let greet = tools.iter().find(|t| t.name() == "greet").unwrap();

    let (_h, sig) = AbortSignal::new();

    // Skill with `agent:` → router invoked, output is the sub-agent's reply.
    let out = pr_summary
        .execute(
            ToolInvocation {
                id: ToolCallId("c1".into()),
                name: "pr-summary".into(),
                input: serde_json::json!({}),
            },
            sig.clone(),
            UpdateSink::noop(),
        )
        .await
        .unwrap();
    let txt = match &out.content[0] {
        ContentBlock::Text { text } => text.clone(),
        _ => panic!(),
    };
    assert!(txt.starts_with("subagent[explore] saw"));

    let received = stub.received.lock().unwrap().clone();
    assert_eq!(received.len(), 1);
    assert_eq!(received[0].0, "explore");
    assert!(received[0].1.contains("rendered prompt body"));

    // Skill without `agent:` → returns rendered body inline. Router NOT
    // invoked again.
    let out = greet
        .execute(
            ToolInvocation {
                id: ToolCallId("c2".into()),
                name: "greet".into(),
                input: serde_json::json!({"positional": ["alice"]}),
            },
            sig,
            UpdateSink::noop(),
        )
        .await
        .unwrap();
    let txt = match &out.content[0] {
        ContentBlock::Text { text } => text.clone(),
        _ => panic!(),
    };
    assert_eq!(txt.trim(), "Hello alice.");
    assert_eq!(stub.received.lock().unwrap().len(), 1); // unchanged
}
