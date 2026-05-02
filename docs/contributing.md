# Contributing

Hivecore welcomes contributions. This doc is canonical; the root [CONTRIBUTING.md](../CONTRIBUTING.md) points here.

## Before you start

1. Read [introduction.md](./introduction.md), [concepts.md](./concepts.md), [architecture.md](./architecture.md), and [philosophy.md](./philosophy.md). Internalize the model before changing it.
2. Browse [DECISIONS.md](./DECISIONS.md) — the why of current design.
3. Check [roadmap.md](./roadmap.md) for the current focus phase.

## Workflow

### Small changes (typo, doc fix, small bugfix)

1. Fork, branch (`fix/<scope>` or `docs/<scope>`).
2. Make the change.
3. Run all dev commands (see below). All must pass.
4. Update `CHANGELOG.md` in `[Unreleased]` if user-visible.
5. Open a PR.

### Non-trivial changes (new feature, breaking change, architecture shift)

1. Open an issue with the `rfc:` label first. Describe the proposal and tradeoffs.
2. Wait one week for community feedback (or shorter if maintainers ack).
3. If accepted, follow the small-change workflow but also:
   - Add an ADR to [DECISIONS.md](./DECISIONS.md).
   - Update [architecture.md](./architecture.md) if structural.
   - Update [comparison.md](./comparison.md) if positioning shifts.
4. Open the PR; reference the RFC issue.

### TDD-spike-attach pattern

Hivecore uses a strict spike-then-attach workflow:

1. Create branch `spike/<topic>` for exploratory work.
2. Write the scenario file in `scenarios/<topic>.yaml` defining what proves it works.
3. Implement until scenario passes in a sandbox.
4. Harden: write proper tests mirroring the scenario.
5. Update `docs/` with concept/architecture deltas.
6. Add ADR if architecturally significant.
7. PR from spike branch → main, atomic squash-merge.
8. Phase manifest in `.planning/phases/<NN>-<slug>/manifest.json` records the merge.

## Dev commands

```
cargo check --workspace
cargo test --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
pnpm --filter ./apps/web lint
pnpm --filter ./apps/web typecheck
pnpm --filter ./apps/web test
```

All seven must pass before merge. CI enforces.

## Branch naming

- `feature/<scope>` — new feature
- `fix/<scope>` — bug fix
- `docs/<scope>` — documentation only
- `rfc/<name>` — RFC implementation
- `spike/<topic>` — TDD scratch (never merged directly)
- `refactor/<scope>` — internal restructure

## Commit messages

Imperative mood. One logical change per commit.

Good:
- `Add KG ingestion for ADR documents`
- `Fix bi-temporal validity-window edge case`

Bad:
- `Added some stuff`
- `Updated KG`

## PR checklist

- [ ] Tests pass (`cargo test --workspace`)
- [ ] Format clean (`cargo fmt --all --check`)
- [ ] Lint clean (`cargo clippy ... -D warnings`)
- [ ] Frontend checks pass (if `apps/web` touched)
- [ ] `CHANGELOG.md` updated (if user-visible)
- [ ] Docs updated (if behavior changed)
- [ ] ADR added (if architecturally significant)
- [ ] Spike branch closed (if applicable)
- [ ] No new TODO without an issue link

## Skill / persona / workflow contributions

Skills, personas, and workflows are config artifacts, not code. To contribute:

1. Place under `.hivecore/personas/`, `.hivecore/workflows/`, or `.hivecore/skills/` in a sample-org subdir (e.g., `examples/sample-org-fintech/.hivecore/personas/compliance-officer.toml`).
2. Document its purpose in `examples/<org>/README.md`.
3. Include a scenario demonstrating it under `examples/<org>/scenarios/`.

Org-private personas/workflows/skills should NOT be contributed upstream — they live in your fork or your private repo's `.hivecore/`.

## Governance

See [governance.md](./governance.md) for decision-making process and maintainer roles.

## Questions

Open a discussion (not an issue) for questions. Bugs and feature requests get issues.
