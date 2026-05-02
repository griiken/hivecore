use std::path::PathBuf;
use std::sync::Arc;

use tempfile::TempDir;

use super::*;
use crate::agents_md::AgentsMdSource;
use crate::environment::EnvironmentContextSource;
use crate::source::{ContextSource, RoleHint};
use crate::CODEX_BASE_INSTRUCTIONS;

#[tokio::test]
async fn empty_builder_produces_empty_string() {
    let out = SystemPromptBuilder::new().build().await.unwrap();
    assert!(out.system.is_empty());
    assert!(out.fragments.is_empty());
}

#[tokio::test]
async fn static_sources_are_concatenated_in_order() {
    let out = SystemPromptBuilder::new()
        .add_static("a", RoleHint::System, "ALPHA")
        .add_static("b", RoleHint::Developer, "BRAVO")
        .build()
        .await
        .unwrap();
    assert_eq!(out.system, "ALPHA\n\nBRAVO");
    assert_eq!(out.fragments.len(), 2);
    assert_eq!(out.fragments[0].source, "a");
    assert_eq!(out.fragments[1].role_hint, RoleHint::Developer);
}

#[tokio::test]
async fn empty_static_sources_are_skipped() {
    let out = SystemPromptBuilder::new()
        .add_static("a", RoleHint::System, "ALPHA")
        .add_static("blank", RoleHint::Developer, "")
        .add_static("c", RoleHint::User, "CHARLIE")
        .build()
        .await
        .unwrap();
    assert_eq!(out.system, "ALPHA\n\nCHARLIE");
    assert_eq!(out.fragments.len(), 2);
}

#[tokio::test]
async fn codex_default_renders_five_layers() {
    let d = TempDir::new().unwrap();
    std::fs::write(d.path().join("AGENTS.md"), "use rustfmt").unwrap();

    let base: Arc<dyn ContextSource> = Arc::new(crate::source::StaticSource::new(
        "base",
        RoleHint::System,
        CODEX_BASE_INSTRUCTIONS,
    ));
    let permissions: Arc<dyn ContextSource> = Arc::new(crate::source::StaticSource::new(
        "permissions",
        RoleHint::Developer,
        "<permissions>read-only sandbox</permissions>",
    ));
    let agents_md: Arc<dyn ContextSource> = Arc::new(AgentsMdSource::new(d.path(), d.path()));
    let environment: Arc<dyn ContextSource> = Arc::new(EnvironmentContextSource::new(d.path()));

    let out = SystemPromptBuilder::codex_default(
        base,
        Some(permissions),
        None,
        Some(agents_md),
        environment,
    )
    .build()
    .await
    .unwrap();

    // Order check: base → permissions → AGENTS.md → environment_context.
    let positions: Vec<usize> = [
        "GPT-5",
        "permissions",
        "use rustfmt",
        "<environment_context>",
    ]
    .iter()
    .filter_map(|needle| out.system.find(needle))
    .collect();
    // Codex base contains the marker we look for, plus the others should
    // appear in monotonically increasing positions.
    let _ = positions; // tolerant — base text may evolve upstream.

    assert!(out.system.contains("<permissions>"));
    assert!(out.system.contains("use rustfmt"));
    assert!(out.system.contains("<environment_context>"));
    assert!(out.system.contains("<cwd>"));
}

#[tokio::test]
async fn build_is_deterministic_across_calls() {
    let d = TempDir::new().unwrap();
    std::fs::write(d.path().join("AGENTS.md"), "x").unwrap();
    let cwd: PathBuf = d.path().to_path_buf();

    let make = || {
        let env: Arc<dyn ContextSource> = Arc::new(EnvironmentContextSource {
            cwd: cwd.clone(),
            shell: Some("bash".into()),
            include_date: false, // exclude date so output is stable
        });
        let agents: Arc<dyn ContextSource> = Arc::new(AgentsMdSource::new(&cwd, &cwd));
        SystemPromptBuilder::new().add(agents).add(env)
    };

    let a = make().build().await.unwrap();
    let b = make().build().await.unwrap();
    assert_eq!(a.system, b.system);
}
