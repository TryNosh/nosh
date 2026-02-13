# nosh

The shell that understands you.

nosh is a modern shell with built-in AI assistance, a powerful plugin system, and a customizable prompt.

## Installation

### Quick Install (Recommended)

```sh
curl -fsSL https://noshell.dev/install.sh | sh
```

### From Source

```sh
cargo install --path .
```

## Features

- **AI-powered completions** - Type naturally and let AI help translate to commands
- **Plugin system** - Extend your prompt with plugins for git, languages, weather, and more
- **Themes** - Customize your prompt appearance with powerful theming
- **Conditional colors** - Colors that change based on values (temperature, battery, git status)
- **Theme inheritance** - Build on existing themes and customize what you need
- **Fast** - Plugins run in parallel with configurable timeouts

## Getting Started

After installation, run:

```sh
nosh
```

To make nosh your default shell:

```sh
chsh -s $(which nosh)
```

## Documentation

Full documentation at [noshell.dev/docs](https://noshell.dev/docs)

- [Configuration](https://noshell.dev/docs/configuration)
- [Themes](https://noshell.dev/docs/themes)
- [Plugins](https://noshell.dev/docs/plugins)
- [Packages](https://noshell.dev/docs/packages)

## Configuration

Configuration lives in `~/.config/nosh/config.toml`:

```toml
[prompt]
theme = "builtins/default"

[ai]
enabled = true
provider = "mistral"
```

## License

MIT
