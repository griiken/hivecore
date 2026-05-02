# Decisions

Append-only architectural decision record (ADR) log. One file per ADR under [`adr/`](./adr/). Newest at the bottom. Never delete; supersede with a new ADR that links back.

## Format per ADR

```
## ADR-NNN · YYYY-MM-DD · <short title>

**Decision.** <one sentence>

**Context.** <why we needed to decide>

**Alternatives considered.** <bullets>

**Consequences.** <what this enables and what it costs>

**Revisit when.** <conditions that would invalidate this>
```

New ADR: copy template above into `docs/adr/<NNN>.md`, then add row below.

## Index

| ADR | Date | Title |
|---|---|---|
| [001](./adr/001.md) | 2026-04-29 | Project bootstrapped via project-ignition |
| [002](./adr/002.md) | 2026-04-29 | Dual MIT OR Apache-2.0 licensing |
| [003](./adr/003.md) | 2026-04-29 | Target agents: Claude Code + Codex (cross-agent compatible) |
| [004](./adr/004.md) | 2026-04-29 | Backend language: Rust |
| [005](./adr/005.md) | 2026-04-29 | Frontend stack: TypeScript + Next.js 16 (App Router) |
| [006](./adr/006.md) | 2026-04-29 | KG-at-core (not bolted on) |
| [007](./adr/007.md) | 2026-04-29 | Personas, workflows, and policies are CONFIG, not code |
| [008](./adr/008.md) | 2026-04-29 | TDD spike-then-attach development workflow |
| [009](./adr/009.md) | 2026-04-29 | GSD harness as project-management tool |
| [010](./adr/010.md) | 2026-04-29 | `.research/` is reference pointers, not content dumps |
| [011](./adr/011.md) | 2026-04-29 | Postgres 17 + pgvector for KG storage; multi-tenant via RLS |
| [012](./adr/012.md) | 2026-04-29 | Use `microsoft/duroxide` for durable orchestrator |
| [013](./adr/013.md) | 2026-04-29 | `shantanugoel/oxydra` as reference implementation (not dependency) |
| [014](./adr/014.md) | 2026-04-29 | UI-first principle + native board+daemon |
| [015](./adr/015.md) | 2026-04-29 | Phase 7 first vertical = PMG Workbench (motadata-itsm) |
| [016](./adr/016.md) | 2026-04-30 | Two-layer runtime split (Layer 1 / Layer 2 / Layer 3) |
| [017](./adr/017.md) | 2026-04-30 | Extension boundary = WASM Component Model + WIT |
| [018](./adr/018.md) | 2026-04-30 | Agent Client Protocol (ACP) alignment + typed action enum |
| [019](./adr/019.md) | 2026-04-30 | Seven-plane orchestration kernel (UOK + Tenancy plane) |
| [020](./adr/020.md) | 2026-04-30 | v0.1 MVP scope: smallest-believable harness |
| [021](./adr/021.md) | 2026-04-30 | Persona → Agent rename (industry alignment) |
| [022](./adr/022.md) | 2026-04-30 | Lifecycle hooks vs tool hooks — separate trait |
| [023](./adr/023.md) | 2026-04-30 | Skills are tool-shaped, not prompt-injected |
| [024](./adr/024.md) | 2026-04-30 | Vendor upstream system prompts verbatim (Apache-2.0) |
| [025](./adr/025.md) | 2026-04-30 | `spikes/` → `crates/hivecore-*`: full move + rename |
| [026](./adr/026.md) | 2026-05-01 | Session log + resume + compaction architecture |
| [027](./adr/027.md) | 2026-05-01 | MCP integration architecture (deferred — design-only) |
| [028](./adr/028.md) | 2026-05-01 | Browser harness — hybrid a11y-snapshot + delegated vision |
| [029](./adr/029.md) | 2026-05-03 | Human-in-the-loop approval primitive (Layer-1 trait + ACP wire shape) |
