# Philosophy

Twelve principles that shape every decision in hivecore. When in doubt, return here.

## 1. KG at core, not bolted on

The knowledge graph is the spine, not a context-retrieval cache. Orchestrator state, gate inputs, skill ranking, role context — all are KG queries. If a feature can be implemented as a KG query, it is.

## 2. Personas, workflows, and policies are config, not code

Orgs customize hivecore by editing TOML, not forking source. Code changes ship hivecore the platform; config changes ship the org's harness.

## 3. Repo-as-harness

Externalize context into the target repo (`.hivecore/` + `AGENTS.md` TOC + `docs/`), not into prompts. The repo remembers; the prompt forgets.

## 4. Validation replaces review

Scenarios + digital twins + production replay are the merge gate. Human review is the escape hatch, not the default.

## 5. Bi-temporal honesty

Facts have validity windows. The system never lies about what it knew when. An agent on a stale branch reads correct-for-its-time facts.

## 6. Multi-tenant isolation by default

Each tenant is a separate Knowledge Core. Cross-tenant queries forbidden at the retrieval layer, not at the prompt layer. Permission-aware retrieval is non-negotiable.

## 7. Durable execution from day one

Every run is journaled. Crashes resume from last good checkpoint. Saga compensation handles partial multi-persona failures. No "fingers crossed it doesn't crash."

## 8. Atomic commits, append-only logs

Every persona action is a single atomic git commit. The KG run subgraph is append-only. Replay is always possible.

## 9. Compute is cheaper than human attention

Bias toward more agent runs, not more human review. If an agent is wrong, that is a fixable harness problem; if a human is bored, that is wasted talent.

## 10. Compounding skills

Every successful run improves a skill. Every failed run improves the harness (gate added, skill updated, constitution rule added). Skills compound; failures don't.

## 11. Boring substrate, opinionated harness

The substrate (storage, RPC, sandbox, durability) uses commodity tech (Postgres, gRPC, Firecracker, OpenTelemetry). The harness (personas, workflows, gates, scenarios) is where opinions live.

## 12. Open source as obligation

Hivecore is OSS so the patterns it embodies become public. Internal "AI software factories" at Stripe, Spotify, Ramp are too valuable to stay locked. Hivecore makes the pattern adoptable by everyone.
