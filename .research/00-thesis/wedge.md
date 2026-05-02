# The unique wedge

What hivecore has that no other tool combines.

## Single-line wedge

**Native board+daemon + UI-first surfaces + KG-at-core + custom personas + custom workflow DAG + first-class control plane + multi-tenant isolation + OSS — all in one complete platform.**

## Decomposed

### 1. KG-at-core (not bolted on)
Orchestrator state, gate inputs, skill ranking, role context, audit trail — all are KG queries. Generic platforms add KG as a retrieval cache. Hivecore makes the KG load-bearing.

**Existing pure-KG**: PlayerZero, Bito, Tabnine ECE, Augment, Qodo. **None** offer customizable persona + workflow + governance.

### 2. Custom personas (config, not code)
Orgs declare any role with `.hivecore/personas/<name>.toml`: name, tools, gates, context loader, model preference, budget. No source changes needed.

**Existing role-based platforms**: BMad (fixed 21 agents), AgentsRoom, CrewSwarm. **None** allow defining roles with full harness contract via config.

### 3. Custom workflow DAG (config, not code)
State machine is a `.dag.toml` file. Transitions, guards, gates configurable per workflow. Org A runs `intent → spec → review → dev → QA → deploy`. Org B runs `incident → triage → fix → validate → ship`. Same kernel.

**Existing spec-driven**: Spec-Kit, Kiro, BMad. **All** hardcode the workflow.

### 4. First-class control plane
RBAC, policies, budgets, kill switches, escalation rules, audit retention — all configurable. Not afterthought.

**Existing platforms with strong governance**: Harness AI, Zably AgenticOS, monaOS — but their workflows and personas are vendor-fixed.

### 5. Multi-tenant Knowledge Core isolation
Per-tenant KG instance (TrustGraph pattern). Permission-aware retrieval at query layer. Cross-tenant queries forbidden at API. Audit log per tenant, hash-chained.

**Existing multi-tenant**: SaaS platforms like Codegen, Augment Cloud — but you don't control them. Self-host options usually mean single-tenant.

### 6. OSS (Apache + MIT dual-licensed)
Self-hostable. Contribution-friendly. No vendor lock. No usage caps.

**Existing OSS in adjacent space**: multica, paperclip, OpenHands, Goose, ai-sdlc-framework. **None** combine all five wedges above.

## Failure modes for the wedge

This wedge fails if:

1. A foundation model becomes good enough that the harness doesn't matter (currently empirically false per Claude Code paper 98.4% finding).
2. A vendor (Anthropic, OpenAI, Microsoft) ships a comparable platform that matches all five wedges as managed cloud (possible long-term; OSS adoption-led moat is the defense).
3. The OSS community fails to converge on a common KG ontology, leaving every org's hivecore incompatible with every other (mitigated by shipping a strong reference ontology in P0–P5).
4. Build-vs-buy decisions favor heavy SaaS over self-hosted OSS at the enterprise tier (mitigated by hivecore's potential commercial cloud offering post-v1.0).

## The two-year thesis

Within 24 months of v1.0:

- 100+ organizations run hivecore as their SDLC harness.
- KG ontology becomes the de-facto open standard for SDLC knowledge graphs.
- "Validation replaces review" becomes the dominant pattern, with hivecore as the OSS reference.
- Persona/workflow libraries get exchanged across orgs (with isolation preserved).
