# Core thesis

Hivecore is the **OSS, UI-first, complete platform for org-customizable AI SDLC harnesses** — board + daemon + KG + orchestrator + harness + sandbox + UI + CLI under one umbrella, self-hosted, with every capability exposed through a polished Next.js workspace UI as the primary surface.

## The bet

The Claude Code paper (arxiv 2604.14228) found 98.4% of an agent's value sits in the deterministic infrastructure around the model — not in the model itself. As foundation models converge, the substrate becomes the moat. Internal "AI software factories" (Stripe Minions 1.3k PRs/wk, Spotify Honk 1.5k PRs, OpenAI Harness 1M LOC zero-code) prove the economics. None of them are adoptable by other orgs.

Hivecore makes the pattern adoptable.

## Two-layer model

```
┌─────────────────────────────────────────────────┐
│  Org user-space                                  │
│  - Custom personas (TOML)                        │
│  - Custom workflow DAG (TOML)                    │
│  - Custom policies + governance (TOML)           │
│  - Custom KG ontology overlays (TOML)            │
│  - Skills, scenarios, runtimes (config)          │
└────────────────────┬────────────────────────────┘
                     │
┌────────────────────▼────────────────────────────┐
│  Hivecore kernel (OSS, Rust + Next.js)          │
│  - KG engine (multi-tenant, bi-temporal)         │
│  - Durable orchestrator (journal, replay, saga)  │
│  - Coordination protocol (file ownership)        │
│  - Persona harness (5-layer)                     │
│  - Gate engine (computational + inferential)     │
│  - Skill registry (signed, versioned)            │
│  - Scenario validation engine                    │
│  - Sandbox runner (Firecracker)                  │
│  - Runtime adapters (CC, Codex, Pi, custom)      │
└─────────────────────────────────────────────────┘
```

The kernel is hivecore. User-space is whatever the org configures. Code changes ship the kernel; config changes ship the org's harness.

## Validation hierarchy

Hivecore eats its own dog food at multiple levels:

1. **Self-build** — hivecore is built using GSD + the same TDD-spike-then-attach discipline it preaches.
2. **First vertical** — motadata-itsm plugin authoring proves the kernel against real customer customizations.
3. **OSS adoption** — once kernel + first vertical are stable, other orgs adopt and contribute back to the user-space pattern library.

## Why this is unique

Every existing platform hardcodes some combination of the three customization surfaces:

| Platform | Persona | Workflow | Control plane |
|---|---|---|---|
| Multica / Paperclip | partial | hardcoded | partial |
| AI-SDLC Framework | hardcoded | partial | first-class |
| StrongDM Software Factory | internal-only | hardcoded | internal-only |
| BMad Method | fixed at 21 agents | hardcoded | none |
| Spec-Kit / Kiro | hardcoded | hardcoded | none |
| LangGraph / CrewAI / AutoGen | DIY (no SDLC opinion) | DIY | none |
| **Hivecore** | **first-class config** | **first-class config** | **first-class config** |

Plus hivecore is the only one with **KG-at-core** (not bolted on) **+ multi-tenant Knowledge Cores** (per-org isolation enforced at retrieval) **+ OSS** (Apache+MIT, self-hostable, contribution-friendly).

## What success looks like

Pre-v1.0:
- Kernel phases (P0–P5) ship: KG substrate working with motadata-itsm subset + multi-tenant isolation enforced.
- First persona (Dev) opens real PRs end-to-end with ≥75% merge rate on plugin authoring tasks.

v1.0 GA:
- Full workflow DAG executes (PM → Dev → QA → Review → Deploy) on real customer requirements.
- Self-improvement nightly loop compounds skills measurably.
- Sim layer (P19) predicts blast radius pre-merge.

Post-v1.0:
- ≥3 external organizations adopt and contribute personas/workflows back.
- KG ontology versions reach broad community agreement.
- "Hivecore-compatible" becomes a category descriptor.
