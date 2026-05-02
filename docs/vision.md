# Vision

> **Edit this file with your moonshot before first launch.**

## North star

Every engineering organization runs an AI-driven SDLC harness tuned to their domain, governance, and team — not a generic vendor's opinion. Hivecore is the OSS, UI-first, complete platform (board + daemon + KG + harness + UI) that makes this possible to deploy in hours, customize through visual editors, and scale from solo dev to multi-tenant enterprise.

## What "winning" looks like

Five years out:

- Hundreds of organizations run hivecore as their SDLC harness substrate.
- Each has its own persona library, workflow DAG, and control plane — none looking exactly alike.
- Skills compound across the community: a fintech-specific compliance-officer persona authored at one org gets shared, reviewed, and reused at others (with org-private isolation preserved).
- "Validation replaces review" becomes the dominant pattern: agents author code, scenarios validate it against digital-twin mocks of dependent systems, humans only intervene at policy boundaries.
- Hivecore's KG layer becomes the de-facto open standard for SDLC knowledge graphs, ingested by agents from any vendor.

## What hivecore is NOT

- Not a coding agent (those are runtime adapters underneath).
- Not a board+daemon (multica/paperclip do that — hivecore consumes them).
- Not a hardcoded SDLC methodology (Spec-Kit, BMad, Kiro do that — hivecore lets you write your own).
- Not a vertical-specific tool (motadata-itsm is the first vertical for validation, not the product).
- Not a SaaS — OSS first, self-hosted by default. A managed cloud may follow, never instead.

## Why now

- Foundation models converged on baseline reasoning quality (per Claude Code paper, 98.4% of agent value sits in the deterministic harness around the model, not the model).
- Durable execution went from optional to baseline (1.86T AI agent executions on Temporal alone).
- Knowledge graphs proved out at production scale (PlayerZero Sim-1, Bito, Tabnine ECE, GitLab gkg).
- Internal "software factories" demonstrated the economics (Stripe 1.3k PRs/wk, Spotify 1.5k PRs, OpenAI Harness 1M LOC zero-code in 5 months).
- Generic OSS that combines all of the above does not exist.

## First milestone

- Phase 0–10 of [roadmap.md](./roadmap.md) ship: KG substrate + durable orchestrator + coordination + first Dev-role harness end-to-end.
- First vertical (motadata-itsm plugin authoring) demonstrates value with real customer customizations.
- First public release tagged.
