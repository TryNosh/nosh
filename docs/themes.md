# Theme System

Themes control prompt appearance using TOML configuration with markdown-style color syntax.

## Theme Locations

```
~/.config/nosh/
├── themes/                  # Your local themes (from /create)
│   └── mytheme.toml
└── packages/
    ├── builtins/            # Ships with nosh
    │   └── themes/
    │       └── default.toml
    └── awesome-themes/      # Git-installed packages
        └── themes/
            └── dark.toml
```

## Setting the Active Theme

In `~/.config/nosh/config.toml`:

```toml
[prompt]
theme = "builtins/default"       # Built-in theme (default)
theme = "mytheme"                # Your local theme (in themes/)
theme = "awesome-themes/dark"    # Package theme (from /install)
```

## Theme Inheritance

Themes can extend other themes, inheriting their settings and only overriding what you need:

```toml
# ~/.config/nosh/themes/mytheme.toml
extends = "builtins/default"

[prompt]
# Only override the prompt format - everything else comes from default
format = "[{user}@{host}](dim) [{dir}](blue) [{prompt:char}](green) "
char = "$"
```

**What gets inherited:**
- `prompt` - format, char, char_error (only if not specified in child)
- `plugins` - merged, child overrides parent for same plugin
- `colors` - simple colors and conditional colors merged, child overrides parent

**Inheritance chain:** Themes can extend themes that extend other themes (max depth: 10).

```toml
# minimal.toml
extends = "builtins/default"

[prompt]
format = "[{dir}](blue) $ "

# Everything else (plugins, colors, conditional colors) inherited from default
```

## Installing Theme Packages

```
/install user/repo          # Install package from GitHub
/upgrade                    # Update builtins and all packages
/packages                   # List and manage packages
```

## Theme Format

```toml
[prompt]
# Format string with styled segments and variables
format = "[{dir}](blue bold) [{prompt:char}](green) "

# Prompt character for successful commands
char = "❯"

# Prompt character after failed commands
char_error = "❯"

[plugins]
# Enable/disable plugins and set options
"builtins/context" = { enabled = true }
"builtins/exec_time" = { enabled = true, min_ms = 1000 }

[colors]
# Named colors for semantic use (optional)
path = "#5f87af"
git_clean = "#87af87"
git_dirty = "#d7af5f"
error = "#d75f5f"
warning = "#d7af5f"
success = "#87af87"
info = "#87afd7"
ai_command = "#af87d7"
```

## Styled Segments

Use markdown-style syntax: `[text](color modifiers)`

```toml
format = "[{dir}](blue bold) [{builtins/context:git_branch}](purple italic)"
```

### Colors

**Named colors:**
- `black`, `red`, `green`, `yellow`, `blue`, `purple` (or `magenta`), `cyan`, `white`

**Hex colors (24-bit truecolor):**
- `#RRGGBB` format, e.g., `#5f87af`

### Modifiers

- `bold` - Bold text
- `dim` - Dimmed text
- `italic` - Italic text
- `underline` - Underlined text

Combine with spaces: `[text](blue bold underline)`

## Conditional Colors

Colors can change based on the value being displayed. Define conditional colors in `[colors]`:

```toml
[prompt]
format = "[{weather/weather:temp}](temperature) [{battery/battery:percent}](battery_level)"

# Conditional color: changes based on the value
[colors.temperature]
default = "white"
rules = [
  { match = "^-", color = "blue" },      # Negative = cold (blue)
  { below = 10, color = "cyan" },         # Below 10 = cool (cyan)
  { above = 25, color = "red" },          # Above 25 = hot (red)
]

[colors.battery_level]
default = "green"
rules = [
  { below = 20, color = "red" },          # Critical
  { below = 50, color = "yellow" },       # Low
]

[colors.git_indicator]
default = "green"
rules = [
  { not_empty = true, color = "yellow" }, # Dirty = yellow
]
```

### Rule Conditions

Rules are evaluated in order; first match wins. Available conditions:

| Condition | Description | Example |
|-----------|-------------|---------|
| `empty` | Value is empty | `{ empty = true, color = "gray" }` |
| `not_empty` | Value is not empty | `{ not_empty = true, color = "red" }` |
| `contains` | Value contains string | `{ contains = "error", color = "red" }` |
| `match` | Value matches regex | `{ match = "^-", color = "blue" }` |
| `above` | Numeric value > threshold | `{ above = 25, color = "red" }` |
| `below` | Numeric value < threshold | `{ below = 10, color = "blue" }` |

Multiple conditions in one rule must ALL match (AND logic):

```toml
rules = [
  { above = 0, below = 10, color = "cyan" },  # Between 0 and 10
]
```

### Numeric Extraction

For `above`/`below` conditions, numbers are extracted from values like:
- `+5°C` → 5
- `-10.5°F` → -10.5
- `85%` → 85
- `v1.2.3` → 1 (first number found)

### Built-in Conditional Colors

The default theme includes these conditional colors ready to use:

| Color Name | Use Case | Rules |
|------------|----------|-------|
| `temperature` | Weather temps | blue (<5°), cyan (<15°), white (15-20°), yellow (20-30°), red (>30°) |
| `battery` | Battery % | red (<15%), yellow (<30%), green (≥30%) |
| `percentage` | Any % value | red (<20%), yellow (<50%), green (≥50%) |
| `git_indicator` | Git status | green (clean/empty), yellow (dirty/has content) |
| `load` | CPU/load avg | green (<1.0), yellow (1.0-2.0), red (>2.0) |

Use them directly or inherit them via `extends = "builtins/default"`:

```toml
extends = "builtins/default"

[prompt]
format = "[{weather:temp}](temperature) [{battery:percent}](battery) "
```

## Built-in Variables

| Variable | Description |
|----------|-------------|
| `{cwd}` | Full current directory path |
| `{dir}` or `{cwd_short}` | Last path component (~ for home) |
| `{user}` | Username |
| `{host}` | Hostname |
| `{newline}` or `\n` | Line break |
| `{prompt:char}` | Prompt character (uses `char` or `char_error` based on last exit code) |

## Plugin Variables

Format: `{package/plugin:variable}`

```toml
format = """
[{dir}](blue) [{builtins/context:git_branch}](purple){builtins/context:git_status}
[{builtins/context:rust_version}](red) [{builtins/exec_time:took}](yellow)
[{prompt:char}](green) """
```

### Plugin Naming

| Source | Format | Example |
|--------|--------|---------|
| Built-in | `builtins/plugin` | `{builtins/context:git_branch}` |
| Local (from /create) | `plugin` | `{myplugin:myvar}` |
| Package (from /install) | `package/plugin` | `{awesome-pkg/fancy:myvar}` |

### Available Built-in Variables

From `builtins/context`:
- `git_branch` - Current git branch
- `git_status` - Git status indicator (clean/dirty)
- `package_name`, `package_version`, `package_icon` - Package info
- `rust_version`, `rust_icon` - Rust toolchain
- `node_version`, `node_icon` - Node.js
- `bun_version`, `bun_icon` - Bun runtime
- `go_version`, `go_icon` - Go
- `python_version`, `python_icon` - Python
- `cpp_version`, `cpp_icon` - C++
- `docker_version`, `docker_icon` - Docker

From `builtins/exec_time`:
- `duration` - Command duration (e.g., "1.2s")
- `took` - Duration with prefix (e.g., "took 1.2s")

From `builtins/git`:
- `branch` - Current branch name
- `dirty` - Dirty indicator icon

## Plugin Configuration

Enable/disable plugins and set options per-theme:

```toml
[plugins]
"builtins/context" = { enabled = true }
"builtins/exec_time" = { enabled = true, min_ms = 1000 }
"builtins/git" = { enabled = false }
"awesome-pkg/fancy" = { enabled = true }
```

The `min_ms` option for `exec_time` sets the minimum duration (in milliseconds) before showing execution time.

## Creating a Theme

Use the `/create` command:

```
/create
> Theme
> Enter name: mytheme
```

This creates `~/.config/nosh/packages/local/themes/mytheme.toml` with a starter template.

Or manually create a file:

```toml
# ~/.config/nosh/packages/local/themes/mytheme.toml
[prompt]
format = "[{dir}](blue) $ "
char = "$"
char_error = "!"
```

Then set it in config:

```toml
[prompt]
theme = "local/mytheme"
```

## Example: Minimal Theme

```toml
[prompt]
format = "[{dir}](blue) $ "
char = "$"
char_error = "!"
```

## Example: Two-Line Theme

```toml
[prompt]
format = """
[{user}](green)@[{host}](yellow):[{dir}](blue bold)
[{builtins/context:git_branch}](purple){builtins/context:git_status} \
[{builtins/exec_time:took}](dim)
[{prompt:char}](green bold) """
char = "❯"
char_error = "✗"

[plugins]
"builtins/context" = { enabled = true }
"builtins/exec_time" = { enabled = true, min_ms = 500 }

[colors]
path = "blue"
git_clean = "green"
git_dirty = "yellow"
error = "red bold"
ai_command = "purple"
```

## Example: Developer Theme

Shows language versions when in relevant project directories:

```toml
[prompt]
format = """
[{dir}](blue bold) [{builtins/context:git_branch}](purple){builtins/context:git_status} \
{builtins/context:rust_icon} [{builtins/context:rust_version}](red) \
{builtins/context:node_icon} [{builtins/context:node_version}](green) \
{builtins/context:python_icon} [{builtins/context:python_version}](blue) \
[{builtins/exec_time:took}](yellow)
[{prompt:char}](green bold) """
char = "❯"
char_error = "❯"

[plugins]
"builtins/context" = { enabled = true }
"builtins/exec_time" = { enabled = true, min_ms = 1000 }
```

## Tips

- Empty variables are automatically hidden (no leftover brackets)
- Multiple spaces are collapsed to single spaces
- Use `\` at end of line in multiline strings to continue without newline
- The `[colors]` section is for your reference; use colors directly in format string
- Plugins run in parallel with a 100ms soft timeout, so adding more plugins won't slow your prompt proportionally
