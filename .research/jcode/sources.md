# jcode source pointers

10-line max per source. Bookmarks not content dumps.

## Repo

- **URL**: https://github.com/1jehuang/jcode
- **License**: MIT (single)
- **Lang**: Rust + Swift (iOS) + JS (figma/mockups)
- **Maturity**: v0.11.6, 1.8k stars, 56 releases, 2,897 commits.
- **Author**: solo (1jehuang).
- **Checked**: 2026-05-01.

## Architecture docs worth reading

- `docs/SWARM_ARCHITECTURE.md` — coordinator + worktree-mgr + agents, lifecycle (spawned/ready/running/blocked/completed/failed/stopped/crashed), completion-report policy. Status: **Proposed** (not implemented).
- `docs/MEMORY_ARCHITECTURE.md` — async cross-session memory; petgraph DiGraph; Memory/Tag/Cluster nodes; HasTag/InCluster/RelatesTo/Supersedes/Contradicts edges; embedder = `all-MiniLM-L6-v2`; sidecar = GPT-5.3 Codex Spark; cascade BFS retrieval. Status: **Implemented (core)**.
- `docs/AMBIENT_MODE.md` — proactive always-on mode: garden + scout + work in single pass; subscription-first OAuth; user takes priority; agent self-schedules wake. Status: **Design**.
- `docs/SAFETY_SYSTEM.md` — two-tier (auto-allowed / requires-permission). Multi-channel notify (email/SMS/desktop/webhook/TUI). Review queue persistent. Status: **Design**, ambient-mode is only consumer.
- `docs/MODULAR_ARCHITECTURE_RFC.md` — admits "modular monolith" today; target = foundation/domain/interface/composition layers; "high-churn must depend on stable lower layers, never the reverse".
- `docs/CRATE_OWNERSHIP_BOUNDARIES.md` — `*-types` crates own DTOs (no I/O); domain modules own runtime behavior; `jcode-core` for shared primitives only; compile-speed decision rule.

## Crate inventory (35 crates)

`jcode-{agent-runtime,ambient-types,auth-types,azure-auth,background-types,batch-types,config-types,core,desktop,embedding,gateway-types,memory-types,message-types,mobile-{core,sim},notify-email,pdf,plan,protocol,provider-{core,gemini,metadata,openrouter},selfdev-types,session-types,side-panel-types,task-types,terminal-launch,tui-{core,markdown,mermaid,render,workspace},usage-types}`.

Dominant pattern: `*-types` crates everywhere. Heavy `*-tui-*`. No extension crate, no WASM.

## Key shape findings vs hivecore

- jcode admits monolith → migrating to layered. Hivecore enforces layers from day 1.
- jcode = single-server multi-client. Hivecore = ACP-mounted into any client.
- jcode "self-dev" = agent edits jcode source directly (no sandbox). Hivecore extensions are WASM.
- jcode memory architecture is *implemented* and worth porting patterns from for v0.3 KG work.
- jcode swarm architecture is *proposed* — paper exercise, no shipping code yet.
