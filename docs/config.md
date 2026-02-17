# Configuration

nosh configuration is stored in TOML format at `~/.config/nosh/config.toml`.

## Full Configuration Reference

```toml
# Onboarding flag (set automatically)
onboarding_complete = true

# Welcome message (empty = no message)
welcome_message = ""

[ai]
# Number of recent exchanges to include as context
context_size = 10

# Enable agentic mode (??)
agentic_enabled = true

# Max command executions per agentic query
max_iterations = 10

# Timeout in seconds (0 = no timeout)
timeout = 0

[behavior]
# Show translated command before running
show_command = true

[prompt]
# Theme name (see Theme Naming below)
theme = "builtins/default"

# Syntax highlighting for shell input
syntax_highlighting = true

[history]
# Commands to load for arrow-key navigation
load_count = 200
```

## Options Reference

### Root Level

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `onboarding_complete` | bool | `false` | Set after first-run setup |
| `welcome_message` | string | `""` | Message shown on shell start |

### `[ai]` Section

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `context_size` | int | `10` | Conversation memory size for `?` queries |
| `agentic_enabled` | bool | `true` | Enable `??` investigative mode |
| `max_iterations` | int | `10` | Max steps in agentic investigation |
| `timeout` | int | `0` | Agentic timeout in seconds (0 = unlimited) |

### `[behavior]` Section

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `show_command` | bool | `true` | Display AI command before execution |

### `[prompt]` Section

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `theme` | string | `"builtins/default"` | Active theme (see naming below) |
| `syntax_highlighting` | bool | `true` | Syntax highlighting for shell input |

### `[history]` Section

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `load_count` | int | `200` | Recent commands for arrow navigation |

## Theme Naming

| Source | Format | Example |
|--------|--------|---------|
| Built-in | `builtins/name` | `theme = "builtins/default"` |
| Local (from /create) | `name` | `theme = "mytheme"` |
| Package (from /install) | `package/name` | `theme = "awesome-pkg/dark"` |

## Example Configurations

### Minimal
```toml
onboarding_complete = true

[prompt]
theme = "builtins/default"
```

### Custom Local Theme
```toml
onboarding_complete = true

[prompt]
theme = "mytheme"
```

### Package Theme
```toml
onboarding_complete = true

[prompt]
theme = "starship-themes/gruvbox"
```

### Power User
```toml
onboarding_complete = true

[ai]
context_size = 20
max_iterations = 15
timeout = 300

[history]
load_count = 1000

[prompt]
theme = "builtins/default"
```

### Disable Agentic Mode
```toml
[ai]
agentic_enabled = false
```

## File Locations

```
~/.config/nosh/
├── config.toml              # Main configuration
├── credentials.toml         # API credentials (do not share)
├── permissions.toml         # Command permissions
├── history.db               # Command history (SQLite)
├── init.sh                  # Shell initialization script
├── themes/                  # Your local themes (from /create)
├── plugins/
│   └── community/           # Your local plugins (from /create)
└── packages/
    ├── builtins/            # Built-in themes, plugins, completions
    │   ├── themes/
    │   ├── plugins/
    │   └── completions/
    └── {package-name}/      # Git-installed packages
        ├── themes/
        ├── plugins/
        └── completions/
```

### Legacy Fallback

If `~/.nosh/` exists and `~/.config/nosh/` doesn't, nosh uses `~/.nosh/` for backwards compatibility.

## Slash Commands

| Command | Description |
|---------|-------------|
| `/setup` | Run setup wizard to sign in |
| `/usage` | Show usage, balance, manage subscription |
| `/buy` | Buy tokens or subscribe to a plan |
| `/config` | Open or edit config files |
| `/create` | Create or link a nosh package |
| `/install user/repo` | Install package from GitHub |
| `/upgrade` | Upgrade nosh to latest version |
| `/sync` | Sync config, builtins, and packages |
| `/packages` | List and manage installed packages |
| `/convert-zsh FILE` | Convert zsh completion to TOML |
| `/clear` | Clear AI conversation context |
| `/reload` | Reload config and theme |
| `/help` | Show help |
| `exit` | Quit nosh |

## Query Syntax

| Syntax | Description |
|--------|-------------|
| `command` | Run command directly |
| `?query` | Translate natural language via AI |
| `??query` | Agentic mode - AI investigates before answering |
