# Implementation Plan

Each milestone introduces one concept, ships something runnable, and is anchored
by an annotated git tag (`m0-skeleton`, `m1-streaming`, …).

## Status

| Tag | Milestone | Status |
|-----|-----------|--------|
| `m0-skeleton` | Skeleton — core types, `LlmProvider` trait, CLI scaffold | ✅ done |
| `m1-streaming` | `OpenAICompatProvider` + SSE streaming against LM Studio | ✅ done |
| `m2-repl` | Multi-turn REPL (`reedline`), in-memory history, `/clear /save /load` | ✅ done |
| `m3-config` | TOML config file, multiple provider/model presets | ✅ done |
| `m4-multimodal` | Image/file `ContentBlock` variants, provider serialisation, REPL `/attach` | ✅ done |
| `m5-context` | `ContextBuilder`: sliding-window + summarisation strategies | ⬜ pending |
| `m6-tools` | `Tool` trait, built-in tools, agent loop, approval policy | ⬜ pending |
| `m7-persistence` | SQLite persistence via `sqlx`, `/resume <id>` | ⬜ pending |
| `m8-mcp` | MCP client (stdio transport first, then HTTP/SSE) | ⬜ pending |
| `m9-rag` | RAG from scratch: embeddings, chunker, `sqlite-vec`, retriever | ⬜ pending |
| `m10-native` | Native in-process inference (mistral.rs / candle on Metal) | ⬜ pending |
| `m11-apertus` | Apertus via publicai.co (config entry in `OpenAICompatProvider`) | ⬜ pending |

## Milestone details

### M0 — Skeleton ✅
- Crate scaffold, Rust 2024 edition
- `Role`, `ContentBlock`, `Message`, `Conversation`
- `StreamEvent`, `StopReason` (streaming-first design, tool events included)
- `LlmProvider` trait + `CompletionRequest` + `EventStream` type alias
- `Error`/`Result` via `thiserror`
- `clap` CLI, `tracing` logging
- GitHub Actions CI: `fmt --check`, `clippy -D warnings`, `test`

### M1 — Streaming ✅
- `OpenAICompatProvider`: POST `/chat/completions` with `stream: true`
- Manual SSE/NDJSON line parser via `async_stream::stream!`
- Normalises `TextDelta`, `MessageEnd`, `Usage` into `StreamEvent`
- `reqwest 0.13` (rustls), `tokio 1.52`, `async-stream 0.3`
- `cargo run -- -m <model> <prompt>` streams to stdout
- `examples/hello.rs`

### M2 — REPL
- `reedline`-based REPL loop
- Maintain `Conversation` in memory; send full history each turn
- Slash commands: `/clear`, `/quit`, `/save <file>`, `/load <file>`
- Streaming output in the REPL (same `StreamEvent` loop)

### M3 — Config ✅
- `~/.config/agency/config.toml` (or `--config` override)
- Sections: `[providers.<name>]` with `base_url`, `api_key`, `default_model`
- `[defaults] provider` selects the active provider when `--provider` is not given
- CLI flags override config values; model is required (CLI or config)
- `src/config.rs`: `Config`, `ProviderConfig`, `Defaults` — load + merge logic
- `examples/config.rs`

### M4 — Multimodal ✅
- `ImageData` enum: `Url(String)` or `Base64(String)` (inline data)
- `ContentBlock::Image { media_type, data }` and `ContentBlock::File { name, media_type, data }`
- `ContentBlock::Audio { format, data }` — OpenAI-compat `input_audio`
  accepts only base64 bytes with `format ∈ { "wav", "mp3" }`, so audio carries
  its data inline as a `String` rather than reusing `ImageData`
- `OpenAICompatProvider` serialises image/file/audio blocks as OpenAI content
  parts (`image_url` with `data:<media_type>;base64,<bytes>` URL, `file` part,
  or `input_audio` part)
- Plain-text messages still serialise as a bare string for compatibility
- REPL `/attach <path>` command: reads the file, base64-encodes it, infers
  type from extension (images → `Image`, `.wav`/`.mp3` → `Audio`, else `File`),
  queues the block for the next message
- `examples/vision.rs`: send a local image and stream the description
- New dep: `base64 = "0.22"`

Note: streaming deserialisation of model-generated image blocks is not
implemented — OpenAI-compat SSE doesn't define an image-delta event. Image
output support arrives with the providers that need it.

**Server compatibility caveat.** LM Studio's OpenAI-compat gateway currently
restricts `content` parts to `text` and `image_url`. Both `input_audio` and
`file` parts return `HTTP 400` ("`content` objects must have a `type` field
that is either `text` or `image_url`"), regardless of the loaded model. The
serialisation here matches OpenAI's published content-parts schema, so it
works against providers that implement it (the real OpenAI API, vLLM with
audio-capable models, etc.) — but expect M4 audio/file attachments to fail
against LM Studio until that gateway catches up. Image attachments work as
long as the loaded model is vision-capable.

### M5 — Context construction
- `ContextBuilder` type: takes conversation + system + tool schemas + budget
- Token counting (provider-specific, estimate for non-OpenAI)
- Strategy 1: sliding window (drop oldest messages)
- Strategy 2: summarise-and-compact (LLM call to compress evicted history)

### M6 — Tool use
- `Tool` trait: `name()`, `description()`, schema (`schemars`), `async fn execute()`
- Built-in tools: `read_file`, `list_dir`, `run_shell` (with approval gate)
- `ToolPolicy`: always-allow / ask-once / always-deny
- Wire tool-call deltas through `StreamEvent` for each provider
- Agent loop: stream → accumulate tool calls → execute → append `ToolResult` → continue

### M7 — Persistence
- `sqlx` + SQLite, migrations
- Tables: `conversations`, `messages`, `tool_calls`
- Write on each `MessageEnd`; `/resume <id>` in the REPL
- In-memory `Conversation` remains the working representation

### M8 — MCP client
- Implement MCP protocol (JSON-RPC over stdio first, then HTTP/SSE)
- Discover server's tools/resources/prompts
- Wrap each MCP tool as a `Tool` impl — slots into M6's agent loop unchanged
- Test against an existing server (e.g. `mcp-server-filesystem`, `mcp-server-fetch`)

### M9 — RAG
- `EmbeddingsProvider` trait (Ollama `/api/embeddings` or OpenAI-compat endpoint)
- Document ingestion: chunker with configurable overlap
- Vector store: `sqlite-vec` (reuses the M7 database)
- `Retriever` trait with two integration modes:
  - Explicit `search_docs` tool the model can call
  - Implicit retrieval injected by `ContextBuilder` before each turn

### M10 — Native local backend
- `mistral.rs` or `candle` provider for in-process Metal inference on Apple Silicon
- Same `LlmProvider` trait — provider swap is one config line
- Validates the trait design holds across the HTTP/in-process boundary

### M11 — Apertus
- Verify publicai.co API shape; likely just a config entry in `OpenAICompatProvider`
- Provider selection via `--provider` flag or config file
