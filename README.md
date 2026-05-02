<div align="center">

# Hivecore

**OSS substrate for org-customizable AI SDLC harnesses.**

[Docs](./docs/introduction.md) · [Vision](./docs/vision.md) · [Concepts](./docs/concepts.md) · [Architecture](./docs/architecture.md) · [Roadmap](./docs/roadmap.md) · [Decisions](./docs/DECISIONS.md) · [Session warmup](./docs/SESSION-START.md)

</div>

---

> ⚠️ Just scaffolded with `project-ignition`. Sections below are starter templates — edit `README.md`, `docs/vision.md`, and `docs/roadmap.md` before first launch.

## What this does

Hivecore is the OSS substrate for AI-driven SDLC orchestration. Companies clone hivecore and define **their own personas** (roles beyond just PM/Dev/QA — compliance officer, firmware reviewer, license auditor, anything), **their own workflow** (DAG state machine as config, not code), and **their own control plane** (governance, policies, budgets, audit, kill switches). Hivecore provides the substrate; orgs provide opinions.

Built on a multi-tenant temporal knowledge graph (KG-at-core), durable orchestration, and the repo-as-harness pattern. Validation replaces code review.

## Install

```
# placeholder — see docs/quickstart.md once Phase 0 ships
cargo install --path cli
```

## Quickstart

```
hivecore init                # scaffold .hivecore/ in your repo
hivecore persona add dev     # declare a persona
hivecore workflow set default.dag.toml
hivecore run "fix flaky test in auth module"
```

## Status

- Current focus: Phase 0 — KG substrate primitives + persona/workflow config schema.
- Next: see [docs/roadmap.md](./docs/roadmap.md).

## Stack

- **Backend**: Rust (axum, sqlx, tokio) — single binary
- **Frontend**: TypeScript + Next.js 16 (App Router)
- **Storage**: Postgres 17 + pgvector
- **Sandbox**: Firecracker microVMs
- **Wire**: gRPC + WebSocket
- **Telemetry**: OpenTelemetry

## License

Dual-licensed under **MIT OR Apache-2.0**. See [LICENSE-MIT](./LICENSE-MIT), [LICENSE-APACHE](./LICENSE-APACHE), and [docs/terms.md](./docs/terms.md).

## Contributing

See [docs/contributing.md](./docs/contributing.md). Hivecore's [Code of Conduct](./CODE_OF_CONDUCT.md) applies to every contribution.
