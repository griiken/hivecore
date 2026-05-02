<div align="center">

# Hivecore

**OSS substrate for org-customizable AI SDLC harnesses.**

[Docs](./docs/introduction.md) · [Architecture](./docs/architecture.md) · [Concepts](./docs/concepts.md) · [Decisions](./docs/DECISIONS.md) · [Session warmup](./docs/SESSION-START.md) · [Roadmap](./docs/roadmap.md)

</div>

---

## What this is

Hivecore is the substrate. Companies clone it and define **their own agents** (roles — beyond just PM/Dev/QA: compliance officer, firmware reviewer, license auditor, anything), **their own workflow** (DAG as config, not code), and **their own control plane** (policies, budgets, audit, kill switches).

Built around an I/O-free trait core, a pi-shape nested agent loop, an Agent Client Protocol (ACP) server that mounts under Zed / VS Code / Cursor, opt-in MCP integration, a real CDP-backed browser harness, summarising context compaction, and HITL approval primitives — all dual-licensed `MIT OR Apache-2.0`.

## Status — v0.1 substrate

Sixteen crates landed under `crates/hivecore-*/`. **165 unit tests + multiple live e2e flows**. See [`AGENTS.md`](./AGENTS.md) for the substrate map.

| Crate | Layer | What |
|---|---|---|
| `hivecore-runtime-core` | 1 | I/O-free traits: `Tool`, `ModelAdapter`, `EventSink`, `LifecycleHook`, `ContextTransform`, `AbortSignal`, `Approval` + `RiskAugmenter`. |
| `hivecore-openai-adapter` | 2 | Chat Completions w/ streaming + reasoning. |
| `hivecore-agent-loop` | 2 | Driver — pi-shape nested loop, sub-agent spawn, lifecycle wiring. |
| `hivecore-builtin-tools` | 3 | `read_file` / `write_file` / `edit_file` / `bash` / `grep`. |
| `hivecore-persistence` | 3 | Session JSONL + ADR-019 audit JSONL. |
| `hivecore-acp-server` | 3 | Agent Client Protocol stdio server (binary `hivecore-acp`) — mounts under any ACP client. HITL via `session/request_permission`. |
| `hivecore-extension-host` | 3 | WASM Component Model loader + WIT v0.1.0. |
| `hivecore-config` | 3 | TOML `Agent` registry. |
| `hivecore-skills` | 3 | md+frontmatter skills + `agent:` routing. |
| `hivecore-system-prompt` | 3 | Layered prompt builder; vendors Codex `default.md` + Zed `system_prompt.hbs`. |
| `hivecore-coder` | 3 | Sample CLI harness (binary `hivecore-coder`) — proves substrate composability. |
| `hivecore-compaction` | 3 | `SummarizingTransform` (ADR-026) — fires at 80% / 95%, preserves real user messages. |
| `hivecore-browser-{core,runtime,harness}` | 2/3 | `BrowserProvider` trait + `chromiumoxide` daemon + 7 typed `Tool` impls (`browser_session` / `navigate` / `snapshot` / `act` / `wait` / `assert` / `screenshot`). |
| `hivecore-mcp-client` | 3 | Opt-in MCP integration (`rmcp 0.8`, stdio) — lazy 3-meta-tool default (`mcp_servers` / `mcp_discover` / `mcp_call`), `read_only_hint` consumption. |
| `hivecore-tool-policy` | 3 | HITL approval (ADR-029) — `ApprovalHook: impl ToolHook`, `ApprovalPolicy { OnRequest, UnlessTrusted, Never, FailOnAsk }`, 3-state matcher, session cache, `apply_exclude` filter. |

29 ADRs under [`docs/adr/`](./docs/adr/). Newest: ADR-029 (HITL approval primitive) shipped through six revisions tracking pi / Codex / ACP / Warp / Aider / Cline / Continue.dev / Goose / OpenHands SDK / Claude Code prior art.

## Build

```bash
cargo check --workspace
cargo test --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

All four must pass before merge. Rust 1.85+.

## Use

### CLI agent — `hivecore-coder`

```bash
# one-shot prompt
cargo run -p hivecore-coder -- "fix the failing test in crates/hivecore-runtime-core"

# resume the latest session in this cwd
cargo run -p hivecore-coder -- --continue "follow up"

# resume a specific session
cargo run -p hivecore-coder -- --session <id|name> "..."

# autonomous (no HITL prompts)
cargo run -p hivecore-coder -- --no-approval "..."

# read-only investigation mode
cargo run -p hivecore-coder -- --exclude bash,write_file,edit_file "investigate"
```

Sessions persist as JSONL under `~/.hivecore/sessions/default/<session_id>.jsonl` (ADR-026). Compaction fires automatically at 80% / 95% of the model context window.

Environment:
- `OPENAI_API_KEY` — required
- `HIVECORE_OPENAI_BASE_URL` — optional (default OpenAI; set to e.g. z.ai endpoint for GLM)
- `HIVECORE_MODEL` — model id (default `gpt-4.1`)
- `HIVECORE_WORKSPACE` — workspace root (default cwd)

### ACP server — `hivecore-acp`

Mount under Zed / Cursor / VS Code (or any ACP-aware client) over stdio.

```bash
cargo build -p hivecore-acp-server --release
# point your editor at: target/release/hivecore-acp
```

HITL approval surfaces as native `session/request_permission` JSON-RPC. Set `HIVECORE_NO_APPROVAL=1` to opt out.

### Optional MCP servers

Drop a TOML file at `~/.hivecore/mcp.toml` or `<workspace>/.hivecore/mcp.toml`:

```toml
[mcp_servers.git]
command = "uvx"
args = ["mcp-server-git"]
```

Three lazy meta-tools (`mcp_servers` / `mcp_discover` / `mcp_call`) appear in the agent's tool list when any server is configured. Per-tool `read_only_hint` from MCP `ToolAnnotations` feeds the HITL `RiskAugmenter`.

## Stack

- **Rust 1.85+** — single Cargo workspace, sixteen crates
- **rmcp 0.8** — official MCP SDK (stdio transport in v0.2)
- **agent-client-protocol 0.11** — Zed's ACP SDK
- **chromiumoxide 0.9.1** — pinned (workarounds for upstream issues #292, #320 documented)
- **OpenAI Chat Completions** — primary model adapter; `HIVECORE_OPENAI_BASE_URL` enables any compatible endpoint
- **Postgres 17 + pgvector** — *planned* for KG storage (Phase 1, not built)
- **Next.js 16** — *planned* frontend under `apps/web/` (not built)
- **Firecracker** — *planned* sandbox runner (not built)

## Layout

```
crates/                 Rust workspace (sixteen crates)
docs/                   Architecture, concepts, philosophy, comparison, faq, terms
docs/adr/               29 architectural decision records
.research/              Source bookmarks (per ADR-010 — pointers, not content dumps)
apps/web/               (planned) Next.js frontend
cli/                    (placeholder) thin top-level CLI; real binaries are crate-local
scenarios/              (planned) executable validation cases
spikes/                 (gone — moved to `crates/hivecore-*/`, ADR-025)
```

## Documentation

Start with [`docs/SESSION-START.md`](./docs/SESSION-START.md) for a ten-minute warmup, then [`docs/architecture.md`](./docs/architecture.md) and [`docs/concepts.md`](./docs/concepts.md). [`docs/DECISIONS.md`](./docs/DECISIONS.md) indexes the 29 ADRs.

For coding agents working in this tree: read [`AGENTS.md`](./AGENTS.md) — single source of truth, picked up automatically by Codex / Cursor / Continue / Cline / Windsurf / Aider / OpenCode / Claude Code.

## License

Dual-licensed under **MIT OR Apache-2.0** at your option. See [LICENSE-MIT](./LICENSE-MIT), [LICENSE-APACHE](./LICENSE-APACHE), and [docs/terms.md](./docs/terms.md).

## Contributing

See [`docs/contributing.md`](./docs/contributing.md). Hivecore's [Code of Conduct](./CODE_OF_CONDUCT.md) applies. Non-trivial proposals go through an `rfc:`-labelled issue with a one-week comment period before implementation.
