---
url: (handwritten note from PMG head, 2026-04-29; photo on file with author)
status: active-reference
checked: 2026-04-29
tags: [vertical-itsm, motadata-itsm, pmg, requirements, design-partner]
---

**Why it matters**: Canonical V1 acceptance criteria for Phase 7 (PMG Workbench). PMG head is the design partner per ADR-015. The note maps ~80% to existing v1 hivecore plan; remaining gaps are `WEB-14` (diagrams), `WEB-15` (video walkthrough), `ADAPT-V2-05` (voice multi-language, deferred to v2).

**Anchor (decoded list)**:
```
PMG Workbench
→ Markdown
→ Git + Worktree + Publish
→ Connect, Collaboration, History (Roadshow)
→ Knowledge Base, KG
→ Diagrams (+ SVG)
→ Markdown library for easy navigation
→ Web-first (local later)
→ Task list, usage threading, rainbow/kanban (Releases tracking)
   - Usability: Video (architecture) — Mastered for PMG
Advanced:
→ Raw → Record → KG (Mastered for PMG)
→ Voice support (multi-language)
→ Connect (for update) — chat-like or Git-style PR comments
Right column: cleansing / audit ops / coordinated / Audit AH PR
```

**What we adopt**: Treat as Phase 7 acceptance criteria. Map each item to existing hivecore feature (markdown, git+worktree → kernel; KG → P2; web-first → ADR-014 UI-first; kanban → P4 native board; audit → P6 dashboards; collaboration → run viewer + multi-user RBAC; diagrams → new WEB-14; video → new WEB-15; voice → v2). PMG head reviews weekly starting Phase 4 demo.

**What to avoid**: Drifting to PMG-only platform (kernel must stay SDLC-general; ITSM-specific bits live in `.hivecore/ontology/itsm.toml` overlay, not in core).
