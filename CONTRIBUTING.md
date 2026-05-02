# Contributing

The canonical contributor guide lives at [**docs/contributing.md**](./docs/contributing.md). Start there.

Quick summary:

- Open an `rfc:`-labelled issue before large PRs.
- Run `cargo check --workspace && cargo test --workspace && cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && pnpm --filter ./apps/web lint && pnpm --filter ./apps/web typecheck && pnpm --filter ./apps/web test` before submitting.
- Be kind. See [CODE_OF_CONDUCT.md](./CODE_OF_CONDUCT.md).
- Governance and decision-making: [docs/governance.md](./docs/governance.md).
