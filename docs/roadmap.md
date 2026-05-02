# Roadmap

Hivecore is built phase-by-phase, gated by dependency, not date. A phase is "done" when its scenario passes, tests are written, docs updated, ADRs (if any) recorded, and the PR is merged to main.

## Phase dependency graph

```
P0  Init (repo, docs scaffold, ADR-0001 stack pick, CI)
        │
        ├─ P1   KG engine v0 (typed nodes/edges, Postgres+pgvector)
        │        └─ P2  Bi-temporal layer (validity windows, transaction time)
        │                └─ P3  KG ingestion (code, ADRs, tickets, slack)
        │                        ├─ P4  KG MCP server (query API)
        │                        └─ P5  Knowledge Core isolation (multi-tenant)
        ├─ P6   Durable orchestrator (journal, replay, saga)
        │        └─ P7  Coordination board (file ownership, msg bus)
        ├─ P8   Sandbox runner (Firecracker, network ACL, secret broker)
        ├─ P9   Persona/Workflow/Policy config schema + parser
        └─ P10  First persona: Dev harness  (depends: P3, P6, P7, P8, P9)
                 └─ P11  Scenario validation engine
                          └─ P12  Gate engine (computational + inferential)
                                   └─ P13  Skill registry + provenance writeback
                                            ├─ P14  PM persona
                                            ├─ P15  QA persona (replay-based)
                                            ├─ P16  Review persona (KG-constraint check)
                                            └─ P17  Deploy persona
                                                     └─ P18  Self-improvement nightly loop
                                                              └─ P19  Sim layer (PlayerZero pattern)

Frontend track (parallel to P10+):
        F0  Next.js 16 scaffold + auth
            └─ F1  Workspace board (kanban, run viewer)
                    └─ F2  Live run streaming (WS)
                            └─ F3  KG visualizer
                                    └─ F4  Skill registry UI

Vertical track (after P10 ships):
        V0  motadata-itsm plugin authoring scenario
            └─ V1  Real customer requirement → first hivecore PR
```

## Milestone definitions

### M1 — KG substrate working
Goal: KG ingests motadata-itsm code + ADR samples, queries return useful blast-radius and skill-ranking results, multi-tenant isolation enforced.
Phases: P0, P1, P2, P3, P4, P5.

### M2 — Single persona end-to-end
Goal: Dev persona runs a real task in sandbox, KG-scoped context, harness gates green, opens PR.
Phases: P6, P7, P8, P9, P10, P11, P12.

### M3 — Multi-persona workflow
Goal: Full workflow DAG (PM → Dev → QA → Review → Deploy) executes a customer-requirement → merged-PR pipeline.
Phases: P13, P14, P15, P16, P17.

### M4 — Self-improvement + sim
Goal: Nightly loop compounds skills; sim layer predicts blast radius pre-merge.
Phases: P18, P19.

### M5 — Frontend + first vertical
Goal: Public board + run viewer; motadata-itsm plugin authoring proven on real customer cases.
Phases: F0–F4, V0, V1.

## Conventions

- Phase numbering is stable; new urgent work inserts as decimal phases (e.g. 7.1, 7.2).
- Each phase has its own directory under `.planning/phases/<NN>-<slug>/` (managed by GSD harness).
- ADRs live in [DECISIONS.md](./DECISIONS.md), numbered globally.
- "Done" = scenario green + tests + docs updated + PR merged to main.

## Non-goals

- No public managed cloud until M5+.
- No vertical beyond motadata-itsm until M5 ships.
- No SOC2 certification until commercial launch (architecture future-proofs for it).
