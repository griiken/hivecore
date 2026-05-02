# apps/web

Hivecore frontend. Next.js 16 (App Router) + TypeScript.

Phase F0 will scaffold the actual app (`pnpm create next-app@latest .` with App Router + TS strict + Tailwind). This README is a placeholder.

See [docs/architecture.md](../../docs/architecture.md) for the full frontend architecture.

## Stack

- Next.js 16 (App Router)
- TypeScript strict
- React Query (server state)
- Zustand (client state)
- shadcn/ui components
- tRPC layer to Rust backend
- WebSocket for live run streaming

## Phases that touch this app

- F0 — initial scaffold + auth
- F1 — workspace board (kanban, run viewer)
- F2 — live run streaming (WS)
- F3 — KG visualizer
- F4 — skill registry UI
