---
url: https://github.com/supermemoryai/smfs
status: active-reference
checked: 2026-04-29
tags: [harness, repo-as-harness, agent-instruction-injection, rust, oss]
---

**Why it matters**: Mine 2 patterns for hivecore. NOT a substrate (hosted-API-as-source-of-truth coupling rejected per Out of Scope; smfs requires supermemory.ai backend).

**Anchor**: Thin Rust client mounting Supermemory's hosted RAG product as POSIX filesystem. FUSE on Linux, NFSv3 on macOS. ~73★, v0.0.1, MIT, ~2 weeks old at check time.

**What we adopt**:
1. **`agent_hint.rs` block-injection pattern** — delimited `<!-- >>> tag:begin >>> -->` blocks injected into `~/.claude/CLAUDE.md` / `~/.codex/AGENTS.md` / `~/.gemini/GEMINI.md`, scoped to cwd, orphan-sweep on next mount if daemon crashed. Hivecore's `.hivecore/` persona harness needs almost exactly this for surfacing per-workflow persona prompts. Use the algorithm, not the code.
2. **Push coalescing rule** (`sync/mod.rs`) — "at most 2 server requests per filepath: one inflight + one pending." Plus four-loop split (delta pull / deletion scan / push / inflight poller) as separate JoinSet tasks with `watch::Sender<bool>` shutdown. Reuse the shape for hivecore KG sync.

**What to avoid**: Hosted-API-as-source-of-truth coupling. Modifying user-global agent files by default (smfs does this; hivecore should scope to repo `.hivecore/` only). Don't take a git dependency on a 2-week-old 0.0.1 crate.
