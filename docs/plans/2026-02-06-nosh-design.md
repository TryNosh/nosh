# nosh Design Document

**Date:** 2026-02-06
**Status:** Draft

## Overview

nosh ("natural shell") is a shell that translates natural language into shell commands using AI. Instead of `ls -lhS | grep -v '^d'`, users type `list all files here, sort by size`.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                         nosh (Rust)                         │
│                     Local Shell Client                      │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │   REPL +    │  │   Plugin    │  │   Safety/Permission │  │
│  │  Job Ctrl   │  │   System    │  │       Engine        │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │  History +  │  │   Config    │  │    AI Adapter       │  │
│  │ Autocomplete│  │   (TOML)    │  │  (Ollama / Cloud)   │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                              │
              ┌───────────────┴───────────────┐
              ▼                               ▼
   ┌────────────────────┐          ┌────────────────────┐
   │   Ollama (local)   │          │    Nosh Cloud      │
   │   User's machine   │          │ (Hono on Vercel)   │
   └────────────────────┘          └────────────────────┘
                                             │
                                   ┌─────────┴─────────┐
                                   ▼                   ▼
                            ┌───────────┐       ┌───────────┐
                            │ Replicate │       │  Stripe   │
                            └───────────┘       └───────────┘
```

## Components

### 1. Rust Shell Client

**Features:**
- AI-first: all input goes through AI for translation
- History (up/down arrows)
- Autocomplete (tab for files/paths)
- Job control (ctrl+z, fg, bg)
- Plugin system for themes and hooks
- Smart safety system with command parsing

**Key crates:**
- `rustyline` - readline, history, autocomplete
- `tokio` - async runtime
- `reqwest` - HTTP client
- `serde` + `toml` - config parsing
- `crossterm` - terminal colors
- `shell-words` / `shlex` - parse shell commands
- `nix` - job control

**Project structure:**
```
nosh/
├── Cargo.toml
├── src/
│   ├── main.rs
│   ├── repl/
│   │   ├── mod.rs
│   │   ├── readline.rs
│   │   └── jobs.rs
│   ├── ai/
│   │   ├── mod.rs
│   │   ├── ollama.rs
│   │   ├── cloud.rs
│   │   └── prompt.rs
│   ├── safety/
│   │   ├── mod.rs
│   │   ├── parser.rs
│   │   ├── analyzer.rs
│   │   └── permissions.rs
│   ├── plugins/
│   │   ├── mod.rs
│   │   ├── loader.rs
│   │   ├── builtins.rs
│   │   └── theme.rs
│   ├── config/
│   │   ├── mod.rs
│   │   └── schema.rs
│   ├── auth/
│   │   ├── mod.rs
│   │   └── magic_link.rs
│   └── exec/
│       ├── mod.rs
│       └── runner.rs
├── plugins/
│   ├── git.toml
│   ├── exec_time.toml
│   └── ...
└── server/
    └── ...
```

### 2. Nosh Cloud (TypeScript/Hono/Vercel)

**Structure:**
```
server/
├── src/
│   ├── index.ts
│   ├── routes/
│   │   ├── auth.ts
│   │   ├── ai.ts
│   │   ├── billing.ts
│   │   └── account.ts
│   ├── lib/
│   │   ├── replicate.ts
│   │   ├── stripe.ts
│   │   ├── email.ts
│   │   └── db.ts
│   └── middleware/
│       └── auth.ts
├── vercel.json
└── package.json
```

**Endpoints:**
```
POST /auth/login          # send magic link
GET  /auth/verify?token=  # verify link, return JWT
POST /ai/complete         # proxy to Replicate (authed)
GET  /account             # subscription status
GET  /account/credits     # credit balance
POST /billing/portal      # Stripe customer portal URL
POST /billing/buy-credits # purchase top-up
POST /billing/webhook     # Stripe webhooks
```

**Database (Vercel Postgres):**
- Users: email, stripe_customer_id, created_at
- Subscriptions: user_id, plan, status, current_period_end
- Credits: user_id, balance, monthly_allowance, resets_at

## User Flows

### First Run / Onboarding

```
$ nosh

Welcome to nosh!

How would you like to power your shell?

  [1] Ollama (free, runs locally)
  [2] Nosh Cloud (subscription)
  [q] Quit

>
```

**Ollama path:** Check if Ollama is running, guide through model selection (default: llama3.2).

**Cloud path:** Magic link auth → 7-day trial → ready to use.

### Normal Usage

```
~/projects $ show me all TODO comments in this project
⚡ grep -rn "TODO" --include="*.rs" .

./src/main.rs:42: // TODO: implement job control
./src/ai.rs:15: // TODO: add retry logic
```

The `⚡` indicates an AI-translated command.

### Permission Prompts

When AI generates a command involving IO/destructive operations:

```
~/projects $ delete all log files

nosh wants to run: rm -f **/*.log

  [enter] Allow once
  [a] Always allow "rm" on log files
  [d] Always allow here (~/projects)
  [n] Don't run

>
```

**Smart parsing understands:**
| Command | Risk Level |
|---------|-----------|
| `rm temp.txt` | low |
| `rm *.log` | medium |
| `rm -rf ./target` | medium |
| `rm -rf ~` | critical (requires typing "yes") |
| `rm -rf /` | blocked |
| `curl ... \| sh` | critical |

### Credits Management

```
~/projects $ nosh credits

Credits: 847 / 1000 (resets in 12 days)

  [b] Buy more credits
  [q] Back
```

**Low balance warning:**
```
⚠ Low credits (23 remaining)
  [enter] Continue
  [b] Buy more
```

**Zero credits:**
```
✗ Out of credits.

  [b] Buy more credits
  [o] Switch to Ollama (free, local)
```

## Configuration

**Location:** `~/.config/nosh/`

```
~/.config/nosh/
├── config.toml
├── permissions.toml
├── credentials.toml
└── plugins/
    ├── builtin/
    │   ├── git.toml
    │   ├── exec_time.toml
    │   └── ...
    └── community/
└── themes/
    └── cyberpunk.toml
```

**config.toml:**
```toml
[ai]
backend = "ollama"  # or "cloud"
model = "llama3.2"

[prompt]
theme = "default"

[behavior]
show_translated_command = true
```

**permissions.toml:**
```toml
[directories]
"~/projects" = ["rm:*.log", "git"]
"~/scratch" = ["all"]

[commands]
"git *" = "allow"
"curl *" = "ask"
```

## Plugin System

Plugins provide prompt variables. Even built-ins are plugins.

**Plugin definition (git.toml):**
```toml
[plugin]
name = "git"
description = "Git branch and status"

[provides]
branch = { command = "git branch --show-current 2>/dev/null" }
dirty = { command = "git status --porcelain 2>/dev/null", transform = "non_empty" }

[icons]
dirty = "✗"
clean = "✓"
```

**Theme references plugins:**
```toml
[prompt]
format = "{user}@{host} {cwd_short} {git:branch} {exec_time:duration} λ "

[plugins]
git = { enabled = true, style = "#ffff00" }
exec_time = { enabled = true, min_ms = 1000 }
```

**Available hooks:**
- `directory_change` - when cd happens
- `before_command` - before any command runs
- `after_command` - after command completes
- `on_error` - when command fails

## AI Integration

### Ollama (Local)

- User configures model in config.toml
- Default: llama3.2
- Requests go directly to local Ollama API

### Nosh Cloud

- Credits-based billing
- Subscription gives X credits/month
- Can buy top-up packs
- Proxies to Replicate

### Prompt Engineering

System prompt instructs AI to:
1. Translate natural language to shell commands
2. Recognize existing shell commands and pass through
3. Flag dangerous operations
4. Return structured response (command + explanation)

## Security Considerations

- Credentials stored in `~/.config/nosh/credentials.toml` with restricted permissions
- JWT tokens for cloud auth, stored locally
- Command parsing happens locally (don't trust AI for safety)
- Critical commands always require confirmation
- Some commands are blocked entirely (rm -rf /)
