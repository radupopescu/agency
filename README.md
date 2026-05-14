# Agency

[![CI](https://github.com/radupopescu/agency/actions/workflows/ci.yml/badge.svg)](https://github.com/radupopescu/agency/actions/workflows/ci.yml)

An LLM agent for learning and experimentation

## Build

```sh
cargo build
cargo run -- --help
```

## Usage

Single-turn prompt against a local LM Studio server:

```sh
cargo run -- --model "google/gemma-4-e4b" "Hello, what are you?"
```

Optional flags:

| Flag | Default | Description |
|------|---------|-------------|
| `-m, --model` | *(required)* | Model name as shown in LM Studio |
| `--base-url` | `http://localhost:1234/v1` | OpenAI-compatible API base URL |
| `--api-key` | — | Bearer token for authenticated endpoints |

Run the M1 standalone example (env-var friendly):

```sh
cargo run --example hello "Hello, what are you?"

# Override defaults
AGENCY_MODEL="google/gemma-4-e4b" \
AGENCY_BASE_URL="http://localhost:1234/v1" \
  cargo run --example hello "Hello"
```

## License

MIT