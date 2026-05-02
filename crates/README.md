# crates/

Rust workspace member crates. Each crate has a single responsibility and an explicit boundary contract.

## Layout

Crates land here as phases ship. Inventory documented in [../docs/architecture.md](../docs/architecture.md).

## Conventions

- `*-core` crates are I/O-free. Pure domain logic, types, traits.
- `*-io`, `*-runtime`, `adapter-*` crates do I/O. Wire to `*-core` types.
- Member `Cargo.toml` uses `workspace = true` for shared deps and metadata.
- Cross-crate dependencies must be explicit in `Cargo.toml`; no transitive surprises.
- Each crate has a `README.md` with one-line description and a public-API summary.
- Each new crate that introduces architectural boundary requires an ADR.

## Hooks enforce

- `.claude/hooks/check-no-io.sh` warns if `*-core` crates import I/O.
- Future: `.claude/hooks/check-cross-adapter.sh` warns on cross-adapter imports.
