# Research

Lean reference pointers, not content dumps.

## Format spec

Every research entry is a 10-line maximum file in YAML frontmatter + 2-3 prose lines. URL is canonical; we do not copy content.

```yaml
---
url: https://example.com/path
status: active-reference | superseded | archived
checked: 2026-04-29
tags: [tag1, tag2, tag3]
---

**Why it matters**: <one sentence — what we steal from this>

**Anchor**: <the one fact/quote we'd lose if URL dies>

**What we adopt**: <pattern, schema, principle>
```

## Status values

- `active-reference` — currently informing decisions
- `superseded` — replaced by another source (link via `replaced-by:` field)
- `archived` — URL dead, anchor preserved
- `disputed` — found contradicting source; see `disputed-by:` field

## Maintenance rule

Quarterly URL liveness check. HTTP 404 → bump to `archived` (anchor-fact preserved). Content drift from anchor → bump to `superseded` and add a successor pointer.

## Tag taxonomy

- `kg`, `kg-temporal`, `kg-multi-tenant`, `kg-schema`
- `harness`, `harness-5-layer`, `harness-claude-code`
- `agent`, `agent-coding`, `agent-orchestration`, `agent-runtime`
- `coordination`, `concurrency`, `worktree`
- `validation`, `replay`, `digital-twin`, `gates`
- `spec-driven`, `spec-as-source`
- `durable-execution`, `journal-replay`, `saga`
- `skills`, `skill-registry`, `provenance`
- `internal-factory`, `production-pattern`
- `world-model`, `simulation`
- `vertical-itsm`, `vertical-other`
- `governance`, `policy`, `multi-tenant`
- `tribal-knowledge`, `org-memory`
- `benchmark`

## Sections

| Dir | Topic |
|---|---|
| `00-thesis/` | Our own thinking — core thesis, wedge, end goal |
| `01-landscape/` | Tier-by-tier competitor and adjacent project map |
| `02-internal-factories/` | Stripe, Spotify, Ramp, OpenAI Harness, StrongDM, Cloudflare |
| `03-kg-and-context/` | KG engines, context engines, code digital twins |
| `04-spec-driven/` | Spec-driven development tools and patterns |
| `05-harness-engineering/` | Harness architecture papers, practitioner guides |
| `06-verification/` | Replay verification, AI code review, GNN vuln detection |
| `07-self-improvement/` | Nightly loops, generator-evaluator isolation |
| `08-coordination/` | File ownership, message passing, multi-agent concurrency |
| `09-durability/` | Durable execution patterns, Temporal/Restate, saga |
| `10-skill-registry/` | Skill registries, signing, provenance |
| `11-multi-tenant/` | Knowledge core isolation, permission-aware retrieval |
| `12-benchmarks/` | SWE-Bench Pro, Terminal-Bench, scenario benchmarks |
| `13-org-memory/` | Tribal knowledge capture, ADR rationale spine |
| `14-vertical-validation/` | First vertical (motadata-itsm) context |

## Master index

See [sources.md](./sources.md) for the deduplicated URL index, tag-classified.

## Adding a pointer

1. Find the right section dir.
2. File name: kebab-case slug of the source (e.g., `playerzero-sim1.md`).
3. Use the format spec above.
4. Add the URL row to `sources.md` with appropriate tags.
5. Reference it from a `docs/` file if it informed a decision.

## Anti-patterns

- Don't copy paragraphs. Quote one line max as the anchor.
- Don't write summaries. The "why it matters" is enough.
- Don't add pointers proactively for "completeness". Add only when cited from a decision or design doc.
- Don't keep dead URLs without archiving. Either preserve anchor + mark archived, or remove if irrelevant.
