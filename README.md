# SwitchYard

SwitchYard is an agent harness for working with model providers.

Currently, SwitchYard is implemented as a CLI application.

## Supported Platforms

SwitchYard currently targets Linux on x86_64.

Other Unix-like platforms may work, but are not actively tested. Windows is not currently supported.

## Installation

Install with `curl`:

```sh
curl -L https://github.com/borna/zozin/releases/latest/download/switchyard-linux-x86_64.tar.gz -o switchyard-linux-x86_64.tar.gz
tar -xzf switchyard-linux-x86_64.tar.gz
chmod +x switchyard-linux-x86_64
mkdir -p ~/.local/bin
mv switchyard-linux-x86_64 ~/.local/bin/switchyard
```

Or install with `wget`:

```sh
wget https://github.com/borna/zozin/releases/latest/download/switchyard-linux-x86_64.tar.gz -O switchyard-linux-x86_64.tar.gz
tar -xzf switchyard-linux-x86_64.tar.gz
chmod +x switchyard-linux-x86_64
mkdir -p ~/.local/bin
mv switchyard-linux-x86_64 ~/.local/bin/switchyard
```

Make sure `~/.local/bin` is on your `PATH`, then run:

```sh
switchyard
```

## Requirements

SwitchYard connects to a local model provider. The supported providers are Ollama and llama.cpp.

### Ollama

Ollama is the default provider.

SwitchYard connects to:

```text
http://localhost:11434
```

and uses this model by default:

```text
llama3.2
```

Make sure Ollama is running and the model is available before starting SwitchYard.

### llama.cpp

SwitchYard also supports llama.cpp-compatible local servers.

By default, SwitchYard connects to:

```text
http://localhost:8080
```

and uses this model name:

```text
local-model
```

Start SwitchYard with llama.cpp as the provider:

```sh
SWITCHYARD_PROVIDER=llama.cpp switchyard
```

## Environment Variables

Use `SWITCHYARD_PROVIDER` to choose the startup provider:

```sh
SWITCHYARD_PROVIDER=ollama switchyard
SWITCHYARD_PROVIDER=llama.cpp switchyard
```

Configure Ollama with:

```sh
SWITCHYARD_OLLAMA_BASE_URL=http://localhost:11434
SWITCHYARD_OLLAMA_MODEL=llama3.2
```

Configure llama.cpp with:

```sh
SWITCHYARD_LLAMA_CPP_BASE_URL=http://localhost:8080
SWITCHYARD_LLAMA_CPP_MODEL=local-model
```

SwitchYard also reads these provider-native fallback variables when the matching `SWITCHYARD_*` variable is not set:

```sh
OLLAMA_BASE_URL
OLLAMA_MODEL
LLAMA_CPP_BASE_URL
LLAMA_CPP_MODEL
```

Provider and model selections are persisted after the CLI exits. Environment variables set the initial defaults, but persisted selections are reused on later runs.

## First Run

On first run, SwitchYard prompts for an encryption password and creates its local state directory.

State is stored under:

```text
$XDG_STATE_HOME/switchyard
```

If `XDG_STATE_HOME` is unset or relative, SwitchYard falls back to:

```text
$HOME/.local/state/switchyard
```

## License

SwitchYard is licensed under the [BSD-3-Clause](https://opensource.org/licenses/BSD-3-Clause) License. See [LICENSE](LICENSE).
