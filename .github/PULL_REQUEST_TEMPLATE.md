## What

<!-- One sentence: what this PR does. -->

## Why

<!-- Link to issue or rfc:. Brief reason. -->

## How

<!-- Approach in 2-4 bullets. Architectural notes if relevant. -->

## Checklist

- [ ] `cargo check --workspace` passes
- [ ] `cargo test --workspace` passes
- [ ] `cargo fmt --all --check` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] `pnpm --filter ./apps/web lint` passes (if frontend touched)
- [ ] `pnpm --filter ./apps/web typecheck` passes (if frontend touched)
- [ ] `pnpm --filter ./apps/web test` passes (if frontend touched)
- [ ] `CHANGELOG.md` updated under `[Unreleased]` (if user-visible)
- [ ] Docs updated (if behavior changed)
- [ ] ADR added in `docs/DECISIONS.md` (if architecturally significant)
- [ ] Scenario added/updated in `scenarios/` (if new feature)
- [ ] Spike branch closed (if applicable)
