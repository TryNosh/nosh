# nosh

The shell that understands you.

nosh is a fast, native shell with built-in AI assistance, a plugin system, and a customizable prompt. Type commands normally or describe what you want in plain English — nosh translates it to the right command.

## Installation

```sh
curl -fsSL https://noshell.dev/install.sh | sh
```

Or build from source:

```sh
cargo install --path .
```

## Quick Start

```sh
nosh
```

Type commands directly, or prefix with `?` to use AI:

```
~/projects/app ❯ ?find large files in this directory
 ➜ find . -type f -size +100M -exec ls -lh {} \;
```

Use `??` for agentic mode — AI investigates before answering:

```
~/projects/app ❯ ??why are my tests failing
```

## Features

- **AI translation** — describe what you want, get the right command
- **Agentic mode** — AI runs commands, reads output, and investigates
- **Syntax highlighting** — commands, strings, flags, and operators are colored as you type
- **Plugins** — extend your prompt with git status, execution time, and more
- **Themes** — fully customizable prompt with conditional colors and inheritance
- **Completions** — tab completion for commands, flags, paths, and arguments
- **Fast** — native Rust, plugins run in parallel, ~2ms prompt latency
- **Safety layer** — AI-generated commands go through risk assessment and permission checks

## Configuration

Configuration lives in `~/.config/nosh/config.toml`:

```toml
[prompt]
theme = "builtins/default"
syntax_highlighting = true

[ai]
context_size = 10
agentic_enabled = true

[history]
load_count = 200
```

Shell customizations go in `~/.config/nosh/init.sh`:

```bash
alias ll='ls -la'
alias gs='git status'
export EDITOR=vim
```

## Commands

| Command | Description |
|---------|-------------|
| `?query` | Translate natural language to a command |
| `??query` | Agentic mode — AI investigates before answering |
| `/setup` | Sign in to nosh Cloud |
| `/config` | Open or edit config files |
| `/reload` | Reload config and theme |
| `/help` | Show all commands |

## Documentation

Full documentation at [noshell.dev/docs](https://noshell.dev/docs)

- [Configuration](https://noshell.dev/docs/configuration)
- [Themes](https://noshell.dev/docs/themes)
- [Plugins](https://noshell.dev/docs/plugins)
- [Completions](https://noshell.dev/docs/completions)
- [Packages](https://noshell.dev/docs/packages)

## License

Apache 2.0 — see [LICENSE](LICENSE) for details.
