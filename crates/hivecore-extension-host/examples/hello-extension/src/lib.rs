//! Sample hivecore extension. Registers two tools that prove the contract
//! works:
//!
//!   - `say_hello` — pure compute (no host calls).
//!   - `lookup` — exercises the `host::get-setting` import path.

wit_bindgen::generate!({
    world: "extension",
    path: "wit/since_v0.1.0",
});

use hivecore::extension::host;
use serde::Deserialize;

struct HelloExt;

export!(HelloExt);

impl Guest for HelloExt {
    fn list_tools() -> Vec<ToolSpec> {
        vec![
            ToolSpec {
                name: "say_hello".into(),
                description: "Greets the supplied name with a friendly message.".into(),
                parameters_json_schema: r#"{"type":"object","properties":{"name":{"type":"string"}},"required":["name"]}"#.into(),
            },
            ToolSpec {
                name: "lookup".into(),
                description: "Looks up a tenant-scoped setting by key. Returns 'unset' if missing.".into(),
                parameters_json_schema: r#"{"type":"object","properties":{"key":{"type":"string"}},"required":["key"]}"#.into(),
            },
        ]
    }

    fn run_tool(name: String, args_json: String) -> Result<ToolResult, String> {
        host::log(host::LogLevel::Info, &format!("running {name}"));
        match name.as_str() {
            "say_hello" => {
                #[derive(Deserialize)]
                struct A {
                    name: String,
                }
                let args: A =
                    serde_json::from_str(&args_json).map_err(|e| format!("bad args: {e}"))?;
                Ok(ToolResult {
                    content: format!("Hello, {}! — from hello-extension v0.1", args.name),
                    is_error: false,
                })
            }
            "lookup" => {
                #[derive(Deserialize)]
                struct A {
                    key: String,
                }
                let args: A =
                    serde_json::from_str(&args_json).map_err(|e| format!("bad args: {e}"))?;
                let value = host::get_setting(&args.key);
                Ok(ToolResult {
                    content: value.unwrap_or_else(|| "unset".into()),
                    is_error: false,
                })
            }
            other => Err(format!("unknown tool: {other}")),
        }
    }

    fn init() {
        host::log(host::LogLevel::Info, "hello-extension initialised");
    }
}
