---
url: https://github.com/shantanugoel/oxydra
status: active-reference
checked: 2026-04-29
tags: [harness, agent-orchestration, rust, oss, reference-impl]
---

**Why it matters**: Reference impl for Rust agent orchestration patterns. Read source before P4 (`hivecore-persona`) and P9 (config schemas). Per ADR-013, NOT a dependency.

**Anchor**: "Rust-based AI agent orchestrator with always-evolving + self-learning agents, strong isolation, provider flexibility, tool execution, persistent memory, multi-session/multi-agent/multi-user concurrency." MIT, ~55★, v0.3.1.

**What we adopt**: `#[tool]` macro pattern (auto schema generation), safety-tier model for tool permissions, multi-user concurrency design, LLM provider abstraction trait. Do NOT depend; learn and re-implement.
