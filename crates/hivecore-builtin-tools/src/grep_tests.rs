use std::sync::Arc;

use hivecore_execution_env::LocalEnv;
use hivecore_runtime_core::{AbortSignal, ExecutionEnv, ToolCallId, ToolInvocation, UpdateSink};
use tempfile::TempDir;

use super::*;

fn env_for(dir: &TempDir) -> Arc<dyn ExecutionEnv> {
    Arc::new(LocalEnv::new(dir.path().to_path_buf()))
}

#[tokio::test]
async fn finds_pattern_across_files() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("a.txt"), "alpha\nfoo here\n").unwrap();
    std::fs::write(dir.path().join("b.txt"), "no match\n").unwrap();
    std::fs::create_dir(dir.path().join("sub")).unwrap();
    std::fs::write(dir.path().join("sub/c.txt"), "another foo\n").unwrap();

    let tool = GrepTool::new(WorkspaceRoot::new(dir.path()).unwrap(), env_for(&dir));
    let (_h, sig) = AbortSignal::new();
    let out = tool
        .execute(
            ToolInvocation {
                id: ToolCallId("c".into()),
                name: "grep".into(),
                input: serde_json::json!({"pattern":"foo"}),
            },
            sig,
            UpdateSink::noop(),
        )
        .await
        .unwrap();
    assert!(!out.is_error);
    let matches = out.details.unwrap()["matches"].as_u64().unwrap();
    assert_eq!(matches, 2);
}

#[tokio::test]
async fn case_insensitive_flag_works() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("a.txt"), "FOO\nfoo\n").unwrap();
    let tool = GrepTool::new(WorkspaceRoot::new(dir.path()).unwrap(), env_for(&dir));
    let (_h, sig) = AbortSignal::new();
    let out = tool
        .execute(
            ToolInvocation {
                id: ToolCallId("c".into()),
                name: "grep".into(),
                input: serde_json::json!({"pattern":"foo","case_insensitive":true}),
            },
            sig,
            UpdateSink::noop(),
        )
        .await
        .unwrap();
    let matches = out.details.unwrap()["matches"].as_u64().unwrap();
    assert_eq!(matches, 2);
}

#[tokio::test]
async fn skips_hidden_dirs() {
    let dir = TempDir::new().unwrap();
    std::fs::create_dir(dir.path().join(".git")).unwrap();
    std::fs::write(dir.path().join(".git/HEAD"), "secret\n").unwrap();
    std::fs::write(dir.path().join("visible.txt"), "secret\n").unwrap();
    let tool = GrepTool::new(WorkspaceRoot::new(dir.path()).unwrap(), env_for(&dir));
    let (_h, sig) = AbortSignal::new();
    let out = tool
        .execute(
            ToolInvocation {
                id: ToolCallId("c".into()),
                name: "grep".into(),
                input: serde_json::json!({"pattern":"secret"}),
            },
            sig,
            UpdateSink::noop(),
        )
        .await
        .unwrap();
    let matches = out.details.unwrap()["matches"].as_u64().unwrap();
    assert_eq!(matches, 1);
}
