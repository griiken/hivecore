# crates/hivecore-builtin-tools/AGENTS.md

**Layer 3 — five builtin tools.** Each is a `hivecore_runtime_core::Tool` impl.

## Tools

| Name | Purpose |
|---|---|
| `read_file` | Read a workspace file with optional `offset`/`limit` (Claude-Code-style line numbers). |
| `write_file` | Overwrite or create a file; auto-creates parent dirs. |
| `edit_file` | Exact-string replace; rejects ambiguous edits unless `replace_all = true`. |
| `bash` | Shell command with timeout + kill-on-drop + 32 KiB output cap. |
| `grep` | Regex search across the workspace; hidden-dir skip; offloaded to a blocking task. |

## Path safety

Every tool resolves user-supplied paths through `WorkspaceRoot::resolve`:
- `..` rejected pre-canonicalisation.
- Absolute paths must already live under the canonicalised root.
- Symlink escapes detected via canonicalised containment check.

`WorkspaceRoot::new(path)` canonicalises at construction. The longest-existing-prefix dance lets `write_file` create new files under the root.

## Test commands

```
cargo test -p hivecore-builtin-tools
OPENAI_API_KEY=... cargo run --example swe_agent
```

## v0.2 backlog

- `glob` tool for batch file selection.
- Optional sandboxed `bash` execution (Codex-style: bwrap / landlock / seatbelt) — current impl runs unsandboxed.
- A `WriteHookGate` `ToolHook` example showing how a harness can require approval before destructive edits.
