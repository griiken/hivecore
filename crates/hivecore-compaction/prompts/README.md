# Vendored compaction prompts

Per ADR-024, vendoring upstream prompts requires explicit attribution + a license-compatibility note.

## `summary.md`

- **Source.** `badlogic/pi-mono`, file `packages/coding-agent/src/core/compaction/compaction.ts`, constant `SUMMARIZATION_PROMPT` (the template literal beginning `The messages above are a conversation to summarize…`).
- **License.** MIT (kept under MIT, attributed; MIT permits inclusion in an Apache-2.0 distribution per Apache-2.0 §4.1 — see ADR-030 for hivecore's Apache-2.0-only licensing).
- **Synced from.** `https://github.com/badlogic/pi-mono` (branch `main`).
- **Sync date.** 2026-05-01.

## `summary_prefix.md`

- **Source.** `openai/codex`, file `codex-rs/core/templates/compact/summary_prefix.md`.
- **License.** Apache-2.0 — same licence as hivecore (ADR-030); no compatibility note needed.
- **Synced from.** `https://github.com/openai/codex` (branch `main`).
- **Sync date.** 2026-05-01.

## Sync hygiene

To refresh:

```
curl -fsSL 'https://raw.githubusercontent.com/openai/codex/main/codex-rs/core/templates/compact/summary_prefix.md' \
  -o crates/hivecore-compaction/prompts/summary_prefix.md
```

Re-read the upstream `compaction.ts` file for `SUMMARIZATION_PROMPT` updates and patch `summary.md` by hand if changed. Bump the sync date here.
