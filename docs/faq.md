# FAQ

## What is hivecore in one sentence?

OSS, UI-first, complete platform for org-customizable AI SDLC harnesses: native board + daemon + KG + orchestrator + harness + sandbox + UI + CLI under one umbrella.

## How is hivecore different from multica or paperclip?

Multica and paperclip provide board + daemon + agent runtime — solid infrastructure, but personas and workflow are largely fixed and there's no KG, no governance/policy plane, no validation engine. Hivecore ships its own native board + daemon (so you don't run two stacks), adds KG-at-core, makes personas/workflows/policies customizable as both TOML and UI editors, ships scenario+replay validation, and bundles policy/audit dashboards. **In one sentence: hivecore is the complete platform; multica/paperclip are subsets.**

## Do I need multica or paperclip alongside hivecore?

No. Hivecore ships its own daemon and board natively (per ADR-014). One stack to deploy.

## How is hivecore different from PlayerZero or Bito?

PlayerZero and Bito provide knowledge graphs over codebases, plus agents on top. They are vertically integrated for their own workflow. Hivecore is the platform substrate: KG + customizable personas + customizable workflow DAG + control plane, OSS, with org-private isolation. PlayerZero's Sim-1 is an aspirational reference for hivecore's P19 sim layer.

## Can I use hivecore with Claude Code? Codex? Cursor?

Yes — runtime adapters are pluggable. Hivecore ships adapters for Claude Code, Codex, OpenCode, and Pi. Add your own by implementing the `RuntimeAdapter` trait.

## Do I need Postgres?

Yes. Postgres 17 + pgvector is the storage layer. SQLite-only mode may come post-v1.0 for solo dev use; not a v1 priority.

## Do I need Firecracker?

For production multi-tenant deployments, yes. For local dev and trusted single-tenant, Docker is supported as a sandbox runner.

## Is hivecore a SaaS?

No. Hivecore is OSS, self-hosted by default. A managed cloud may follow once OSS matures, never instead.

## What is the first vertical?

motadata-itsm plugin authoring. The author has domain expertise + customer access there. The platform is SDLC-general; ITSM is the validation case.

## Why Rust for backend?

KG engine + bi-temporal layer + durable orchestrator are perf-sensitive and benefit from zero-cost abstractions and no-GC determinism. Firecracker integration is Rust-native. The "98.4% infrastructure" finding from the Claude Code paper means the substrate quality matters more than agent quality — Rust gives that quality.

## Why Next.js for frontend?

App Router maturity + TypeScript end-to-end + React Query + Zustand is the modern fast stack. shadcn/ui gives polished components without lock-in.

## Can I use hivecore without writing my own personas?

Yes — hivecore ships reference personas (PM, Arch, Dev, QA, Review, Deploy) as starting examples. Most orgs will customize; some will run defaults.

## How does multi-tenancy work?

Each tenant = one Knowledge Core (isolated graph instance). Permission-aware retrieval at query layer prevents cross-tenant leakage. Audit log per tenant, hash-chained.

## What about regulatory compliance (SOC 2, HIPAA, PCI)?

Architecture is designed-in for SOC 2. Certification deferred until commercial launch. HIPAA/PCI require additional hardening; supported but not certified.

## How do I contribute?

See [contributing.md](./contributing.md). TL;DR: open an `rfc:` issue for non-trivial changes, follow the spike-then-attach workflow, run all dev commands before PR.

## Is hivecore production-ready?

Pre-v1.0. Currently building the substrate phases (P0–P5). Track [roadmap.md](./roadmap.md) for status.

## Where does the name come from?

Hive (multiple workers, swarm coordination) + core (substrate, kernel). The platform is the core; orgs build their own hive on top.
