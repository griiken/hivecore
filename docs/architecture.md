# Architecture

Hivecore is organized in layers. Each layer has a single responsibility and a stable contract with the layers above and below.

## Layer diagram

```
┌─────────────────────────────────────────────────────────────┐
│ Frontend (apps/web)              CLI (cli/)                 │
│ Next.js 16 + TS                  Rust single binary         │
│ Board, run viewer, KG visualizer Local + remote daemon ops  │
└──────────────────┬──────────────────────────────────────────┘
                   │ gRPC + WebSocket
┌──────────────────▼──────────────────────────────────────────┐
│ Control plane API (crates/hivecore-api)                     │
│ axum + tower; auth, RBAC, policy enforcement                │
└──────────────────┬──────────────────────────────────────────┘
                   │
┌──────────────────▼──────────────────────────────────────────┐
│ Orchestrator (crates/hivecore-orchestrator)                 │
│ Durable state machine, journal-replay, saga compensation    │
└────┬─────────────────┬──────────────────┬───────────────────┘
     │                 │                  │
┌────▼────┐      ┌─────▼──────┐    ┌──────▼─────────────┐
│ KG      │      │ Coordination│    │ Sandbox runner    │
│ engine  │      │ board       │    │ (Firecracker)     │
└────┬────┘      └─────┬──────┘    └──────┬─────────────┘
     │                 │                  │
┌────▼─────────────────▼──────────────────▼─────────────────┐
│ Persona harness (crates/hivecore-persona)                  │
│ 5 layers per persona run: orchestration, context, tools,   │
│ verification, operations                                    │
└──────────────────┬──────────────────────────────────────────┘
                   │ adapter trait
┌──────────────────▼──────────────────────────────────────────┐
│ Runtime adapters (crates/adapter-*)                         │
│ Claude Code | Codex | OpenCode | Pi | custom                │
└─────────────────────────────────────────────────────────────┘
```

## Crate inventory (Rust workspace)

```
crates/
├── hivecore-core/           I/O-free domain types (Persona, Workflow, Verdict, Run, Skill, Artifact, Tenant, Episode, Fact)
├── hivecore-config/         Persona/workflow/policy/ontology TOML schemas + parser
├── hivecore-kg/             Knowledge Core engine (multi-tenant, bi-temporal, hybrid search)
├── hivecore-orchestrator/   Durable execution wrapping microsoft/duroxide; saga; workflow DAG executor
├── hivecore-coordination/   File ownership + message bus
├── hivecore-sandbox/        Firecracker / Docker runner abstraction; secret broker
├── hivecore-persona/        Persona harness execution (5-layer)
├── hivecore-gates/          Gate engine (computational + inferential)
├── hivecore-skills/         Skill registry, versioning, provenance
├── hivecore-validation/     Scenario engine + production replay + digital-twin mocks
├── hivecore-budget/         Token/time/dollar caps; loop detection
├── hivecore-board/          Native kanban board state + task queue + lifecycle
├── hivecore-daemon/         Native daemon runner (binary) — claims tasks, spawns sandbox, streams events
├── hivecore-api/            HTTP + gRPC + WebSocket control plane API
├── hivecore-telemetry/      OpenTelemetry tracing, metrics
├── hivecore-audit/          Append-only hash-chained audit log
├── hivecore-sim/            Simulation engine (P8 aspirational)
├── adapter-claude-code/     Claude Code runtime adapter
├── adapter-codex/           Codex CLI runtime adapter
├── adapter-opencode/        OpenCode runtime adapter
├── adapter-pi/              Pi SDK runtime adapter
└── hivecore-cli/            CLI binary
```

## Frontend (`apps/web/`) — UI-first primary surface

Next.js 16 (App Router), TypeScript strict, React Query (server state), Zustand (client state), shadcn/ui, typed client to Rust backend (tRPC bridge or OpenAPI codegen), WebSocket subscription for live run streaming.

Every backend capability has a corresponding UI surface (UI-first principle, ADR-014). UI generates the underlying TOML/YAML config so git-versioning + AI-readability are preserved.

| Surface | Phase | Backing |
|---|---|---|
| Auth + Hello page | P1 | API health endpoint |
| KG visualizer (Cytoscape/React Flow, bi-temporal `as_of` slider, blast-radius highlight) | P2 | KG MCP API |
| Tenant + Knowledge Core admin | P2 | Tenant API |
| Ontology editor (custom node/edge types via forms → TOML) | P2 | Config API |
| Workspace board (kanban, task creation form, persona/workflow selector) | P4 | Board + API |
| Live run viewer (WebSocket events, replayable timeline) | P4 | Run + WS API |
| Daemon admin (register/configure/monitor daemons) | P4 | Daemon API |
| Visual persona editor (forms → TOML) | P5 | Config API |
| Visual workflow DAG editor (n8n-style canvas → DAG TOML) | P5 | Config API |
| Skill registry UI (browse/version/sign/audit) | P5 | Skills API |
| Scenario authoring UI (visual builder → YAML) | P5 | Validation API |
| Policy + RBAC management UI | P6 | Policy API |
| Audit + telemetry dashboards (cost, gate failures, TBD) | P6 | Audit + Metrics |

## Storage

- **Postgres 17 + pgvector** — primary store (KG nodes/edges/metadata, runs, skills, audit log)
- **S3 / object store** — large artifacts (run logs, snapshots, scenario captures)
- **Local filesystem** — sandbox per-run scratch (ephemeral)

## Wire formats

- gRPC for control plane RPC
- WebSocket for live run event streaming (frontend ↔ orchestrator)
- HTTP/JSON for webhooks (GitHub, GitLab, Slack, etc.)
- MCP (Model Context Protocol) for KG-as-tool exposure to runtime adapters

## Concurrency model

- Per task: roles sequential along workflow DAG.
- Across tasks: parallel via priority lanes (queue).
- Within a persona: parallel sub-agents allowed (e.g., Dev persona spawns parallel test-writer + doc-writer).
- Worktree isolation per run (one task = one git worktree, never two on same branch).
- File ownership board prevents cross-run filesystem conflicts.

## Durability

- Every persona run = append-only event log (prompts, tool calls, file edits, gate results).
- Orchestrator state journaled per transition.
- On crash: read last good journal entry, reconstruct context, resume from checkpoint.
- Saga pattern for multi-persona workflows: compensating actions on partial failure.

## Multi-tenancy

- Each tenant = one Knowledge Core (isolated graph instance).
- RBAC enforced at retrieval (permission-aware retrieval pattern).
- Cross-tenant queries forbidden at API level.
- Audit log per tenant, hash-chained.

## Extension points

- **Personas** — declare in `.hivecore/personas/*.toml`.
- **Workflows** — declare in `.hivecore/workflows/*.dag.toml`.
- **Policies** — declare in `.hivecore/policies/*.toml`.
- **KG ontology** — extend node/edge types in `.hivecore/ontology/*.toml`.
- **Runtime adapters** — implement `RuntimeAdapter` trait.
- **Gates** — implement `Gate` trait, register in policy.
- **Skills** — markdown + optional scripts in `.hivecore/skills/`.

## Dependencies (system-level)

- Postgres 17 + pgvector extension
- Firecracker (or Docker for dev)
- Rust 1.78+
- Node.js 20+ + pnpm 9+

## Empty dependency graph

(Phase 0 will populate. Each crate added produces a node + ADR documenting its boundary contract.)

```
[ to be filled as crates land ]
```
