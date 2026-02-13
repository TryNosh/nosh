# Completion System

nosh uses TOML-based completions for tab completion. Completions can be hand-written or converted from ZSH completion files.

## Completion Locations

```
~/.config/nosh/packages/
├── builtins/
│   └── completions/
│       ├── git.toml
│       ├── cargo.toml
│       ├── npm.toml
│       └── docker.toml
└── awesome-pkg/             # Git-installed packages can provide completions
    └── completions/
        └── kubectl.toml
```

Files are named `{command}.toml` and loaded on-demand when you tab-complete that command.

## Updating Completions

```
/upgrade                     # Updates builtins and all packages
```

## Converting ZSH Completions

Convert existing ZSH completion files to nosh TOML format:

```
/convert-zsh /path/to/completion/file
```

Example:
```
/convert-zsh /usr/share/zsh/functions/Completion/Unix/_git
```

This generates TOML output that you can save to a completion file.

## TOML Completion Format

```toml
[completions.mycommand]
description = "My command description"

# Simple subcommands (name = description)
[completions.mycommand.subcommands]
build = "Build the project"
test = "Run tests"
run = "Run the project"

# Simple options (flag = description)
[completions.mycommand.options]
"-h" = "Show help"
"--version" = "Print version"

# Options that take values
"-c" = { description = "Config file", takes_value = true }
"--output" = { description = "Output path", takes_value = true, value_completer = "directories" }

# Positional argument completer
positional = "files"

# Dynamic completers (shell commands)
[completions.mycommand.dynamic]
my_items = { command = "my-command list --names 2>/dev/null" }
```

## Subcommand Options

Subcommands can have their own options and positional completers:

```toml
[completions.git.subcommands.commit]
description = "Record changes"
positional = "files"
options = [
    { name = "-m", description = "Commit message", takes_value = true },
    { name = "-a", description = "Stage all modified files" },
    { name = "--amend", description = "Amend previous commit" },
]
```

## Built-in Completers

Use these for `value_completer` or `positional`:

| Completer | Description |
|-----------|-------------|
| `files` | Files in current directory |
| `directories` | Directories only |
| `executables` | Executable files in PATH |
| `env_vars` | Environment variables |
| `users` | System users |
| `groups` | System groups |
| `hosts` | Known SSH hosts |
| `processes` | Running processes |
| `signals` | POSIX signals |

Example:
```toml
"--config" = { description = "Config path", takes_value = true, value_completer = "files" }
"--output-dir" = { description = "Output directory", takes_value = true, value_completer = "directories" }
positional = "directories"
```

## Dynamic Completers

Run shell commands to generate completions dynamically:

```toml
[completions.git.dynamic]
git_branches = { command = "git branch --format='%(refname:short)' 2>/dev/null" }
git_remotes = { command = "git remote 2>/dev/null" }
git_tags = { command = "git tag 2>/dev/null" }
```

Use dynamic completers in options or positional:
```toml
[completions.git.subcommands.checkout]
positional = "git_branches"
options = [
    { name = "-b", description = "Create new branch", takes_value = true },
]
```

### Caching

Results are cached for 5 seconds by default. Override with `cache_seconds`:

```toml
[completions.mycommand.dynamic]
slow_completer = { command = "expensive-command", cache_seconds = 30 }
fast_completer = { command = "quick-command", cache_seconds = 1 }
```

## ZSH Completion Conversion

### Supported Syntax

```zsh
#compdef mycommand

_mycommand() {
    _arguments \
        '-h[Show help]' \
        '--version[Print version]' \
        '-c[Config file]:config:_files' \
        '1:subcommand:(build test run)'
}
```

Converts to:
```toml
[completions.mycommand]

[completions.mycommand.options]
"-h" = "Show help"
"--version" = "Print version"
"-c" = { description = "Config file", takes_value = true }

[completions.mycommand.subcommands]
build = ""
test = ""
run = ""
```

### Parsed Elements

| ZSH Syntax | TOML Result |
|------------|-------------|
| `#compdef cmd` | Command name |
| `-h[Help]` | Short option |
| `--verbose[Verbose output]` | Long option |
| `--config[Description]:type:_files` | Option with value completer |
| `'1:name:(a b c)'` | Subcommands |
| `_describe` blocks | Subcommands with descriptions |

## Example: Complete Git Completion

```toml
[completions.git]
description = "Git version control"

[completions.git.options]
"--version" = "Print git version"
"-C" = { description = "Run as if started in path", takes_value = true, value_completer = "directories" }
"--git-dir" = { description = "Set path to repository", takes_value = true, value_completer = "directories" }

[completions.git.subcommands]
status = "Show working tree status"
log = "Show commit logs"
diff = "Show changes"
add = "Add file contents to index"
commit = "Record changes to repository"
push = "Update remote refs"
pull = "Fetch and integrate with another repository"
checkout = "Switch branches or restore files"
branch = "List, create, or delete branches"
merge = "Join two or more development histories"

[completions.git.subcommands.commit]
description = "Record changes to repository"
positional = "files"
options = [
    { name = "-m", description = "Commit message", takes_value = true },
    { name = "-a", description = "Stage all modified files" },
    { name = "--amend", description = "Amend previous commit" },
    { name = "--no-edit", description = "Use previous commit message" },
]

[completions.git.subcommands.checkout]
description = "Switch branches or restore files"
positional = "git_branches"
options = [
    { name = "-b", description = "Create and switch to new branch", takes_value = true },
    { name = "-B", description = "Create/reset and switch to branch", takes_value = true },
]

[completions.git.subcommands.add]
description = "Add file contents to index"
positional = "files"
options = [
    { name = "-A", description = "Add all files" },
    { name = "-p", description = "Interactively choose hunks" },
]

[completions.git.dynamic]
git_branches = { command = "git branch --format='%(refname:short)' 2>/dev/null" }
git_remotes = { command = "git remote 2>/dev/null" }
git_tags = { command = "git tag 2>/dev/null" }
```

## Completion Context

The system determines what to complete based on cursor position:

| Context | Trigger | Completes |
|---------|---------|-----------|
| Command | First word | Executables from PATH |
| Subcommand | After command | Defined subcommands |
| Option | Starts with `-` | Defined options |
| Option Value | After option with `takes_value` | Uses `value_completer` |
| Positional | Other positions | Uses `positional` completer |

## Creating Package Completions

Packages can provide completions for any command. Create `completions/{command}.toml` in your package:

```
my-nosh-package/
└── completions/
    ├── kubectl.toml
    └── helm.toml
```

After `/install user/my-nosh-package`, tab completion for `kubectl` and `helm` will work automatically.

## Tips

1. **Start simple** - Add subcommands and common options first
2. **Use dynamic completers** - For values that change (branches, containers, etc.)
3. **Test incrementally** - Tab complete after each change to verify
4. **Check existing completions** - Look at `packages/builtins/completions/` for examples
