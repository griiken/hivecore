# crates/hivecore-extension-host/AGENTS.md

**Layer 3 — WASM extension host.** Loads `.wasm` Component Model files implementing `hivecore:extension/extension@0.1.0` (see `wit/since_v0.1.0/`).

## What lives here

| Module | Purpose |
|---|---|
| `bindings` | `wasmtime::component::bindgen!` output for the WIT world. |
| `host` | `HostState` (per-extension wasmtime store) + `SettingsProvider` trait. |
| `loader` | `ExtensionLoader` + `Extension` — instantiation + lifecycle (`init` / `list-tools` / `run-tool`). |
| `tool_adapter` | `ExtensionTool` — adapts a WASM tool into `hivecore_runtime_core::Tool`. |
| `error` | `ExtensionError`. |

## Why we don't use Zed's host

- Zed's `extension_host` is **GPL-3.0** + drags in 30+ Zed-internal crates (`gpui`, `language`, `fs`, `dap`, `project`, …).
- Our boundary is multi-tenant + scoped to tools/audit/KG, not LSP/DAP.

We borrow Apache-2.0 portions only: the `since_vX.Y.Z/` versioned WIT directory layout (ADR-017), and the "extension declares its tools, host adapts to native trait" architecture.

## Capability surface (v0.1)

The world `extension` exports:
- `init: func()` — called once at load.
- `list-tools: func() -> list<tool-spec>` — catalog.
- `run-tool: func(name, args-json) -> result<tool-result, string>` — invoke.

And imports (the *only* things an extension can call back into):
- `host.log(level, message)`
- `host.get-setting(key) -> option<string>`

Everything else (FS, network, syscalls) is OFF. Adding a capability is an explicit, ADR-tracked decision.

## Building an extension

The example under `examples/hello-extension/` is its own workspace (excluded from the parent — different target).

```
cd examples/hello-extension
cargo build --release --target wasm32-wasip2
# → target/wasm32-wasip2/release/hello_extension.wasm  (~99K)
```

## Test commands

```
cargo test -p hivecore-extension-host   # builds the example on demand
```

## v0.2 backlog

- `register-skill`, `register-command`, `subscribe-hook` exports (WIT v0.2).
- Tenant-scoped settings: `host.get-setting` already plumbs through `SettingsProvider`; the multi-tenant adapter is the missing piece.
- Hot-reload / file watcher integration (mirror `skills::SkillWatcher`).
