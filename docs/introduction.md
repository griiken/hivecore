# Introduction

Hivecore is the **OSS complete platform for org-customizable AI SDLC harnesses** — board + daemon + KG + orchestrator + harness + sandbox + UI + CLI under one umbrella, self-hosted, **UI-first**.

## The problem

Every company building AI-driven SDLC automation today faces the same wall: off-the-shelf coding agents (Claude Code, Codex, Cursor) are powerful but generic; internal "AI software factories" (Stripe Minions, Spotify Honk, Ramp Inspect, OpenAI Harness) are domain-specific but locked away. Existing OSS board+daemon platforms (multica, paperclip) cover task management but lack KG, customizable personas, or governance. SDLC-platform OSS (ai-sdlc-framework) provides governance but lacks KG. Spec-driven tools (Spec-Kit, BMad, Kiro) define methodology but leave personas and governance up to you.

No single OSS platform combines: native board+daemon, knowledge-graph-at-core context, customizable personas, customizable workflow DAG, first-class control plane, and a polished UI as the primary surface. That gap is hivecore.

## What hivecore does

Hivecore is the **complete stack** any organization can clone, deploy, and configure to run its own SDLC harness:

- **Native board + daemon** — built-in kanban board for tasks; built-in daemon runner that executes agent runs in sandboxes and streams events back. No external multica/paperclip dependency.
- **UI-first surfaces** — every capability has a Next.js workspace UI (board, run viewer, KG visualizer, visual persona editor, workflow DAG editor, scenario authoring, policy + audit dashboards, tenant + ontology admin, daemon management). TOML config + CLI are power-user fallbacks; UI generates the underlying config so git versioning + AI-readability are preserved.
- **Custom personas** — declare roles beyond just PM/Dev/QA: compliance officer, firmware reviewer, license auditor, security analyst, anything. Visual editor or TOML.
- **Custom workflow** — state machine as DAG (TOML or n8n-style visual editor). Define your own stages, transitions, and gates.
- **Custom control plane** — governance, policies, budgets, approval rules, audit, kill switches, all configurable through UI.
- **KG at core** — multi-tenant bi-temporal knowledge graph spanning code, requirements, ADRs, tests, deployments, incidents, skills, runs, tribal knowledge. Permission-aware retrieval; cross-tenant queries forbidden at the retrieval layer.
- **Repo-as-harness** — `.hivecore/` directory in each repo carries org persona/workflow/policy config, scenarios, skills, run state. Repo is the harness, not the prompt.
- **Validation replaces review** — scenario-based execution against digital-twin mocks of dependent systems plus production-traffic replay; the merge gate is "all scenarios pass," not "human approved."

## Who it's for

- Engineering organizations that want AI agents handling the long tail of SDLC work (toil, customizations, domain-specific authoring) but need full control over how, when, and by whom.
- Teams whose domain has roles, gates, or policies generic platforms cannot express.
- Anyone willing to write config (personas, workflow, ontology) instead of waiting for vendor features.

## Reading order

1. [vision.md](./vision.md) — moonshot
2. [concepts.md](./concepts.md) — core nouns
3. [architecture.md](./architecture.md) — system layers
4. [philosophy.md](./philosophy.md) — design principles
5. [comparison.md](./comparison.md) — vs alternatives
6. [roadmap.md](./roadmap.md) — phases and dependencies
7. [DECISIONS.md](./DECISIONS.md) — append-only ADR log
