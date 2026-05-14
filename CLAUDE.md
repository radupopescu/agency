# Agency — Claude Code context

## Project

Rust LLM agent built step-by-step as a learning project, with a planned article
series. Each milestone is anchored by an annotated git tag.

- **Language:** Rust 2024, async (tokio)
- **Local backend:** LM Studio at `http://localhost:1234/v1` (OpenAI-compatible)
- **Remote backend:** Apertus via publicai.co (OpenAI-compatible)
- **Do not** suggest Anthropic or OpenAI as LLM providers

## Key documents

- [`doc/plan.md`](doc/plan.md) — milestone plan with current status
- [`doc/architecture.md`](doc/architecture.md) — module layout, type design, trait contracts
- [`doc/working-practices.md`](doc/working-practices.md) — branching, checklist, commit style

## Current status

- **M0** `m0-skeleton` ✅ — core types, `LlmProvider` trait, CLI
- **M1** `m1-streaming` ✅ — `OpenAICompatProvider`, SSE streaming, live-tested
- **M2** `m2-repl` ✅ — `reedline` REPL, conversation history, `/clear /save /load /quit`
- **M3** `m3-apertus` ⬜ — next

## Conventions (quick reference)

- Branch per milestone → squash-merge to `main` → annotated tag `mN-slug`
- Before each milestone commit: fmt check → clippy `-D warnings` → tests → live test → update docs
- `cargo add` for new deps (latest version)
- No `git push` — user handles remote
- Prefer Sonnet for implementation; avoid subagents unless clearly needed
