# Session start

Ten-minute warmup for new agent sessions. Read this in order.

## 1. What is hivecore?

OSS substrate for org-customizable AI SDLC harnesses. Companies clone hivecore and define their own personas, workflow DAG, and control plane. KG at core. Validation replaces review. Repo-as-harness.

If you are an agent and have not read [introduction.md](./introduction.md), [concepts.md](./concepts.md), and [philosophy.md](./philosophy.md), do that first.

## 2. Where are we?

Check `.planning/STATE.md` and `git log --oneline -20`. The phase manifests in `.planning/phases/<NN>-<slug>/manifest.json` say what is done, in progress, or blocked.

Currently focused on: Phase 0 — repo init + scaffold. Track progress in [roadmap.md](./roadmap.md).

## 3. What can you change without asking?

- Trivial fixes (typo, comment, tightening a doc).
- Scoped feature work inside an open phase, following the spike-then-attach workflow.
- Adding research pointers to `.research/` (10-line max per source).

## 4. What requires an `rfc:` issue first?

- Any change to KG ontology (node/edge types).
- Any change to persona/workflow/policy config schema.
- Any new crate.
- Any breaking API change.
- Any change to public docs that shifts positioning.

## 5. Dev commands

```
cargo check --workspace
cargo test --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
pnpm --filter ./apps/web lint
pnpm --filter ./apps/web typecheck
pnpm --filter ./apps/web test
```

All seven must pass before merge.

## 6. Critical rules (memorize)

- Core library crates I/O-free (`crates/*-core/`).
- Personas/workflows/policies are CONFIG (TOML), not code.
- KG ontology changes need an ADR.
- TDD: spike branch → scenario passes → tests → docs → main.
- Frontend: React Query for server, Zustand for client; WS invalidates queries, never writes stores.
- Multi-tenant isolation enforced at retrieval, not at prompt.
- Durable execution from day one; every run journaled.

## 7. Tools you will use most

- GSD harness (`/gsd-*` skills) for phase work.
- Onelens / onelens-palace for code KG queries.
- Context7 / find-docs for library API lookups.
- TaskCreate / TaskUpdate for multi-step work.

## 8. Files an agent should never overwrite without explicit instruction

- `LICENSE` (Apache-2.0 — see ADR-030)
- `CODE_OF_CONDUCT.md`, `SECURITY.md`
- `docs/DECISIONS.md` (append-only — only add new ADRs)
- `CHANGELOG.md` (append-only — only update `[Unreleased]` and add new versions)

## 9. Where to find more

- High-level: [vision.md](./vision.md), [philosophy.md](./philosophy.md)
- Technical: [architecture.md](./architecture.md), [concepts.md](./concepts.md)
- Process: [contributing.md](./contributing.md), [governance.md](./governance.md)
- Compare: [comparison.md](./comparison.md)
- History: [DECISIONS.md](./DECISIONS.md)
- Research bookmarks: [.research/sources.md](../.research/sources.md)

## 10. If you are stuck

Surface the blocker explicitly. Ask one focused question. Do not guess at architecture decisions; either find an existing ADR or open an `rfc:` issue.
