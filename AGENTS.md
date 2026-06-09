# AGENTS.md

## Repo Shape
- Rust 2024 workspace with resolver `3`; members are discovered from `apps/*` and `crates/*`.
- CLI package is `apps/cli` (`switchyard`); `apps/cli/src/main.rs` initializes encryption and terminal state, then hands off to the TUI loop in `apps/cli/src/application.rs`.
- Internal crates are path dependencies: `switchyard-core` has model/provider/session types and `switchyard-crypto` has Argon2/AES-GCM vault code. CLI-local provider and runtime path logic lives in `apps/cli/src/provider.rs` and `apps/cli/src/runtime.rs`.

## Commands
- Build all workspace members: `cargo build --workspace`
- Run the CLI TUI: `cargo run -p switchyard`
- Test all targets: `cargo test --workspace`
- Focus a test by name: `cargo test --workspace <test_name>`
- Test one package: `cargo test -p <package-name>`
- Format check: `cargo fmt --check`
- Lint all targets/features: `cargo clippy --workspace --all-targets --all-features -- -D warnings`

## Verification Notes
- GitHub Actions has separate stable-Rust workflows: Build runs `cargo build --workspace` and `cargo build --workspace --release`; CI runs `cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, and `cargo test --workspace`.
- No repo-local pre-commit hooks, rust-toolchain, rustfmt, clippy, or task-runner config exists; use installed Rust defaults.
- Tests are currently inline unit tests; there are no `tests/`, `benches/`, or `examples/` directories.
- Keep `Cargo.lock` committed; this workspace contains the `switchyard` binary application.
- `apps/cli/build.rs` sets `SWITCHYARD_VERSION` from `GITHUB_REF_NAME` when it matches the dev-release tag shape, otherwise from the current ISO week.
- Dev releases are tag-triggered only; `.github/workflows/dev-rel.yml` accepts tags matching `YYwWW[a-z]*` and builds `cargo build -p switchyard --release`.

## Provider Runtime
- Default provider is Ollama at `http://localhost:11434` with model `llama3.2`; set `SWITCHYARD_PROVIDER=llama.cpp` to use llama.cpp at `http://localhost:8080` with model `local-model`.
- Provider-specific env vars override service defaults: `SWITCHYARD_OLLAMA_BASE_URL`/`OLLAMA_BASE_URL`, `SWITCHYARD_OLLAMA_MODEL`/`OLLAMA_MODEL`, `SWITCHYARD_LLAMA_CPP_BASE_URL`/`LLAMA_CPP_BASE_URL`, and `SWITCHYARD_LLAMA_CPP_MODEL`/`LLAMA_CPP_MODEL`.
- In the TUI, `/provider ollama|llama.cpp` switches providers and resets the model to that provider's env/default model; `/model <name>` changes only the current model and strips a trailing `.gguf` for llama.cpp.

## Runtime Gotchas
- `cargo run -p switchyard` initializes/reads the vault before entering the TUI at `$XDG_STATE_HOME/switchyard/salt` only when `XDG_STATE_HOME` is absolute, otherwise `$HOME/.local/state/switchyard/salt`.
- First run prompts for an encryption password; `initial_password()` requires at least 8 chars with uppercase, lowercase, digit, and special char.
- Vault files use a 64-byte binary `SWYVLT` v1 header in `crates/crypto/src/storage`; treat changes there as persisted format changes.
- Logs go to both the in-TUI console buffer and `/tmp/switchyard.log`; `RUST_LOG` only overrides the default filter in debug builds.

## CLI Behavior
- The app uses `ratatui`/`crossterm` raw terminal mode and alternate screen; `Esc` or `/exit` exits, `F12` toggles logs, arrows/PageUp/PageDown scroll, and `Ctrl+Enter`/`Ctrl+J` inserts a newline.
