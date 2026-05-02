//! End-to-end: load the prebuilt `hello-extension.wasm` Component, list its
//! tools, and invoke each one. The test compiles the example on demand if
//! the artifact is missing so a fresh checkout `cargo test` works.

use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use hivecore_extension_host::{
    host::{HostState, InMemorySettings, SettingsProvider},
    ExtensionLoader, ExtensionTool,
};
use hivecore_runtime_core::{AbortSignal, Tool, ToolCallId, ToolInvocation, UpdateSink};

fn extension_wasm() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path =
        manifest.join("examples/hello-extension/target/wasm32-wasip2/release/hello_extension.wasm");
    if !path.exists() {
        let status = Command::new("cargo")
            .args(["build", "--release", "--target", "wasm32-wasip2"])
            .current_dir(manifest.join("examples/hello-extension"))
            .status()
            .expect("cargo build");
        assert!(status.success(), "extension build failed");
    }
    assert!(path.exists(), "wasm artifact missing: {}", path.display());
    path
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lists_tools_and_invokes_them() {
    let path = extension_wasm();
    let loader = ExtensionLoader::new();
    let settings = InMemorySettings::new();
    settings.set("welcome", "from-host");
    let host = HostState::new(
        "hello-extension",
        Arc::new(settings.clone()) as Arc<dyn SettingsProvider>,
    );

    let extension = loader.load(&path, host).expect("load");
    let specs = extension.tools().to_vec();
    assert_eq!(specs.len(), 2);
    let names: Vec<&str> = specs.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"say_hello"));
    assert!(names.contains(&"lookup"));

    let ext_arc = Arc::new(std::sync::Mutex::new(extension));
    let tools: Vec<ExtensionTool> = ExtensionTool::from_extension(ext_arc);

    let say_hello = tools.iter().find(|t| t.name() == "say_hello").unwrap();
    let lookup = tools.iter().find(|t| t.name() == "lookup").unwrap();

    let (_h, sig) = AbortSignal::new();
    let out = say_hello
        .execute(
            ToolInvocation {
                id: ToolCallId("c1".into()),
                name: "say_hello".into(),
                input: serde_json::json!({"name": "hivecore"}),
            },
            sig.clone(),
            UpdateSink::noop(),
        )
        .await
        .expect("say_hello");
    assert!(!out.is_error);
    let txt = match &out.content[0] {
        hivecore_runtime_core::ContentBlock::Text { text } => text.clone(),
        _ => panic!(),
    };
    assert!(txt.contains("Hello, hivecore"));

    // Lookup hits the host import.
    let out = lookup
        .execute(
            ToolInvocation {
                id: ToolCallId("c2".into()),
                name: "lookup".into(),
                input: serde_json::json!({"key": "welcome"}),
            },
            sig,
            UpdateSink::noop(),
        )
        .await
        .expect("lookup");
    let txt = match &out.content[0] {
        hivecore_runtime_core::ContentBlock::Text { text } => text.clone(),
        _ => panic!(),
    };
    assert_eq!(txt, "from-host");
}
