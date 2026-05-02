# Master sources index

Deduplicated URL index. Tag-classified. One row per canonical source. Per-topic pointer files reference back here.

## Internal AI software factories

| Tag | URL | Why |
|---|---|---|
| internal-factory, harness | https://openai.com/index/harness-engineering/ | OpenAI Harness — 1M LOC zero-code, 5 months, 1500 PRs, agent-to-agent review |
| internal-factory, harness, paper | https://arxiv.org/abs/2604.14228v1 | Dive into Claude Code: 98.4% infra / 1.6% AI logic; the harness thesis empirical proof |
| internal-factory, harness | https://www.engineering.fyi/article/harness-engineering-leveraging-codex-in-an-agent-first-world | OpenAI Harness blog mirror with summary |
| internal-factory | https://stripe.dev/blog/minions-stripes-one-shot-end-to-end-coding-agents | Stripe Minions — 1300 PRs/wk, MCP toolshed (400 tools), pre-warmed devboxes |
| internal-factory | https://medium.com/%40oracle_43885/how-stripe-built-secure-unattended-ai-agents-merging-1-000-pull-requests-weekly-1ff42f3fe550 | Stripe Minions deep dive: deterministic prefetch of 15 surgical tools |
| internal-factory | https://engineering.atspotify.com/2025/11/spotifys-background-coding-agent-part-1 | Spotify Honk Part 1 — 1500 PRs, fleet management evolution |
| internal-factory, kg-context | https://engineering.atspotify.com/2025/11/context-engineering-background-coding-agents-part-2 | Spotify Honk Part 2 — context engineering, limited tools intentional |
| internal-factory, validation | https://engineering.atspotify.com/2025/12/feedback-loops-background-coding-agents-part-3 | Spotify Honk Part 3 — feedback loops + verify tool |
| internal-factory | https://www.infoq.com/news/2026/01/ramp-coding-agent-platform/ | Ramp Inspect — 30% of merged PRs |
| internal-factory | https://strongdm.com/blog/the-strongdm-software-factory-building-software-with-ai | StrongDM Software Factory — validation replaces review, digital-twin universe |
| internal-factory | https://blog.cloudflare.com/internal-ai-engineering-stack/ | Cloudflare internal AI engineering stack |
| internal-factory | https://devops.com/open-swe-captures-the-architecture-that-stripe-coinbase-and-ramp-built-independently-for-internal-coding-agents/ | DevOps.com — Stripe/Coinbase/Ramp converged on same arch independently |

## Knowledge graphs and context engines

| Tag | URL | Why |
|---|---|---|
| kg, world-model, simulation | https://playerzero.ai/resources/context-graphs-building-production-world-models-for-the-age-of-ai-agents | PlayerZero — context graphs as production world models |
| kg, world-model, simulation | https://playerzero.ai/research/sim-1 | PlayerZero Sim-1: ensemble model, 30+min coherence, 2770-scenario benchmark |
| kg, world-model | https://playerzero.ai/resources/production-world-model-ai-software-defect-prediction | PlayerZero — engineering world model concept |
| kg, validation | https://www.playerzero.ai/platform/code-simulations | PlayerZero — simulations as PR merge gate |
| kg-temporal, paper | https://arxiv.org/abs/2501.13956 | Zep paper — temporal KG architecture for agent memory, beats MemGPT |
| kg-temporal | https://github.com/getzep/graphiti | Graphiti — Apache 2.0 OSS bi-temporal KG engine |
| kg-temporal | https://docs.falkordb.com/agentic-memory/graphiti.html | Graphiti on FalkorDB |
| kg-multi-tenant | https://trustgraph.ai/guides/key-concepts/knowledge-cores-modular-memory/ | TrustGraph Knowledge Cores — multi-tenant KG isolation pattern |
| kg-schema | https://handbook.gitlab.com/handbook/engineering/architecture/design-documents/gitlab_knowledge_graph/data_model/ | GitLab KG data model — property graph, dual SDLC+Code subgraph |
| kg-schema | https://docs.gitlab.com/user/project/repository/knowledge_graph/ | GitLab gkg user docs, MCP server |
| kg, paper | https://arxiv.org/html/2503.07967v4 | Code Digital Twin — formal SDLC KG model with artifact-knowledge reflection |
| kg, paper | https://arxiv.org/html/2602.20478v1 | Codified Context — 3-tier (hot/specialist/cold) KG infrastructure |
| kg-context | https://www.qodo.ai/blog/introducing-qodo-aware-deep-codebase-intelligence-for-enterprise-development/ | Qodo Aware — context engine |
| kg-context | https://bito.ai/platform/ | Bito — KG of code+commits+issues+docs+slack |
| kg-context | https://docs.bito.ai/ai-architect/knowledge-graph | Bito KG docs |
| kg-context | https://www.tabnine.com/enterprise-context-engine | Tabnine Enterprise Context Engine |
| kg-context | https://augmentcode.com/context-engine | Augment Code Context Engine |
| kg, oss | https://github.com/odpf/compass | raystack/compass — OSS temporal KG context engine |
| kg, oss | https://github.com/DanielBlomma/cortex/ | Cortex — local OSS code KG, tree-sitter, ADR enforcement |
| kg, oss | https://github.com/SerPeter/code-atlas | Code Atlas — Memgraph + AST + MCP |
| kg, sdlc | https://nathanlasnoski.com/2026/03/01/building-an-enterprise-knowledge-graph-for-the-sdlc-is-the-foundation-of-agentic-first-development-and-a-move-from-vibe-to-scale/ | Lasnoski — Enterprise KG for SDLC thesis |

## Spec-driven development

| Tag | URL | Why |
|---|---|---|
| spec-driven | https://kiro.dev/blog/from-chat-to-specs-deep-dive | Kiro — agentic IDE with SDD |
| spec-driven | https://github.com/github/spec-kit | GitHub Spec-Kit — toolkit for SDD across 15+ assistants |
| spec-driven | https://specs.md/compare/overview | SDD tool comparison |
| spec-driven | https://openspec.pro/comparison/ | OpenSpec vs Spec-Kit vs BMAD vs Kiro |
| spec-driven | https://sloth255.com/en/blog/comparing-spec-driven-development-tools-speckit-vs-kiro | SpecKit vs Kiro |
| spec-as-source, oss | https://github.com/bssm-oss/PlainCode | PlainCode — spec-first build orchestrator, file ownership, build receipts |
| spec-driven, gsd | https://github.com/gsd-build/get-shit-done | GSD v1 — current dev harness |
| spec-driven, gsd | https://github.com/gsd-build/gsd-2 | GSD v2 — Pi SDK based, state machine |

## Harness engineering

| Tag | URL | Why |
|---|---|---|
| harness, 5-layer | https://harness-engineering.ai/blog/agent-harness-architecture-how-the-system-works-under-the-hood/ | 5-layer harness architecture canonical breakdown |
| harness, paper | https://www.alphaxiv.org/abs/2603.05344 | Building AI Coding Agents for Terminal — scaffolding/harness/context |
| harness, repo-as-harness | https://nathan-delacretaz.com/thinks/compound-agent-harness | Compound-Agent — repo-as-harness OSS npm |
| harness | https://www.verdent.ai/guides/harness-engineering-ai-coding-workflow | Verdent — harness in practice |
| harness | https://booboone.com/harness-engineering-for-coding-agent-users/ | Booboone — guides + sensors framing |
| harness | https://addyosmani.com/blog/agent-harness-engineering/ | Addy Osmani — agent harness primer |
| harness | https://vitthalmirji.com/2026/02/build-the-harness-not-the-code-a-staff/principal-engineers-guide-to-ai-agent-systems/ | Mirji — staff/principal guide to harness systems |
| harness, oss-sdlc | https://github.com/ai-sdlc-framework/ai-sdlc | AI-SDLC Framework — declarative governance for AI SDLC |

## Verification, replay, code review

| Tag | URL | Why |
|---|---|---|
| validation, replay | https://debugg.ai/resources/deterministic-replay-pipeline-code-debugging-ai | DebuggAI — production-trace → verified-fix replay pipeline |
| validation, replay | https://www.devcover.ai/ | DevCover — shadow traffic replay against PR diffs |
| validation, ai-review | https://www.greptile.com/feature/code-context | Greptile — graph-based PR review, blast radius |
| validation, ai-review | https://www.greptile.com/agent | Greptile Agent — swarm of reviewers |
| validation | https://debugger.ai/ | Debugger.ai — proactive scan + auto-fix PRs |
| validation, gates | https://mirin.pro/blog/agentic-workflows-part4-verification | Mirin — verification patterns part 4 |
| gnn, vuln | https://arxiv.org/pdf/2310.20067 | Vignat — CPG + GAT vulnerability detection |
| gnn, vuln | https://arxiv.org/html/2603.29216v1 | VulGNN — lightweight GNN, CI-deployable |

## Self-improvement loops

| Tag | URL | Why |
|---|---|---|
| self-improve | https://dev.to/askpatrick/how-to-build-a-self-improving-ai-agent-the-nightly-loop-pattern-njn | Patrick — nightly one-fix loop |
| self-improve | https://github.com/unconst/ninja | Ninja — AI-grown coding agent (Arbos outer loop) |
| self-improve | https://github.com/Recusive/Nightshift | Nightshift Recursive — Owl + Raven loops |
| self-improve | https://agentpatterns.ai/agent-design/agent-self-review-loop/ | AgentPatterns — self-review loop |
| self-improve | https://github.com/sortie-ai/sortie/issues/312 | Sortie orchestrator-controlled self-review |
| self-improve | https://github.com/theprint/nfh-self-improvement-loop | Generator-evaluator hard isolation |

## Coordination + concurrency

| Tag | URL | Why |
|---|---|---|
| coordination | https://pubroot.com/ai/agent-architecture/file-ownership-and-message-passing-a-practical-coordination-protocol-for-2026-024/ | File ownership + message passing — 20-agent zero-conflict protocol |

## Durable execution

| Tag | URL | Why |
|---|---|---|
| durable-execution | https://zylos.ai/research/2026-02-17-durable-execution-ai-agents | Zylos — durable execution patterns; Temporal $5B/9.1T executions |

## Skill registries

| Tag | URL | Why |
|---|---|---|
| skill-registry | https://skillreg.dev/ | SkillReg — private SKILL.md registry, semver, SSO |
| skill-registry | https://openbooklet.com/ | OpenBooklet — universal skills/workflows registry, SHA-256 locked |
| skill-registry | https://agentplaybooks.ai/ | AgentPlaybooks — platform-independent skill vault |
| skill-registry | https://alacritous.io/skills | Alacritous — playbooks-as-code |
| skill-registry | https://aithub.space/ | SkillHub — public agent-published skills with provenance |
| skill, codemod | https://codemod.com/blog/npx-codemod-ai | Codemod — AI codemod platform with registry |

## Reference platforms (OSS)

| Tag | URL | Why |
|---|---|---|
| agent-coding, oss | https://github.com/vercel-labs/open-agents | Vercel Open Agents — reference cloud agent platform |
| agent-coding, oss | https://vercel.com/templates/template/open-agents | Open Agents Vercel template |
| agent-coding, oss | https://tekai.dev/catalog/openhands | OpenHands (formerly OpenDevin) — All Hands AI |
| agent-coding, oss | https://agentsindex.ai/compare/goose-block-vs-openhands | Goose vs OpenHands |
| agent-coding, harness | https://mendral.com/blog/how-we-built-our-ai-agent | Mendral — Go agent + Firecracker microVM |

## Tier-1 board+daemon platforms

| Tag | URL | Why |
|---|---|---|
| board-daemon, oss | https://github.com/multica-ai/multica | Multica — multi-runtime board+daemon |
| board-daemon, oss | https://github.com/paperclipai/paperclip | Paperclip — solo agent company simulator |
| board-daemon, oss | https://github.com/kdlbs/kandev | Kandev — kanban+pipeline multi-agent |
| board-daemon, oss | https://github.com/bonaysoft/agent-kanban | Agent Kanban — agent-first board, leader-worker |
| board-daemon, oss | https://github.com/goranefbl/optimushq | OptimusHQ — multi-tenant CC board |
| board-daemon, oss | https://github.com/GreenSheep01201/Claw-Kanban | Claw-Kanban — role-based auto-assign |
| board-daemon, oss | https://github.com/RunMaestro/Maestro | Maestro — desktop multi-agent (2.7k★) |
| board-daemon, oss | https://github.com/titalk/AgentsMesh | AgentsMesh — Go runner + gRPC/mTLS + Relay |

## Tier-2 daemon-only

| Tag | URL | Why |
|---|---|---|
| daemon-issue-pr, oss | https://github.com/ianchenx/reeve | Reeve — Bun/TS, Linear → PRs |
| daemon-issue-pr, oss | https://github.com/ceedaragents/cyrus | Cyrus — Linear/Slack/GH/GL → PRs |
| daemon-issue-pr, oss | https://github.com/nicobistolfi/vigilante | Vigilante — Go, gh-mirror proxy ACL |
| daemon-issue-pr, oss | https://github.com/FratteFlorian/kodama | Kodama — Go+SQLite, telegram, rate-limit failover |
| daemon-issue-pr, oss | https://github.com/proboscis/orch | orch — issue/run/event vocab |
| daemon-issue-pr, oss | https://github.com/flotilla-org/flotilla | Flotilla — multi-host SSH peer, work-item correlation |
| harness-substrate, oss | https://github.com/supermemoryai/smfs | smfs — Supermemory FS mount; mine `agent_hint.rs` block injection pattern + sync coalescing |
| durable-execution, oss, rust | https://github.com/microsoft/duroxide | duroxide — Microsoft's Rust durable execution runtime; hivecore-orchestrator wraps this |
| harness-substrate, oss, rust | https://github.com/shantanugoel/oxydra | oxydra — Rust agent orchestrator; reference impl for `#[tool]` macro + safety tiers |

## Tier-3 harness substrates

| Tag | URL | Why |
|---|---|---|
| harness-substrate, oss | https://github.com/badlogic/pi-mono/ | Pi monorepo — minimal harness + SDK + RPC |
| harness-substrate, oss | https://buildwithpi.ai/ | Pi.dev — terminal harness |
| harness-substrate, oss | https://github.com/majiayu000/harness | majiayu/harness — Rust orchestration platform |
| harness-substrate, oss | https://github.com/akougkas/pancode | PanCode — multi-agent runtime, Pi/SDK/CLI tier |
| harness-substrate, oss | https://github.com/tmdgusya/roach-pi | Roach-Pi — Pi extension with strict discipline |

## Tier-5 multi-agent orchestrators

| Tag | URL | Why |
|---|---|---|
| orchestrator, oss | https://github.com/PrimeLocus/Hydra | Hydra — 5 modes (auto/council/dispatch/smart/chat) |
| orchestrator, oss | https://github.com/B-A-M-N/maestro-orchestrate | Maestro-orchestrate — 22 specialists |
| orchestrator, oss | https://github.com/phil65/agentpool/ | AgentPool — YAML config bridge (ACP/AG-UI) |
| orchestrator, oss | https://mcp.aibase.com/server/1639702816237035567 | Autonomous Lab — PI/Trainee/Reviewer roles via MCP |
| orchestrator, oss | https://github.com/jcanizalez/vibegrid | Vorn/Vibegrid — desktop alpha, PTY native |

## Org memory + tribal knowledge

| Tag | URL | Why |
|---|---|---|
| org-memory | https://cortexdb.ai/docs/blog/engineering-intelligence | CortexDB — engineering memory (Slack+PagerDuty+Jira+GitHub) |
| org-memory, oss | https://jeremyrajan.com/blog/ai-engineering-context-layer/ | Onyx (Danswer) — OSS knowledge index for Confluence/Jira/Slack/GH |
| org-memory | https://daily.jovis.ai/engineering-manager/build-organizational-memory-not-institutional-knowledge/ | Build organizational memory not institutional knowledge |
| org-memory | https://get.mem.ai/guides/architecture-decision-records-ai-notes | Mem.ai — ADR + AI notes |
| org-memory | https://remlabs.ai/blog/rem-labs-slack-channel-memory | REM Labs — Slack channel memory |
| org-memory | https://zylos.ai/research/2026-03-23-organizational-knowledge-management-ai-agent-teams | Zylos — Enterprise Memory Layer architecture |

## SDLC platforms (commercial / direct competitors)

| Tag | URL | Why |
|---|---|---|
| sdlc-platform, oss | https://github.com/ai-sdlc-framework/ai-sdlc | AI-SDLC Framework — direct OSS competitor in same segment |
| sdlc-platform | https://www.kagen.ai/product/intelligent-sdlc-platform | Kagen — intelligent SDLC, Figma→Jira |
| sdlc-platform | https://agentsroom.dev/features/teams | AgentsRoom Teams — n8n-style role canvas |
| sdlc-platform | https://crewswarm.ai/index.html | crewswarm — parallel waves, 20+ specialists |
| sdlc-platform | https://monaos.ai/ | monaOS — 4-plane arch, schema-validated msgs |
| sdlc-platform | https://zably.ai/solutions/agentic-os | Zably AgenticOS — control/data/security/devex planes |
| sdlc-platform | https://agentman.ai/platform | Agentman — Build→Test→Optimize→Monitor |
| ai-review | https://www.codegen.com/blog/how-to-build-agentic-coding-workflows/ | Codegen — agentic workflow platform |
| ai-review | https://developer.harness.io/docs/platform/harness-ai/devops-agent/ | Harness AI DevOps Agent |
| ai-review | https://www.augmentcode.com/guides/how-do-enterprise-teams-build-agentic-workflows | Augment — Coordinator/Specialist/Verifier |

## Cody / Sourcegraph

| Tag | URL | Why |
|---|---|---|
| code-search | https://sourcegraph.com/blog/cody-is-enterprise-ready | Cody Enterprise — 300k repos scale |
| code-search | https://sourcegraph.com/blog/how-cody-provides-remote-repository-context | Cody multi-repo context |
| code-search | https://sourcegraph.com/cody/pricing | Sourcegraph "Goodbye Cody, try Amp" |

## ITSM AI (different segment, watch for runtime collisions)

| Tag | URL | Why |
|---|---|---|
| vertical-itsm | https://help.ivanti.com/ht/help/en_US/ISM/2026/admin-user/Content/AgenticAI/Agentic%20AI.htm | Ivanti Neurons ITSM 2026.1 Agentic AI |
| vertical-itsm | https://www.ivanti.com/company/press-releases/2026/ivanti-unveils-ai-driven-innovations-to-the-neurons-platform-to-power-the-future-of-it-and-security | Ivanti AI announcement Jan 2026 — preview Q1, GA later 2026 |
| vertical-itsm | https://aisera.com/products/next-gen-itsm | Aisera ITSM agentic platform |
| vertical-itsm | https://docs.aisera.com/aisera-platform/llm-operations/understanding-llm-capabilities/aiseras-agentic-ai-for-itsm | Aisera platform docs |
| vertical-itsm | https://www.wolkensoftware.com/features/knowledge-base | Wolken AI service manager |
| vertical-itsm | https://www.quinnox.com/qinfinite/operate/itsm | Quinnox Qinfinite — ITSM with KG over CMDB |
| vertical-itsm | https://hub.teamdynamix.com/products/itsm/ai-service-assist/ | TeamDynamix AI Service Assist |
| vertical-itsm | https://scogo.ai/platform/ai-service-desk | Scogo AI service desk |
| vertical-itsm | https://www.mavenagi.com/integrations/servicenow | Maven AGI ServiceNow integration |

## Benchmarks

| Tag | URL | Why |
|---|---|---|
| benchmark, paper | https://www.semanticscholar.org/paper/SWE-Bench-Pro%3A-Can-AI-Agents-Solve-Long-Horizon-Deng-Da/4b83aa6340be8e0e59309d37ac4a3c9ae1ece14e | SWE-Bench Pro — long-horizon SWE benchmark |
| benchmark | https://terminaltrove.com/compare/ai-coding-agents/goose-cli-vs-openhands-cli/ | Terminal-Bench 2.0 comparison |

## World models, physical AI

| Tag | URL | Why |
|---|---|---|
| world-model | https://blogs.nvidia.com/blog/gtc-2026-virtual-worlds-physical-ai | NVIDIA GTC 2026 — Cosmos 3, Isaac GR00T, world models |

## Build vs buy / org maturity

| Tag | URL | Why |
|---|---|---|
| ops, build-vs-buy | https://www.devx.com/web-development-zone/when-to-build-vs-buy-an-internal-developer-platform/ | DevX — when to build vs buy IDP, Spotify/Stripe/Airbnb consensus |

## Memory / KG OSS

| Tag | URL | Why |
|---|---|---|
| kg-temporal, oss | https://github.com/predictable-labs/ryumem | Ryumem — bi-temporal KG memory, multi-tenant |
| kg-temporal, oss | https://github.com/aexy-io/graphzep | GraphZep — TS implementation of Zep paper |
| kg-temporal, oss | https://github.com/G-SaiVishwas/agentic_memory_layer | Membread — bi-temporal + 47 connectors |
| kg-temporal, oss | https://openmemory.cavira.app/docs/introduction | OpenMemory — HMD v2, 5-sector embeddings |
| kg-temporal, oss | https://github.com/tleers/graphiti | Graphiti fork (tleers) |

---

**Total: ~85 sources indexed.** Per-section files reference these URLs; never copy content.
