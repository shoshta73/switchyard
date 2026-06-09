# Developers

This document is for building, testing, and maintaining SwitchYard from source.

For user installation instructions, see [README.md](README.md).

## Requirements

- Rust stable
- A local model provider for manual testing:
  - Ollama
  - llama.cpp-compatible server

SwitchYard is a Rust 2024 workspace. The CLI package is `switchyard` under `apps/cli`.

## Build

Build all workspace members:

```sh
cargo build --workspace
```

Build the CLI in release mode:

```sh
cargo build -p switchyard --release
```

## Run

Run the CLI from source:

```sh
cargo run -p switchyard
```

On first run, SwitchYard creates local state and prompts for an encryption password.

## Test

Run all tests:

```sh
cargo test --workspace
```

Check formatting:

```sh
cargo fmt --check
```

Run clippy with CI-equivalent settings:

```sh
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

## Repo Layout

- `apps/cli`: CLI application and TUI
- `crates/core`: shared model, provider, session, and message types
- `crates/crypto`: encrypted vault storage and crypto helpers
- `scripts`: maintenance and release helper scripts

## Provider Configuration

SwitchYard defaults to Ollama:

```sh
cargo run -p switchyard
```

Use llama.cpp:

```sh
SWITCHYARD_PROVIDER=llama.cpp cargo run -p switchyard
```

Provider-specific environment variables:

```sh
SWITCHYARD_OLLAMA_BASE_URL=http://localhost:11434
SWITCHYARD_OLLAMA_MODEL=llama3.2

SWITCHYARD_LLAMA_CPP_BASE_URL=http://localhost:8080
SWITCHYARD_LLAMA_CPP_MODEL=local-model
```

Fallback environment variables are also supported:

```sh
OLLAMA_BASE_URL
OLLAMA_MODEL
LLAMA_CPP_BASE_URL
LLAMA_CPP_MODEL
```

## Runtime State

SwitchYard stores runtime state under:

```text
$XDG_STATE_HOME/switchyard
```

If `XDG_STATE_HOME` is unset or relative, it falls back to:

```text
$HOME/.local/state/switchyard
```

Important files:

- `salt`: encrypted vault initializer
- `state.json`: persisted CLI state

The vault file format is persisted data. Treat changes to vault serialization as compatibility-sensitive.

## Logging

Logs are written to:

```text
/tmp/switchyard.log
```

In debug builds, `RUST_LOG` can override the default log filter.

## Releases

Development releases are tag-triggered. Tags must match:

```text
YYwWW[a-z]*
```

Use the helper script to compute or create the next development tag:

```sh
scripts/next-dev-tag.py print
scripts/next-dev-tag.py create
scripts/next-dev-tag.py push
```

The release workflow builds the CLI in release mode and uploads:

```text
switchyard-linux-x86_64.tar.gz
```
