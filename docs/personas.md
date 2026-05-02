# Personas — moonshot view

How hivecore looks at end-state, per persona. Each section: who they are, what they do on the platform, the diagram.

Hivecore ships six built-in persona templates as TOML config. Orgs add their own. Personas are not code — they are config + UI surface + workflow lane + KG access scope + worker dispatch policy.

Mermaid sources live in [.research/diagrams/](#) (to be moved); rendered SVG/PNG under [`./diagrams/`](./diagrams/).

---

## 0. Whole platform (anchor)

Six core layers in Rust, four worker adapters, one Next.js UI. Workers (Claude Code, Codex, Pi, OpenCode) are external — hivecore dispatches to them via the adapter trait.

![Platform](./diagrams/00-platform.svg)

---

## 1. PMG Lead

Drafts product specs in markdown editor. KG surfaces past specs, ADRs, customer asks inline. Publish → workflow auto-files dev tasks with full context bundle. Releases tracked on kanban. Maps to PMG Workbench (Phase 7, motadata-itsm vertical).

![PMG](./diagrams/01-pmg.svg)

---

## 2. Developer

Claims task on native board. Dispatches to Claude Code worker. Worker edits inside Firecracker sandbox. Validator runs scenarios from KG. Diff + scenario results land back on the card. Dev approves merge. Run journaled to KG.

![Developer](./diagrams/02-dev.svg)

---

## 3. QA / Validator

Authors scenarios in UI form (no code path required). Scenarios attach to workflow as merge gates. Replay engine pulls prod traffic samples. Coverage gaps surfaced. Validation replaces review (StrongDM Software Factory pattern).

![QA](./diagrams/03-qa.svg)

---

## 4. Compliance Officer (custom persona)

Proves the "any role" claim. Authors policy in TOML/UI. Policy engine intercepts every agent action. Audit log append-only and signed. Live dashboard. Kill switch halts all workers org-wide.

![Compliance](./diagrams/04-compliance.svg)

---

## 5. ITSM Plugin Author (first vertical)

PMG Workbench in production use. ITSM ontology overlay (`.hivecore/ontology/itsm.toml`) extends KG. Plugin Author drafts plugin spec → workflow dispatches to Claude Code → ITSM scenario suite validates → roadshow viewer for collab + history. PMG head is design partner per ADR-015.

![ITSM](./diagrams/05-itsm.svg)

---

## 6. Org Admin

Provisions tenants (Postgres RLS). Sets per-persona budget caps. Watches KG growth via visualizer. Exports compliance reports from audit log. Telemetry shows cost + latency per persona.

![Admin](./diagrams/06-admin.svg)

---

## How a new org defines a custom persona

```toml
# .hivecore/personas/firmware-reviewer.toml
[persona]
id          = "firmware-reviewer"
name        = "Firmware Reviewer"
description = "Reviews firmware diffs against safety + compliance policy."

[ui]
surface     = "review-board"      # which UI panel they get
permissions = ["read:kg", "write:audit", "approve:firmware-merge"]

[kg.access]
subgraphs = ["code", "run", "policy", "tribal"]
overlays  = ["firmware-domain"]

[workflow.lanes]
gates_at  = ["pre-merge:firmware/*"]
dispatch  = "claude-code"          # which worker handles their tasks
fallback  = "codex"

[budget]
monthly_usd = 2000
hard_cap    = true

[policy]
require    = ["2-of-3-approval", "scenario-pass:firmware-safety"]
```

Drop the file, hot-reload, the persona shows up in UI with their own lane on the board, their own KG view, their own dispatch rules.

---

## Mapping to roadmap phases

| Persona              | First appears in phase | Full UI in phase |
|----------------------|------------------------|------------------|
| Developer            | P1 (Hello UI)          | P4 (Native Board)|
| Org Admin            | P1                     | P6               |
| PMG Lead             | P4                     | P7 (PMG Workbench)|
| QA / Validator       | P5                     | P5               |
| Compliance Officer   | P6                     | P6               |
| ITSM Plugin Author   | P7                     | P7               |

See [roadmap.md](./roadmap.md) for phase contents.
