//! `SkillTool` — turns a `Skill` into a `hivecore_runtime_core::Tool`. The
//! tool's `description()` flows through to the model via the standard tool
//! list; on call, we render the skill body and return it as text.
//!
//! Effect: the model gets the skill *only* when it asks for it (lazy body
//! load), and the description is always available in the tool list (no
//! special prompt-injection plumbing needed).

use std::sync::Arc;

use async_trait::async_trait;
use hivecore_runtime_core::{
    AbortSignal, ContentBlock, RuntimeError, RuntimeResult, Tool, ToolInvocation, ToolOutcome,
    UpdateSink,
};

use crate::render::{render, InvocationArgs, RenderCtx};
use crate::skill::Skill;

/// Routes a rendered skill body into a sub-agent. Mirrors Claude Code's
/// `agent: <name>` semantics. The harness wires one of these when it has
/// a sub-agent registry available.
#[async_trait]
pub trait SubAgentRouter: Send + Sync + std::fmt::Debug {
    async fn run(
        &self,
        agent_name: &str,
        prompt: String,
        signal: AbortSignal,
    ) -> RuntimeResult<String>;
}

pub struct SkillTool {
    skill: Arc<Skill>,
    ctx_factory: Arc<dyn Fn() -> RenderCtx + Send + Sync>,
    router: Option<Arc<dyn SubAgentRouter>>,
}

impl SkillTool {
    pub fn new(skill: Arc<Skill>, ctx_factory: Arc<dyn Fn() -> RenderCtx + Send + Sync>) -> Self {
        Self {
            skill,
            ctx_factory,
            router: None,
        }
    }

    /// Plug a sub-agent router so skills with `agent:` frontmatter can fork.
    pub fn with_router(mut self, router: Arc<dyn SubAgentRouter>) -> Self {
        self.router = Some(router);
        self
    }
}

impl std::fmt::Debug for SkillTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SkillTool")
            .field("name", &self.skill.name)
            .finish()
    }
}

#[async_trait]
impl Tool for SkillTool {
    fn name(&self) -> &str {
        &self.skill.name
    }

    fn description(&self) -> &str {
        &self.skill.description
    }

    fn parameters(&self) -> serde_json::Value {
        // Build a JSON Schema describing positional + named args. Named args
        // come from the frontmatter `arguments:` list; everything else is
        // free-form via an optional `positional` array.
        let mut props = serde_json::Map::new();
        for arg in &self.skill.frontmatter.arguments {
            props.insert(arg.clone(), serde_json::json!({"type": "string"}));
        }
        props.insert(
            "positional".into(),
            serde_json::json!({
                "type": "array",
                "items": {"type": "string"},
                "description": "Positional arguments matched to $0..$N in the skill body."
            }),
        );
        serde_json::json!({
            "type": "object",
            "properties": props,
            "additionalProperties": true
        })
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
        let args = InvocationArgs::from_json(&invocation.input)
            .map_err(|e| RuntimeError::InvalidInput(e.to_string()))?;
        let ctx = (self.ctx_factory)();
        let rendered = render(&self.skill, &args, &ctx)
            .await
            .map_err(|e| RuntimeError::ToolFailed(e.to_string()))?;

        // If the skill names a sub-agent and we have a router, fork.
        if let (Some(agent_name), Some(router)) = (&self.skill.frontmatter.agent, &self.router) {
            let final_text = router.run(agent_name, rendered, signal).await?;
            return Ok(ToolOutcome {
                content: vec![ContentBlock::Text { text: final_text }],
                details: Some(serde_json::json!({
                    "skill": self.skill.name,
                    "delegated_to": agent_name,
                })),
                is_error: false,
            });
        }

        Ok(ToolOutcome {
            content: vec![ContentBlock::Text { text: rendered }],
            details: Some(serde_json::json!({"skill": self.skill.name})),
            is_error: false,
        })
    }
}
