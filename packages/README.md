# packages/

Shared TypeScript packages for the frontend monorepo.

## Conventions

- `@hivecore/<name>` package naming.
- Each package has its own `package.json` and `tsconfig.json`.
- Public API in `src/index.ts`; internal modules under `src/`.
- No `react-dom` imports in non-UI packages (state-management discipline).

## Phases that touch this dir

- F0 — `@hivecore/types` (shared types from Rust → TS via OpenAPI)
- F1 — `@hivecore/api-client` (typed API client)
- F1 — `@hivecore/ui` (shared shadcn-based components)
- F2 — `@hivecore/realtime` (WS subscription helpers)
