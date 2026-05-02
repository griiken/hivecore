# crates/hivecore-openai-adapter/AGENTS.md

**Layer 2 — OpenAI Chat Completions adapter.** Streams responses through the `hivecore_runtime_core::ModelAdapter` trait.

## What lives here

| Module | Purpose |
|---|---|
| `wire` | DTOs for `/v1/chat/completions` request + streaming chunk shape. |
| `client` | `OpenAiClient` + `OpenAiConfig` (HTTP client, base URL, headers). |
| `completion` | `stream_chat` — owns the SSE → `ModelChunk` pipeline. |
| `sse` | Hand-rolled line decoder (no extra dep tree, handles split chunks + CRLF + `[DONE]`). |
| `convert` | `ModelRequest` → `ChatRequest` + `StreamAggregator` (defrags split tool-call frames). |
| `adapter` | `OpenAiAdapter` impl of `ModelAdapter`. |
| `error` | `OpenAiError` (`thiserror`). |

## OpenAI-compat backends

Works against any backend speaking `/v1/chat/completions`:
- OpenAI proper
- Groq, OpenRouter, vLLM, Ollama, …

Override the base URL via `OpenAiConfig::with_base_url(...)`.

## Stream invariants

- Tool calls split across multiple delta frames are reassembled in `StreamAggregator` (Zed pattern, validated against the v1 schema).
- `usage` arrives in a separate frame *after* `finish_reason` when `stream_options.include_usage = true`. The aggregator defers `MessageEnd` until usage is in or `[DONE]` arrives.
- `reasoning_content` deltas (gpt-5 / o-series) map to `ContentBlock::Thinking`.

## Test commands

```
cargo test -p hivecore-openai-adapter
OPENAI_API_KEY=sk-... cargo run --example smoke -- gpt-5.4-nano "say hi"
```

## v0.2 backlog

- Anthropic adapter (parallel `*-anthropic` crate, same `ModelAdapter` trait).
- Multi-provider `ModelRouter` so `Agent.model.provider` actually drives provider selection.
- Live integration tests gated by `OPENAI_API_KEY`.
