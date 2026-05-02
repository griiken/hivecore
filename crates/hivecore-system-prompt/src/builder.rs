//! The builder. Composes a list of `ContextSource`s into a single string
//! suitable for `ModelRequest.system`. Order is whatever order the caller
//! adds them — the builder doesn't reorder, because the *caller* knows
//! what the prefix-preservation invariant requires for their flow.

use std::sync::Arc;

use crate::error::PromptError;
use crate::source::{ContextSource, RoleHint, SourceFragment};

#[derive(Debug, Clone)]
pub struct SystemPromptOutput {
    /// The fully-rendered system prompt — concatenated source bodies with
    /// `\n\n` separators.
    pub system: String,
    /// The raw fragments (in order). Useful for harnesses that target a
    /// richer wire format (Responses API roles, etc.) than our
    /// single-string `ModelRequest.system`.
    pub fragments: Vec<SourceFragment>,
}

#[derive(Default)]
pub struct SystemPromptBuilder {
    sources: Vec<Arc<dyn ContextSource>>,
}

impl std::fmt::Debug for SystemPromptBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SystemPromptBuilder")
            .field(
                "sources",
                &self.sources.iter().map(|s| s.name()).collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl SystemPromptBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(clippy::should_implement_trait)] // builder pattern, not std::ops::Add
    pub fn add(mut self, source: Arc<dyn ContextSource>) -> Self {
        self.sources.push(source);
        self
    }

    pub fn add_static(
        self,
        name: impl Into<String>,
        role_hint: RoleHint,
        body: impl Into<String>,
    ) -> Self {
        self.add(Arc::new(crate::source::StaticSource::new(
            name, role_hint, body,
        )))
    }

    pub async fn build(&self) -> Result<SystemPromptOutput, PromptError> {
        let mut fragments = Vec::with_capacity(self.sources.len());
        for source in &self.sources {
            if let Some(fragment) = source.fragment().await? {
                fragments.push(fragment);
            }
        }

        let system = fragments
            .iter()
            .map(|f| f.body.clone())
            .collect::<Vec<_>>()
            .join("\n\n");

        Ok(SystemPromptOutput { system, fragments })
    }

    /// Convenience: assemble the canonical Codex-style five-layer prompt.
    /// The caller still chooses what to plug in for permissions /
    /// developer / agents-md / cwd — this just sets the order.
    pub fn codex_default(
        base_instructions: Arc<dyn ContextSource>,
        permissions: Option<Arc<dyn ContextSource>>,
        developer: Option<Arc<dyn ContextSource>>,
        agents_md: Option<Arc<dyn ContextSource>>,
        environment: Arc<dyn ContextSource>,
    ) -> Self {
        let mut b = Self::new().add(base_instructions);
        if let Some(p) = permissions {
            b = b.add(p);
        }
        if let Some(d) = developer {
            b = b.add(d);
        }
        if let Some(a) = agents_md {
            b = b.add(a);
        }
        b = b.add(environment);
        b
    }
}

#[cfg(test)]
#[path = "builder_tests.rs"]
mod tests;
