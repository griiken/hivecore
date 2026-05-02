# Comparison

How hivecore differs from adjacent and alternative tools. Brief table; each entry links to `.research/` pointer for the source.

## Direct competitors (same segment — board + daemon + agent runtime)

Hivecore ships board + daemon + agent runtime + KG + harness + control plane natively as one platform. The projects below cover subsets of that surface; hivecore's bet is that an integrated, KG-at-core, UI-first platform with customizable personas/workflows/policies beats fragmented stacks.

| Project | Hivecore comparison |
|---|---|
| [Multica](../.research/01-landscape/multica.md) | Multi-runtime board+daemon. **Strengths**: clean multi-CLI integration, mature daemon. **Hivecore difference**: KG-at-core (multica has none), custom personas + workflow + control plane as first-class config (multica's roles/workflow are largely fixed), bi-temporal multi-tenant Knowledge Cores, validation-replaces-review with scenarios/digital-twin/replay, native KG visualizer + visual workflow DAG editor + visual persona editor in UI. |
| [Paperclip](../.research/01-landscape/paperclip.md) | Solo "agent company simulator" with org-chart governance. **Strengths**: heavy governance UI. **Hivecore difference**: multi-tenant from day one; KG spine; team-scale not solo-scale; validation engine; spec-as-source for plugin authoring. |
| [AI-SDLC Framework](../.research/01-landscape/ai-sdlc-framework.md) | Declarative governance for AI-augmented SDLC. **Strengths**: governance-first DSL. **Hivecore difference**: KG-at-core, native UI surfaces, bi-temporal facts, persona/workflow/policy customization with visual editors. |
| [AgentsRoom Teams / CrewSwarm / Maestro / Hydra](../.research/01-landscape/agent-orchestrators.md) | Multi-agent orchestrators with role definitions. **Strengths**: agent role abstraction. **Hivecore difference**: KG context, durable execution wrapping duroxide, multi-tenant isolation, native validation + replay, OSS substrate (some of these are commercial). |
| [Vercel Open Agents](../.research/01-landscape/open-agents.md) | Reference impl for cloud coding agents. **Strengths**: agent-vs-sandbox separation principle. **Hivecore difference**: full platform vs reference impl; KG, personas, workflows, governance, native UI; not Vercel-locked. |

## Adjacent tools (different layer — runtime adapters or upstream sources)

| Project | Layer | Hivecore relationship |
|---|---|---|
| [OpenHands / Goose / Sweep](../.research/01-landscape/openhands-goose-sweep.md) | Coding agent products | Runtime adapters hivecore plugs in via `RuntimeAdapter` trait |
| [Claude Code / Codex / OpenCode / Pi / Cursor CLI](../.research/01-landscape/coding-agents.md) | Agent CLIs | Runtime adapters hivecore plugs in |
| [LangGraph / CrewAI / AutoGen](../.research/01-landscape/agent-frameworks.md) | Agent frameworks | No SDLC opinion, no KG, no governance — orthogonal; hivecore could use these inside a custom adapter |

## SDLC platforms (direct competitors)

| Project | Position | Hivecore difference |
|---|---|---|
| [AI-SDLC Framework](../.research/01-landscape/ai-sdlc-framework.md) | Declarative governance for AI SDLC | Hivecore adds KG-at-core + persona/workflow customization first-class |
| [StrongDM Software Factory](../.research/02-internal-factories/strongdm-factory.md) | "Validation replaces review" pattern | Operating model hivecore copies; StrongDM is internal/security-specific |
| [BMad Method](../.research/04-spec-driven/bmad.md) | 21 specialized agents, enterprise | Hivecore lets you define your own agents, not pick from a fixed 21 |
| [Spec-Kit / Kiro / OpenSpec](../.research/04-spec-driven/spec-driven-comparison.md) | Spec-driven dev tools | Hivecore embeds spec-driven as one workflow; not the only one |

## Internal AI software factories

| Project | Status | Pattern hivecore adopts |
|---|---|---|
| [Stripe Minions](../.research/02-internal-factories/stripe-minions.md) | Internal, 1.3k PRs/wk | Pre-hydrated MCP tool context, devbox pre-warm |
| [Spotify Honk](../.research/02-internal-factories/spotify-honk.md) | Internal, 1.5k PRs | Limited tools = predictable; verify-tool wraps build/test |
| [OpenAI Harness](../.research/02-internal-factories/openai-harness.md) | Internal, 1M LOC zero-code | AGENTS.md as TOC, agent-to-agent review, gc agents |
| [Ramp Inspect](../.research/02-internal-factories/ramp-inspect.md) | Internal, 30% of PRs | Same dev environment access as humans |

## KG / context engines

| Project | Hivecore relationship |
|---|---|
| [PlayerZero](../.research/03-kg-and-context/playerzero-sim1.md) | Aspirational reference for Sim layer (P19) |
| [Bito / Tabnine / Augment / Qodo](../.research/03-kg-and-context/context-engines.md) | Generic; hivecore adds persona/workflow/policy on top |
| [Graphiti / Zep](../.research/03-kg-and-context/graphiti-zep.md) | Bi-temporal pattern hivecore follows; possible KG impl reference |
| [TrustGraph](../.research/03-kg-and-context/trustgraph-cores.md) | Knowledge Cores pattern hivecore adopts |
| [GitLab Knowledge Graph](../.research/03-kg-and-context/gitlab-gkg.md) | Property graph + dual subgraph pattern hivecore extends |
| [OneLens / OneLens-Palace](../.research/03-kg-and-context/onelens.md) | Author's existing code KG; hivecore can ingest its outputs |

## ITSM AI platforms (different segment)

| Project | Segment | Why not direct competitor |
|---|---|---|
| Ivanti Neurons ITSM Agentic AI | Runtime ticket resolution | Hivecore is build-time customization authoring |
| Aisera | Runtime self-service | Same — different layer |
| Wolken | ITSM service delivery | Same — different layer |

The ITSM AI competitors target end-user ticket flows. Hivecore's first vertical (motadata-itsm) is upstream: AI authors the plugins/queries/customizations that power ITSM platforms.
