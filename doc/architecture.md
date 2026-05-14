# Architecture

## Module layout

Single-crate, flat module structure for now. Split into a Cargo workspace when
compile times or module discipline justify it (expected around M6/M7).

```
src/
├── lib.rs            re-exports the public module tree
├── main.rs           CLI entry point — single-turn or REPL mode
├── error.rs          Error enum, Result<T> alias
├── message.rs        Role, ContentBlock, Message, Conversation
├── stream.rs         StreamEvent, StopReason
├── provider.rs       LlmProvider trait, CompletionRequest, EventStream
├── repl.rs           Repl struct — multi-turn loop, slash commands, save/load
└── providers/
    └── openai_compat.rs   OpenAICompatProvider
```

Future modules (added per milestone):

```
src/
├── agent.rs          Agent struct — owns provider, tools, history, context builder
├── config.rs         TOML-based configuration
├── tools/            Tool trait + built-in tools (read_file, run_shell, …)
├── mcp/              MCP client (stdio + HTTP transport)
└── rag/              EmbeddingsProvider, Chunker, VectorStore, Retriever
```

## Core types (`src/message.rs`)

```
Role            System | User | Assistant | Tool
ContentBlock    Text { text } | ToolUse { id, name, input } | ToolResult { … }
Message         role: Role, content: Vec<ContentBlock>
Conversation    messages: Vec<Message>
```

`ContentBlock` is the extensibility point: `Image` blocks for multimodal support
arrive later without changing the rest of the type.

## Streaming (`src/stream.rs`, `src/provider.rs`)

The `LlmProvider` trait is **streaming-first**: every completion returns a
`Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>` (aliased as
`EventStream`).

```
StreamEvent
  TextDelta(String)                   — incremental text token
  ToolUseStart { id, name }           — model wants to call a tool
  ToolUseDelta { id, partial_json }   — partial tool input JSON
  ToolUseEnd { id }                   — tool input complete
  Usage { input_tokens, output_tokens }
  MessageEnd { stop_reason }
```

`StopReason`: `EndTurn | ToolUse | MaxTokens | StopSequence | Other(String)`.

Defining streaming in the trait from day one means tool use (M6) fits the same
loop without a redesign: the agent polls the stream, accumulates tool-use deltas,
executes tools, and sends results back — all in one coherent pattern.

## Provider abstraction (`src/provider.rs`)

```rust
#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn stream(&self, request: CompletionRequest) -> Result<EventStream>;
    fn name(&self) -> &str;
}
```

`CompletionRequest` carries `model`, `messages`, `system`, `temperature`,
`max_tokens`. Tool schemas are added in M6.

One concrete implementation covers most cases:

| Provider | Module | How |
|----------|--------|-----|
| LM Studio (local) | `providers::openai_compat` | `OpenAICompatProvider` pointed at `http://localhost:1234/v1` |
| Apertus via publicai.co | `providers::openai_compat` | same struct, different base URL + API key |
| Native in-process (M10) | `providers::mistral_rs` or `providers::candle` | same trait, no HTTP |

## SSE parsing (`providers/openai_compat.rs`)

The OpenAI-compatible streaming format is newline-delimited SSE:

```
data: {"choices":[{"delta":{"content":"Hello"},...}]}\n\n
data: {"choices":[{"delta":{},"finish_reason":"stop"}]}\n\n
data: [DONE]\n\n
```

`parse_sse()` accumulates `reqwest` byte chunks into a string buffer, splits on
`\n`, strips the `data: ` prefix, JSON-parses each line, and emits `StreamEvent`
values via `async_stream::stream!`.

## Agent loop (M6, planned)

```
loop {
    stream ← provider.stream(request)
    for event in stream:
        TextDelta     → accumulate into assistant message
        ToolUseStart/Delta/End → accumulate tool call
        MessageEnd(EndTurn)   → break
        MessageEnd(ToolUse)   → execute tool, append ToolResult, continue loop
}
```

## Context construction (M5, planned)

A `ContextBuilder` takes `(Conversation, system_prompt, tool_schemas, budget)`
and produces the `Vec<Message>` actually sent. Two strategies:

- **Sliding window** — drop oldest messages when over token limit.
- **Summarise-and-compact** — call the LLM to summarise the evicted portion.

Token counting is provider-specific (a method on `LlmProvider` or a separate
`Tokenizer` trait).
