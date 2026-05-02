# cli/

Hivecore CLI binary. Rust, single binary distribution.

Phase 0 produces a minimal `hivecore` binary that can:

- `hivecore init` — scaffold `.hivecore/` in a target repo
- `hivecore version` — print version + build info

Subsequent phases add subcommands:

- `hivecore persona add|list|remove` (P9)
- `hivecore workflow set|list` (P9)
- `hivecore run "<task>"` (P10)
- `hivecore daemon start|stop|status` (P10)
- `hivecore kg query "<cypher-like>"` (P4)
- `hivecore skill add|list|publish` (P13)

See [docs/architecture.md](../docs/architecture.md) for the full CLI inventory.
