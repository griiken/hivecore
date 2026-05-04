use std::fs;
use std::sync::Arc;

use hivecore_execution_env::LocalEnv;
use hivecore_runtime_core::{
    AbortSignal, ExecutionEnv, Tool, ToolCallId, ToolInvocation, UpdateSink,
};
use tempfile::TempDir;

use super::*;

fn invoke(name: &str, input: serde_json::Value) -> ToolInvocation {
    ToolInvocation {
        id: ToolCallId("c".into()),
        name: name.into(),
        input,
    }
}

fn env_for(dir: &TempDir) -> Arc<dyn ExecutionEnv> {
    Arc::new(LocalEnv::new(dir.path().to_path_buf()))
}

#[tokio::test]
async fn reads_full_file() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("hi.txt"), "alpha\nbeta\ngamma\n").unwrap();
    let tool = ReadTool::new(WorkspaceRoot::new(dir.path()).unwrap(), env_for(&dir));
    let (_h, sig) = AbortSignal::new();
    let out = tool
        .execute(
            invoke("read_file", serde_json::json!({"path": "hi.txt"})),
            sig,
            UpdateSink::noop(),
        )
        .await
        .unwrap();
    assert!(!out.is_error);
    let text = match &out.content[0] {
        hivecore_runtime_core::ContentBlock::Text { text } => text.clone(),
        _ => panic!(),
    };
    assert!(text.contains("alpha"));
    assert!(text.contains("3\tgamma"));
}

#[tokio::test]
async fn rejects_path_escape() {
    let dir = TempDir::new().unwrap();
    let tool = ReadTool::new(WorkspaceRoot::new(dir.path()).unwrap(), env_for(&dir));
    let (_h, sig) = AbortSignal::new();
    let err = tool
        .execute(
            invoke("read_file", serde_json::json!({"path": "../etc/passwd"})),
            sig,
            UpdateSink::noop(),
        )
        .await
        .unwrap_err();
    assert!(format!("{err}").contains("escapes workspace"));
}

#[tokio::test]
async fn slice_with_offset_limit() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("h.txt"), "1\n2\n3\n4\n5\n").unwrap();
    let tool: Arc<dyn Tool> = Arc::new(ReadTool::new(
        WorkspaceRoot::new(dir.path()).unwrap(),
        env_for(&dir),
    ));
    let (_h, sig) = AbortSignal::new();
    let out = tool
        .execute(
            invoke(
                "read_file",
                serde_json::json!({"path":"h.txt","offset":2,"limit":2}),
            ),
            sig,
            UpdateSink::noop(),
        )
        .await
        .unwrap();
    let text = match &out.content[0] {
        hivecore_runtime_core::ContentBlock::Text { text } => text.clone(),
        _ => panic!(),
    };
    assert!(text.contains("     2\t2"));
    assert!(text.contains("     3\t3"));
    assert!(!text.contains("\t4"));
}
