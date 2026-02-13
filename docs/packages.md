# Package System

nosh uses a unified package system for themes, plugins, and completions. Packages can be built-in (ships with nosh) or installed from GitHub.

## Directory Structure

```
~/.config/nosh/
├── themes/                  # Your local themes (from /create)
│   └── mytheme.toml
├── plugins/
│   └── community/           # Your local plugins (from /create)
│       └── myplugin.toml
└── packages/
    ├── builtins/            # Ships with nosh, updated via /upgrade
    │   ├── themes/
    │   │   └── default.toml
    │   ├── plugins/
    │   │   ├── git.toml
    │   │   ├── exec_time.toml
    │   │   └── context.toml
    │   └── completions/
    │       ├── git.toml
    │       ├── cargo.toml
    │       ├── npm.toml
    │       └── docker.toml
    └── awesome-pkg/         # Git-installed package
        ├── themes/
        │   └── dark.toml
        ├── plugins/
        │   └── fancy.toml
        └── completions/
            └── mytool.toml
```

## Naming Convention

| Source | Theme | Plugin Variable | Example |
|--------|-------|-----------------|---------|
| Built-in | `builtins/name` | `{builtins/plugin:var}` | `theme = "builtins/default"` |
| Local | `name` | `{plugin:var}` | `theme = "mytheme"` |
| Package | `package/name` | `{package/plugin:var}` | `theme = "awesome-pkg/dark"` |

## Installing Packages

Install packages from GitHub:

```
/install user/repo              # GitHub shorthand
/install https://github.com/... # Full URL
/install https://gitlab.com/... # Any Git URL
```

Example:
```
/install someuser/nosh-themes
```

This clones the repository to `~/.config/nosh/packages/nosh-themes/`.

## Updating Packages

The `/upgrade` command updates everything:

```
/upgrade
```

Output:
```
Checking for updates...

Builtins:
  Up to date: Default theme
  Up to date: Git plugin
  Updated: Context plugin
  Up to date: Git completions
  ...

Packages:
  Updated: awesome-themes
  Up to date: cool-plugins

2 item(s) updated.
```

**How updates work:**
- **Builtins**: Compared against content embedded in the nosh binary. Updates when you install a new version of nosh.
- **Git packages**: Runs `git pull` to fetch latest changes from the remote repository.

## Managing Packages

List and remove packages:

```
/packages
```

Shows installed packages with their contents (themes, plugins, completions) and lets you remove them.

## Creating a Package

A package is a Git repository with this structure:

```
my-nosh-package/
├── themes/           # Optional
│   └── mytheme.toml
├── plugins/          # Optional
│   └── myplugin.toml
└── completions/      # Optional
    └── mytool.toml
```

At least one of `themes/`, `plugins/`, or `completions/` should exist.

### Theme File Format

```toml
# themes/dark.toml
[prompt]
format = "[{dir}](blue) [{builtins/context:git_branch}](purple) $ "
char = ">"
char_error = "!"

[plugins]
"builtins/context" = { enabled = true }

[colors]
path = "#5f87af"
error = "#d75f5f"
```

### Plugin File Format

```toml
# plugins/myplugin.toml
[plugin]
name = "myplugin"
description = "What this plugin does"

[provides]
myvar = { command = "echo hello" }
status = { command = "some-check", transform = "non_empty" }

[icons]
dirty = "*"
clean = ""
```

### Completion File Format

```toml
# completions/mytool.toml
[completions.mytool]
description = "My tool description"

[completions.mytool.subcommands]
build = "Build the project"
test = "Run tests"

[completions.mytool.options]
"-h" = "Show help"
"--verbose" = "Verbose output"
```

## Publishing a Package

1. Create a Git repository with the structure above
2. Push to GitHub (or any Git host)
3. Users install with `/install username/repo-name`

Example repository: `github.com/yourname/nosh-cool-themes`

Users install:
```
/install yourname/nosh-cool-themes
```

## Using Package Content

### Themes

In `~/.config/nosh/config.toml`:
```toml
[prompt]
theme = "cool-themes/dark"    # package-name/theme-name
```

### Plugins

In your theme's format string:
```toml
format = "[{cool-themes/fancy:myvar}](cyan)"
```

### Completions

Completions are loaded automatically. If a package provides `completions/kubectl.toml`, tab completion for `kubectl` will work immediately after installation.

## Package Registry

Installed packages are tracked in `~/.config/nosh/packages.toml`:

```toml
[packages.awesome-themes]
name = "awesome-themes"
source = "https://github.com/user/awesome-themes.git"
installed_at = "1707123456"
last_updated = "1707123456"
```

This file is managed automatically by `/install`, `/upgrade`, and `/packages` commands.

## Error Handling

| Error | Cause | Solution |
|-------|-------|----------|
| "Git is not installed" | git command not found | Install git |
| "Could not clone repository" | Invalid URL or network issue | Check URL and connection |
| "Package 'X' is already installed" | Duplicate install | Use `/upgrade` to update |
| "Theme 'pkg/theme' not found" | Package not installed | Run `/install` first |
