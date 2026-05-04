use std::sync::Arc;

use hivecore_execution_env::LocalEnv;
use hivecore_runtime_core::{AbortSignal, ExecutionEnv, ToolCallId, ToolInvocation, UpdateSink};
use tempfile::TempDir;

use super::*;

fn setup(content: &str) -> (TempDir, EditTool) {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("f.txt"), content).unwrap();
    let env: Arc<dyn ExecutionEnv> = Arc::new(LocalEnv::new(dir.path().to_path_buf()));
    let tool = EditTool::new(WorkspaceRoot::new(dir.path()).unwrap(), env);
    (dir, tool)
}

fn inv(input: serde_json::Value) -> ToolInvocation {
    ToolInvocation {
        id: ToolCallId("c".into()),
        name: "edit_file".into(),
        input,
    }
}

#[tokio::test]
async fn replaces_unique_occurrence() {
    let (dir, tool) = setup("hello world");
    let (_h, sig) = AbortSignal::new();
    let out = tool
        .execute(
            inv(serde_json::json!({"path":"f.txt","old_string":"world","new_string":"hivecore"})),
            sig,
            UpdateSink::noop(),
        )
        .await
        .unwrap();
    assert!(!out.is_error);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("f.txt")).unwrap(),
        "hello hivecore"
    );
}

#[tokio::test]
async fn rejects_ambiguous_edit() {
    let (_d, tool) = setup("aa aa aa");
    let (_h, sig) = AbortSignal::new();
    let err = tool
        .execute(
            inv(serde_json::json!({"path":"f.txt","old_string":"aa","new_string":"bb"})),
            sig,
            UpdateSink::noop(),
        )
        .await
        .unwrap_err();
    assert!(format!("{err}").contains("multiple times"));
}

#[tokio::test]
async fn replace_all_works() {
    let (dir, tool) = setup("aa aa aa");
    let (_h, sig) = AbortSignal::new();
    tool.execute(
        inv(serde_json::json!({"path":"f.txt","old_string":"aa","new_string":"bb","replace_all":true})),
        sig,
        UpdateSink::noop(),
    )
    .await
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.path().join("f.txt")).unwrap(),
        "bb bb bb"
    );
}

#[tokio::test]
async fn rejects_missing_string() {
    let (_d, tool) = setup("hello");
    let (_h, sig) = AbortSignal::new();
    let err = tool
        .execute(
            inv(serde_json::json!({"path":"f.txt","old_string":"xyz","new_string":"q"})),
            sig,
            UpdateSink::noop(),
        )
        .await
        .unwrap_err();
    assert!(format!("{err}").contains("not found"));
}
