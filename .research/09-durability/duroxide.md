---
url: https://github.com/microsoft/duroxide
status: active-reference
checked: 2026-04-29
tags: [durable-execution, journal-replay, saga, rust, oss]
---

**Why it matters**: Substrate dependency for `hivecore-orchestrator` (per ADR-012). Collapses ~30% of P3 scope.

**Anchor**: "Lightweight and embeddable durable execution runtime for Rust. Inspired by Durable Task Framework and Temporal." Function chaining, fan-out/fan-in, durable timers, exactly-once resume on crash. v0.1.27 (Feb-Apr 2026), MIT, Microsoft-maintained.

**What we adopt**: dependency for orchestrator — wrap with persona-state + workflow-DAG semantics. Pin specific version; vendor snapshot if upstream stalls.
