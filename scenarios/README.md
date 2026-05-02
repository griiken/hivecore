# scenarios/

Validation scenarios — declarative cases that prove a phase, persona, or workflow works.

## Format

```yaml
# scenarios/<phase-or-feature>.yaml
name: kg-engine-v0
phase: P1
description: Insert nodes/edges, query by tag, validate isolation between Knowledge Cores.

setup:
  - postgres: 17
    extensions: [pgvector]
  - hivecore: dev-mode

steps:
  - name: insert-nodes
    action: kg.upsert-nodes
    input: fixtures/p1-nodes.json
    expect:
      count: 100

  - name: query-by-tag
    action: kg.query
    input:
      query: 'MATCH (n:Function) WHERE n.name = "compute_sla" RETURN n'
    expect:
      min_results: 1

  - name: cross-tenant-rejected
    action: kg.query
    tenant: tenant-a
    input:
      query: 'MATCH (n) WHERE n.tenant_id = "tenant-b" RETURN n'
    expect:
      rejected: true
      error_code: cross_tenant_query_forbidden

teardown:
  - cleanup: yes
```

## Conventions

- One scenario per phase or feature.
- Scenarios are the spec's ground truth; if scenario passes, the spec is satisfied.
- Fixtures (test data) under `scenarios/fixtures/`.
- Scenarios run in CI on every PR via `cargo test --workspace -- --ignored scenario_*`.

## Replacing tests?

Scenarios are end-to-end. They do not replace unit tests. The pyramid:

1. **Scenarios** (this dir) — end-to-end declarative cases (slowest, most confidence).
2. **Integration tests** in `tests/` per crate (medium speed).
3. **Unit tests** in `#[cfg(test)]` mods (fastest).

A phase ships when:
- Scenario passes.
- Unit tests cover all critical branches.
- Integration tests cover the public API contract.
