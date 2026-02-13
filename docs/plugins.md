# Plugin System

nosh uses a TOML-based plugin system for extending prompt functionality. Plugins provide dynamic variables that can be referenced in themes.

## Plugin Locations

```
~/.config/nosh/
├── plugins/
│   └── community/           # Your local plugins (from /create)
│       └── myplugin.toml
└── packages/
    ├── builtins/            # Ships with nosh
    │   └── plugins/
    │       ├── git.toml
    │       ├── exec_time.toml
    │       └── context.toml
    └── awesome-pkg/         # Git-installed packages
        └── plugins/
            └── fancy.toml
```

## How Plugins Execute

When your prompt renders, nosh fetches all plugin values **in parallel** with smart timeout handling:

- **Parallel execution** - All plugin commands run concurrently via async tasks
- **Soft timeout (100ms)** - If a command doesn't finish in time, nosh uses the cached value
- **Background continuation** - Slow commands keep running after the prompt appears, updating the cache for next time
- **Hard timeout (5s)** - Commands running too long are terminated to prevent resource buildup
- **No duplicate runs** - If a plugin is still running from the previous prompt, nosh won't start a new instance

This means your prompt stays fast even with slow plugins. On first entry to a directory, a slow plugin might show nothing (empty cache), but by the next prompt it will have the value ready.

## Plugin Types

### Built-in Plugins (`builtins/`)

Embedded in nosh and installed to `packages/builtins/plugins/` on first run.

- `builtins/context` - Language versions, git info via nosh-context library
- `builtins/exec_time` - Command execution duration
- `builtins/git` - Git branch and status via shell commands

Update via `/upgrade` when you update nosh.

### Local Plugins

Your own plugins created with `/create`. Stored in `packages/local/plugins/`. Use `local/` prefix in themes.

### Package Plugins

Installed from GitHub with `/install user/repo`. Stored in `packages/{name}/plugins/`. Use `{package/plugin:var}` format.

## Using Plugins in Themes

Reference plugin variables as `{package/plugin:variable}`:

```toml
[prompt]
# Built-in plugins use builtins/ prefix
format = "[{builtins/context:git_branch}](purple) [{builtins/exec_time:took}](yellow)"

# Local plugins (from /create) use local/ prefix
format = "[{local/myplugin:myvar}](cyan)"

# Package plugins use their package name
format = "[{awesome-pkg/fancy:myvar}](cyan)"
```

## Plugin Format

```toml
[plugin]
name = "myplugin"
description = "What this plugin does"

[provides]
# Command-based variable
myvar = { command = "echo hello" }

# With transform
status = { command = "some-check", transform = "non_empty" }

# Internal variable (from nosh-context)
git_branch = { source = "internal" }

[icons]
dirty = "*"
clean = ""

[config]
min_ms = 500
```

## Variable Providers

### Command-based

Executes a shell command and uses the output. Results are cached for 500ms.

```toml
[provides]
branch = { command = "git branch --show-current 2>/dev/null" }
uptime = { command = "uptime | awk '{print $3}'" }
```

### Transform-based

Processes command output with a transform function:

```toml
[provides]
dirty = { command = "git status --porcelain 2>/dev/null", transform = "non_empty" }
```

**Available transforms:**

| Transform | Description |
|-----------|-------------|
| `non_empty` | Returns icon from `[icons]` based on whether output exists (uses `dirty`/`clean` icon names) |
| `with_icon` | Prepends the variable's icon to the value; hides entirely if empty |
| `trim` | Trims whitespace from output |

**`non_empty` transform** - for status indicators:

```toml
[provides]
dirty = { command = "git status --porcelain", transform = "non_empty" }

[icons]
dirty = "*"    # Returned when command has output
clean = ""     # Returned when command output is empty
```

**`with_icon` transform** - for values with conditional icons:

```toml
[provides]
temp = { command = "curl -s 'wttr.in?format=%t'", transform = "with_icon" }

[icons]
temp = "🌡️"    # Icon name matches variable name
```

When the command returns `+5°C`, the variable outputs `🌡️ +5°C`.
When the command returns empty (or times out), the variable outputs nothing - useful for hiding the entire segment when data isn't available.

### Timeout and Cache Settings

Each variable can have custom timeout and cache settings:

```toml
[provides]
# Fast command - wait up to 50ms, cache for 1 second
fast_var = { command = "echo hello", timeout = "50ms", cache = "1s" }

# Slow API - don't wait at all (fully async), cache for 5 minutes
weather = { command = "curl -s 'wttr.in?format=%t'", timeout = "0", cache = "5m" }

# Volatile data - always fetch fresh (no caching)
time = { command = "date +%H:%M:%S", cache = "always" }

# Static data - cache forever (only fetch once)
hostname = { command = "hostname", cache = "never" }
```

**Timeout options:**

| Value | Behavior |
|-------|----------|
| `"0"` | Fully async - don't wait, immediately use cached value |
| `"100ms"` | Wait up to 100ms (default) |
| `"1s"` | Wait up to 1 second |

**Cache options:**

| Value | Behavior |
|-------|----------|
| `"always"` | No caching - always fetch fresh data |
| `"never"` | Cache forever - only fetch once per session |
| `"500ms"` | Cache for 500ms (default) |
| `"10s"` | Cache for 10 seconds |
| `"5m"` | Cache for 5 minutes |
| `"1h"` | Cache for 1 hour |

**Duration format:** Number followed by unit: `ms` (milliseconds), `s` (seconds), `m` (minutes), `h` (hours). If no unit specified, defaults to milliseconds.

### Internal

Uses built-in providers from the nosh-context library. These are fast because they don't spawn shell processes.

```toml
[provides]
rust_version = { source = "internal" }
```

**Available internal variables** (via `builtins/context`):

| Variable | Description |
|----------|-------------|
| `git_branch` | Current git branch |
| `git_status` | Status indicator (clean/dirty) |
| `package_name` | Package name from package.json/Cargo.toml/etc |
| `package_version` | Package version |
| `package_icon` | Package icon (📦) |
| `rust_version` | Rust toolchain version |
| `rust_icon` | Rust icon (🦀) |
| `node_version` | Node.js version |
| `node_icon` | Node icon (⬢) |
| `bun_version` | Bun version |
| `bun_icon` | Bun icon (🥟) |
| `go_version` | Go version |
| `go_icon` | Go icon (🐹) |
| `python_version` | Python version |
| `python_icon` | Python icon (🐍) |
| `cpp_version` | C++ compiler version |
| `cpp_icon` | C++ icon (⚙️) |
| `docker_version` | Docker version |
| `docker_icon` | Docker icon (🐳) |

## Built-in Plugins Reference

### builtins/context

Provides all internal variables listed above. This is the recommended plugin for language/tool detection.

```toml
# In your theme
format = "[{builtins/context:git_branch}](purple) [{builtins/context:rust_version}](red)"

[plugins]
"builtins/context" = { enabled = true }
```

### builtins/exec_time

Shows command execution duration.

```toml
[provides]
duration = { source = "internal" }  # e.g., "1.2s"
took = { source = "internal" }      # e.g., "took 1.2s"

[config]
min_ms = 500  # Only show if command took longer than this
```

Usage:
```toml
format = "[{builtins/exec_time:took}](yellow)"

[plugins]
"builtins/exec_time" = { enabled = true, min_ms = 1000 }
```

### builtins/git

Git info via shell commands (alternative to context plugin's git support).

```toml
[provides]
branch = { command = "git branch --show-current 2>/dev/null" }
dirty = { command = "git status --porcelain 2>/dev/null", transform = "non_empty" }

[icons]
dirty = "*"
clean = ""
```

Usage: `{builtins/git:branch}`, `{builtins/git:dirty}`

## Creating a Plugin

### Using /create

```
/create
> Plugin
> Enter name: weather
```

Creates `~/.config/nosh/packages/local/plugins/weather.toml` with a template.

### Manual Creation

Create `~/.config/nosh/packages/local/plugins/myplugin.toml`:

```toml
[plugin]
name = "myplugin"
description = "My custom plugin"

[provides]
myvar = { command = "echo hello" }
```

Use in theme:
```toml
format = "[{local/myplugin:myvar}](cyan)"
```

### Example: Weather Plugin

```toml
[plugin]
name = "weather"
description = "Current weather from wttr.in"

[provides]
temp = { command = "curl -s 'wttr.in?format=%t' 2>/dev/null", transform = "with_icon" }
condition = { command = "curl -s 'wttr.in?format=%C' 2>/dev/null" }

[icons]
temp = "🌡️"
```

Usage:
```toml
format = "[{local/weather:temp}](cyan) $ "
```

With `with_icon`, the temperature shows as `🌡️ +5°C` when available, or nothing when the API times out.

### Example: Battery Plugin (macOS)

```toml
[plugin]
name = "battery"
description = "Battery status"

[provides]
percent = { command = "pmset -g batt | grep -o '[0-9]*%' | head -1" }
```

### Example: Kubernetes Context

```toml
[plugin]
name = "k8s"
description = "Kubernetes context"

[provides]
context = { command = "kubectl config current-context 2>/dev/null" }
namespace = { command = "kubectl config view --minify -o jsonpath='{..namespace}' 2>/dev/null" }
```

## Installing Package Plugins

```
/install user/repo              # Install from GitHub
/install https://github.com/... # Install from full URL
```

Manage packages:
```
/upgrade                        # Update builtins and all packages
/packages                       # List and remove packages
```

Packages are cloned to `~/.config/nosh/packages/` and can contain themes, plugins, and completions.

## Plugin Configuration in Themes

Enable, disable, or configure plugins per-theme:

```toml
[plugins]
"builtins/context" = { enabled = true }
"builtins/exec_time" = { enabled = true, min_ms = 500 }
"builtins/git" = { enabled = false }
"myplugin" = { enabled = true }
"awesome-pkg/fancy" = { enabled = true }
```

When a plugin is disabled, its variables return empty strings and are hidden from the prompt.

## Performance Tips

1. **Prefer `builtins/context`** over shell commands for git/language detection - it's synchronous and never delays your prompt
2. **Use `timeout = "0"` for slow commands** - API calls or slow commands should use `timeout = "0"` to avoid delaying your prompt. Shows cached value immediately while fetching in background.
3. **Cache aggressively for stable data** - Use `cache = "5m"` or `cache = "never"` for data that doesn't change often (weather, hostname, k8s context)
4. **Use `cache = "always"` sparingly** - Only for truly volatile data that must be fresh every prompt
5. **Keep commands under 100ms when possible** - Faster commands show fresh data immediately instead of cached values
6. **Disable unused plugins** - Set `enabled = false` in your theme to skip them entirely
7. **Use `2>/dev/null`** - Suppress error output to speed up commands that may fail
8. **Hard timeout is 5 seconds** - Design plugins to complete within a reasonable time or they'll be terminated
