# Changelog

## Since 26w24a

### Added

- Added slash commands in the TUI:
  - `/exit` exits the application.
  - `/provider [ollama|llama.cpp]` switches provider, or opens a provider picker when run without arguments.
  - `/model [name]` switches model, or opens a model picker when run without arguments.
- Added in-app provider and model selection menus for switching between Ollama and llama.cpp without restarting.
- Added automatic model discovery to the model picker for Ollama and llama.cpp local providers.
- Added short-lived keyring caching for the vault encryption key, reducing repeat password prompts during a session.
- Added persistence for devtools visibility, selected provider, and selected model across CLI runs.

### Changed

- Provider switches now reset the selected model to that provider's configured default.
- llama.cpp model names entered with a trailing `.gguf` are normalized by stripping the extension.

### Internal

- Local snapshot versions are now derived from existing dev-release git tags for the current ISO week, while GitHub release builds still use matching tag names directly.
- Split TUI slash-command handling into `apps/cli/src/command.rs` with unit coverage.
- Added key cache serialization, expiry, invalid-cache cleanup, and unit coverage in `apps/cli/src/key_cache.rs`.
