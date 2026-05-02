# crates/hivecore-browser-runtime/AGENTS.md

**Layer 3.** Default `BrowserProvider` impl over `chromiumoxide` (CDP).
ADR-028. Patterns adopted from `gsd-build/gsd-browser` (Apache-2.0); no
verbatim source vendoring (per ADR-024 vendored content carries
attribution — runtime crate has none yet).

## What lives here

| Module | Purpose |
|---|---|
| `chrome_provider` | `ChromeProvider` (impls `BrowserProvider`) + `ChromeConfig`. |

## Architecture

- One `Browser` (chromiumoxide) per session. Spawned via `Browser::launch`.
- `tokio::task::JoinHandle` drives the event loop; aborted on session close.
- `Mutex<HashMap<BrowserSessionId, Session>>` owns all live sessions.
- Snapshot version monotonic per session. Increments on every `snapshot()`.
- `verify_tenant` runs before any session-bound op. Cross-tenant access
  returns `BrowserError::CrossTenantDenied` by construction.

## v0.1 implementation status

| Method | Status |
|---|---|
| `ensure_session` / `close_session` / `list_sessions` | working |
| `navigate` | working |
| `snapshot` (CDP `Accessibility.getFullAXTree`) | working — filters to ~16 actionable roles |
| `act` | **stub** — captures intent + re-snapshots; CDP dispatch deferred to v0.2 (chromiumoxide 0.9.1's node-id-to-element-handle bridge is awkward) |
| `wait_for` | partial — `Delay` and `Load`/`DomContentLoaded`; other conditions degrade to delay |
| `assert` | working — re-snapshots, evaluates predicates against the fresh tree |
| `screenshot` | working |
| `back` / `forward` / `reload` | not implemented (returns `CapabilityMissing`) |
| `console_messages` / `network_list` / `eval` / `trace_*` | not implemented |

## Smoke test (live)

```
cargo run --example smoke -p hivecore-browser-harness
```

Drives the full v0.1 surface: open → navigate → snapshot → assert →
screenshot → close. Passes locally on Linux with
`/usr/bin/google-chrome-stable` installed.

## Tests

```
cargo test -p hivecore-browser-runtime          # unit (no live Chrome)
cargo run --example smoke -p hivecore-browser-harness   # live e2e
```

CI fixture story (per ADR-028 §9, browser-harness AGENTS.md): local static
test server, no real-internet calls. Not yet implemented — v0.2.

## v0.2 backlog

- **Real CDP action dispatch in `act`** — bridge `AxNodeId` → CDP backendNodeId
  → DOM.resolveNode → Runtime.callFunctionOn. Replaces the v0.1 stub. Likely
  needs a small `chromiumoxide` PR or an in-tree workaround.
- **Full `WaitCondition` predicates** — selector visibility, network-idle,
  text-visible, URL-regex.
- **`back` / `forward` / `reload`** — `Page::go_back` is straightforward;
  add when a real harness needs them.
- **Per-tenant artifact root enforcement** — currently `ChromeConfig.artifact_root`
  is set but unused (screenshots return bytes only). Wire `screenshot()` to
  write under `<artifact_root>/<tenant_id>/<session_id>/screenshot-<n>.png`.
- **`InjectionScan` capability** — gsd-browser-style scanner over snapshot
  text. Tags `SnapshotElement.injection_flagged = true`.
- **`CredentialVault` capability** — AES-GCM + Argon2 keyed by `tenant_id`.
- **Per-(tenant, session) UNIX socket daemon** — currently sessions are
  in-process. v0.2 splits the `Browser` into a sidecar process so daemon
  lifetime survives harness restart.
- **Stale-ref-after-navigation guard tightening** — `act` checks the version
  but a navigation between snapshot and act could leave the version stale
  in subtle cases.

## Stability

Pre-1.0. `ChromeConfig` may grow fields additively. Public surface is
the `BrowserProvider` impl + `ChromeProvider::new` + `ChromeConfig`.
