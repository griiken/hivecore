# spikes/

TDD scratch space. Each spike branch creates a subdir here for exploratory work that has not yet been hardened.

## Workflow

1. Create branch `spike/<topic>`.
2. Create `spikes/<topic>/` with experimental code.
3. Write `scenarios/<topic>.yaml` defining what proves it works.
4. Iterate until scenario passes.
5. Harden: extract validated logic into `crates/` or `apps/web/`, write proper tests, update docs.
6. Open PR `spike/<topic> → main`. Squash-merge.
7. Spike subdir deleted at merge (or moved to `examples/` if useful as reference).

## Conventions

- Spikes are **never** depended on by main code.
- Spikes that don't pan out get deleted; do not hoard.
- Spike code does not need to follow critical rules (I/O in core, etc.) — that's the point.
- `target/` and `node_modules/` inside spikes are gitignored.
