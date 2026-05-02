//! Skill registry. Holds the loaded skills, enforces unique names, and
//! produces the `SkillTool` adapters that go into `ToolRegistry`.

use std::collections::HashMap;
use std::sync::Arc;

use crate::error::SkillError;
use crate::render::RenderCtx;
use crate::skill::Skill;
use crate::tool::{SkillTool, SubAgentRouter};

#[derive(Debug, Default, Clone)]
pub struct SkillRegistry {
    skills: HashMap<String, Arc<Skill>>,
    order: Vec<String>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_skills(skills: Vec<Skill>) -> Result<Self, SkillError> {
        let mut reg = Self::new();
        for s in skills {
            reg.insert(s)?;
        }
        Ok(reg)
    }

    pub fn insert(&mut self, skill: Skill) -> Result<(), SkillError> {
        if let Some(existing) = self.skills.get(&skill.name) {
            return Err(SkillError::Duplicate {
                name: skill.name,
                first: existing.source_path.display().to_string(),
                second: skill.source_path.display().to_string(),
            });
        }
        self.order.push(skill.name.clone());
        self.skills.insert(skill.name.clone(), Arc::new(skill));
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<Arc<Skill>> {
        self.skills.get(name).cloned()
    }

    pub fn len(&self) -> usize {
        self.skills.len()
    }

    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }

    pub fn names(&self) -> &[String] {
        &self.order
    }

    /// Build one `SkillTool` per skill that's allowed model-invocation. Use
    /// this to plug skills into an `AgentLoop`'s `ToolRegistry`.
    pub fn model_invokable_tools(
        &self,
        ctx_factory: impl Fn() -> RenderCtx + Send + Sync + 'static,
    ) -> Vec<Arc<dyn hivecore_runtime_core::Tool>> {
        self.model_invokable_tools_with_router(ctx_factory, None)
    }

    /// Same as `model_invokable_tools` but plumbs a `SubAgentRouter` into
    /// each tool, so skills with `agent: <name>` frontmatter delegate to a
    /// child agent instead of returning the rendered body inline.
    pub fn model_invokable_tools_with_router(
        &self,
        ctx_factory: impl Fn() -> RenderCtx + Send + Sync + 'static,
        router: Option<Arc<dyn SubAgentRouter>>,
    ) -> Vec<Arc<dyn hivecore_runtime_core::Tool>> {
        let factory = Arc::new(ctx_factory);
        self.order
            .iter()
            .filter_map(|name| self.skills.get(name))
            .filter(|s| !s.frontmatter.disable_model_invocation)
            .map(|s| {
                let factory = factory.clone();
                let mut tool = SkillTool::new(s.clone(), Arc::new(move || (factory)()));
                if let Some(r) = &router {
                    tool = tool.with_router(r.clone());
                }
                let arc: Arc<dyn hivecore_runtime_core::Tool> = Arc::new(tool);
                arc
            })
            .collect()
    }
}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
