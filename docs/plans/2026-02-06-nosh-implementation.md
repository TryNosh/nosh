# nosh Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a natural language shell that translates human commands to shell commands using AI.

**Architecture:** Rust CLI client handles REPL, job control, plugins, and safety. TypeScript/Hono server on Vercel handles auth, billing, and Replicate proxy. Local Ollama support for free tier.

**Tech Stack:** Rust (rustyline, tokio, reqwest, crossterm), TypeScript (Hono, Stripe SDK, Resend), Vercel Postgres, Stripe, Replicate.

---

## Phase 1: Minimal Working Shell

Get a shell that can take input, send to Ollama, and execute the result. No safety, no plugins, no cloud - just the core loop.

---

### Task 1: Initialize Rust Project

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `.gitignore`

**Step 1: Create Cargo project**

Run:
```bash
cargo init --name nosh
```

**Step 2: Add initial dependencies to Cargo.toml**

```toml
[package]
name = "nosh"
version = "0.1.0"
edition = "2024"
description = "Natural language shell powered by AI"

[dependencies]
tokio = { version = "1", features = ["full"] }
rustyline = "15"
reqwest = { version = "0.12", features = ["json"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
crossterm = "0.28"
anyhow = "1"
```

**Step 3: Create .gitignore**

```
/target
.DS_Store
```

**Step 4: Write minimal main.rs**

```rust
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    println!("nosh v{}", env!("CARGO_PKG_VERSION"));
    Ok(())
}
```

**Step 5: Verify it compiles and runs**

Run: `cargo run`
Expected: `nosh v0.1.0`

**Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock src/main.rs .gitignore
git commit -m "feat: initialize nosh Rust project"
```

---

### Task 2: Basic REPL Loop

**Files:**
- Modify: `src/main.rs`
- Create: `src/repl/mod.rs`
- Create: `src/repl/readline.rs`

**Step 1: Create repl module structure**

Create `src/repl/mod.rs`:
```rust
mod readline;

pub use readline::Repl;
```

**Step 2: Implement basic REPL**

Create `src/repl/readline.rs`:
```rust
use anyhow::Result;
use rustyline::error::ReadlineError;
use rustyline::{DefaultEditor, Editor};
use std::path::PathBuf;

pub struct Repl {
    editor: DefaultEditor,
    history_path: PathBuf,
}

impl Repl {
    pub fn new() -> Result<Self> {
        let editor = DefaultEditor::new()?;
        let history_path = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("nosh")
            .join("history");

        Ok(Self {
            editor,
            history_path,
        })
    }

    pub fn load_history(&mut self) {
        let _ = self.editor.load_history(&self.history_path);
    }

    pub fn save_history(&mut self) {
        if let Some(parent) = self.history_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = self.editor.save_history(&self.history_path);
    }

    pub fn prompt(&self) -> String {
        let cwd = std::env::current_dir()
            .ok()
            .and_then(|p| p.file_name().map(|s| s.to_string_lossy().to_string()))
            .unwrap_or_else(|| "~".to_string());
        format!("{} $ ", cwd)
    }

    pub fn readline(&mut self) -> Result<Option<String>> {
        let prompt = self.prompt();
        match self.editor.readline(&prompt) {
            Ok(line) => {
                let line = line.trim().to_string();
                if !line.is_empty() {
                    let _ = self.editor.add_history_entry(&line);
                }
                Ok(Some(line))
            }
            Err(ReadlineError::Interrupted) => Ok(None), // Ctrl+C
            Err(ReadlineError::Eof) => Ok(None),         // Ctrl+D
            Err(e) => Err(e.into()),
        }
    }
}
```

**Step 3: Add dirs dependency for config path**

Add to `Cargo.toml` dependencies:
```toml
dirs = "6"
```

**Step 4: Update main.rs to use REPL**

```rust
mod repl;

use anyhow::Result;
use repl::Repl;

#[tokio::main]
async fn main() -> Result<()> {
    println!("nosh v{}", env!("CARGO_PKG_VERSION"));
    println!("Type 'exit' to quit.\n");

    let mut repl = Repl::new()?;
    repl.load_history();

    loop {
        match repl.readline()? {
            Some(line) if line == "exit" || line == "quit" => break,
            Some(line) => {
                println!("You typed: {}", line);
            }
            None => break,
        }
    }

    repl.save_history();
    println!("Goodbye!");
    Ok(())
}
```

**Step 5: Verify REPL works**

Run: `cargo run`
- Type some text, verify it echoes back
- Press up arrow, verify history works
- Type `exit`, verify it quits

**Step 6: Commit**

```bash
git add -A
git commit -m "feat: add basic REPL with history"
```

---

### Task 3: Ollama Client

**Files:**
- Create: `src/ai/mod.rs`
- Create: `src/ai/ollama.rs`

**Step 1: Create ai module structure**

Create `src/ai/mod.rs`:
```rust
mod ollama;

pub use ollama::OllamaClient;
```

**Step 2: Implement Ollama client**

Create `src/ai/ollama.rs`:
```rust
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct GenerateRequest {
    model: String,
    prompt: String,
    stream: bool,
    system: String,
}

#[derive(Deserialize)]
struct GenerateResponse {
    response: String,
}

pub struct OllamaClient {
    client: Client,
    base_url: String,
    model: String,
}

impl OllamaClient {
    pub fn new(model: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: "http://localhost:11434".to_string(),
            model: model.to_string(),
        }
    }

    pub async fn translate(&self, input: &str, cwd: &str) -> Result<String> {
        let system_prompt = format!(
            r#"You are a shell command translator. Convert natural language to shell commands.

Current directory: {}

Rules:
1. Output ONLY the shell command, nothing else
2. No explanations, no markdown, no code blocks
3. If the input is already a valid shell command, output it unchanged
4. Use common Unix commands (ls, grep, find, etc.)
5. For dangerous operations (rm, sudo), still output the command - safety is handled separately

Examples:
- "list all files" -> ls -la
- "show disk usage" -> df -h
- "find all rust files" -> find . -name "*.rs"
- "git status" -> git status"#,
            cwd
        );

        let request = GenerateRequest {
            model: self.model.clone(),
            prompt: input.to_string(),
            stream: false,
            system: system_prompt,
        };

        let response = self
            .client
            .post(format!("{}/api/generate", self.base_url))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Ollama request failed: {}",
                response.status()
            ));
        }

        let result: GenerateResponse = response.json().await?;
        Ok(result.response.trim().to_string())
    }

    pub async fn check_available(&self) -> bool {
        self.client
            .get(format!("{}/api/tags", self.base_url))
            .send()
            .await
            .is_ok()
    }
}
```

**Step 3: Add ai module to main.rs and test**

Update `src/main.rs`:
```rust
mod ai;
mod repl;

use ai::OllamaClient;
use anyhow::Result;
use repl::Repl;

#[tokio::main]
async fn main() -> Result<()> {
    println!("nosh v{}", env!("CARGO_PKG_VERSION"));

    let ollama = OllamaClient::new("llama3.2");

    if !ollama.check_available().await {
        eprintln!("Warning: Ollama not available at localhost:11434");
        eprintln!("Start Ollama or configure a different backend.\n");
    }

    println!("Type 'exit' to quit.\n");

    let mut repl = Repl::new()?;
    repl.load_history();

    loop {
        match repl.readline()? {
            Some(line) if line == "exit" || line == "quit" => break,
            Some(line) => {
                let cwd = std::env::current_dir()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|_| ".".to_string());

                match ollama.translate(&line, &cwd).await {
                    Ok(command) => {
                        println!("⚡ {}", command);
                    }
                    Err(e) => {
                        eprintln!("AI error: {}", e);
                    }
                }
            }
            None => break,
        }
    }

    repl.save_history();
    println!("Goodbye!");
    Ok(())
}
```

**Step 4: Test with Ollama**

Run: `cargo run`
- Type "list all files" - should see `⚡ ls -la` (or similar)
- Type "show current directory" - should see `⚡ pwd`

**Step 5: Commit**

```bash
git add -A
git commit -m "feat: add Ollama client for command translation"
```

---

### Task 4: Command Execution

**Files:**
- Create: `src/exec/mod.rs`
- Create: `src/exec/runner.rs`
- Modify: `src/main.rs`

**Step 1: Create exec module structure**

Create `src/exec/mod.rs`:
```rust
mod runner;

pub use runner::execute_command;
```

**Step 2: Implement command execution**

Create `src/exec/runner.rs`:
```rust
use anyhow::Result;
use std::process::{Command, Stdio};

pub fn execute_command(command: &str) -> Result<()> {
    let status = Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;

    if !status.success() {
        if let Some(code) = status.code() {
            eprintln!("Command exited with code {}", code);
        }
    }

    Ok(())
}
```

**Step 3: Integrate execution into main loop**

Update the main loop in `src/main.rs`:
```rust
mod ai;
mod exec;
mod repl;

use ai::OllamaClient;
use anyhow::Result;
use exec::execute_command;
use repl::Repl;

#[tokio::main]
async fn main() -> Result<()> {
    println!("nosh v{}", env!("CARGO_PKG_VERSION"));

    let ollama = OllamaClient::new("llama3.2");

    if !ollama.check_available().await {
        eprintln!("Warning: Ollama not available at localhost:11434");
        eprintln!("Start Ollama or configure a different backend.\n");
    }

    println!("Type 'exit' to quit.\n");

    let mut repl = Repl::new()?;
    repl.load_history();

    loop {
        match repl.readline()? {
            Some(line) if line == "exit" || line == "quit" => break,
            Some(line) => {
                let cwd = std::env::current_dir()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|_| ".".to_string());

                match ollama.translate(&line, &cwd).await {
                    Ok(command) => {
                        println!("⚡ {}", command);
                        if let Err(e) = execute_command(&command) {
                            eprintln!("Execution error: {}", e);
                        }
                    }
                    Err(e) => {
                        eprintln!("AI error: {}", e);
                    }
                }
            }
            None => break,
        }
    }

    repl.save_history();
    println!("Goodbye!");
    Ok(())
}
```

**Step 4: Test full loop**

Run: `cargo run`
- Type "list all files" - should see the command AND its output
- Type "show current directory" - should see pwd output
- Type "echo hello world" - should see "hello world"

**Step 5: Commit**

```bash
git add -A
git commit -m "feat: add command execution"
```

---

## Phase 2: Safety System

Add command parsing and permission prompts before dangerous operations.

---

### Task 5: Shell Command Parser

**Files:**
- Create: `src/safety/mod.rs`
- Create: `src/safety/parser.rs`
- Add to `Cargo.toml`: `shell-words = "1"`

**Step 1: Add shell-words dependency**

Add to `Cargo.toml`:
```toml
shell-words = "1"
```

**Step 2: Create safety module structure**

Create `src/safety/mod.rs`:
```rust
mod parser;

pub use parser::{CommandInfo, ParsedCommand, RiskLevel};
```

**Step 3: Implement command parser**

Create `src/safety/parser.rs`:
```rust
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub enum RiskLevel {
    Safe,       // echo, pwd, ls (no writes)
    Low,        // single file write, git operations
    Medium,     // glob deletes, recursive operations
    High,       // sudo, system modifications
    Critical,   // rm -rf ~, rm -rf /, curl | sh
    Blocked,    // absolutely never allow
}

#[derive(Debug, Clone)]
pub struct CommandInfo {
    pub command: String,
    pub args: Vec<String>,
    pub is_destructive: bool,
    pub is_network: bool,
    pub is_privileged: bool,
    pub affected_paths: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ParsedCommand {
    pub raw: String,
    pub info: CommandInfo,
    pub risk_level: RiskLevel,
    pub risk_reason: String,
}

const DESTRUCTIVE_COMMANDS: &[&str] = &["rm", "rmdir", "mv", "unlink"];
const NETWORK_COMMANDS: &[&str] = &["curl", "wget", "ssh", "scp", "rsync", "nc", "netcat"];
const PRIVILEGED_COMMANDS: &[&str] = &["sudo", "su", "doas"];
const SAFE_COMMANDS: &[&str] = &[
    "echo", "pwd", "ls", "cat", "head", "tail", "grep", "find", "which", "whereis",
    "whoami", "date", "cal", "uptime", "hostname", "uname", "env", "printenv",
    "wc", "sort", "uniq", "diff", "less", "more", "file", "stat", "tree",
];

pub fn parse_command(raw: &str) -> ParsedCommand {
    let words = shell_words::split(raw).unwrap_or_else(|_| vec![raw.to_string()]);

    let (command, args) = if words.is_empty() {
        (String::new(), vec![])
    } else {
        (words[0].clone(), words[1..].to_vec())
    };

    let is_destructive = DESTRUCTIVE_COMMANDS.contains(&command.as_str());
    let is_network = NETWORK_COMMANDS.contains(&command.as_str());
    let is_privileged = PRIVILEGED_COMMANDS.contains(&command.as_str());

    let affected_paths: Vec<String> = args
        .iter()
        .filter(|a| !a.starts_with('-'))
        .filter(|a| Path::new(a).exists() || a.contains('*') || a.contains('/'))
        .cloned()
        .collect();

    let info = CommandInfo {
        command: command.clone(),
        args: args.clone(),
        is_destructive,
        is_network,
        is_privileged,
        affected_paths,
    };

    let (risk_level, risk_reason) = assess_risk(&command, &args, &info);

    ParsedCommand {
        raw: raw.to_string(),
        info,
        risk_level,
        risk_reason,
    }
}

fn assess_risk(command: &str, args: &[String], info: &CommandInfo) -> (RiskLevel, String) {
    // Check for blocked patterns
    if is_blocked(command, args) {
        return (RiskLevel::Blocked, "This command is blocked for safety".to_string());
    }

    // Check for critical patterns
    if let Some(reason) = is_critical(command, args) {
        return (RiskLevel::Critical, reason);
    }

    // Privileged commands
    if info.is_privileged {
        return (RiskLevel::High, format!("Requires elevated privileges ({})", command));
    }

    // Network commands
    if info.is_network {
        // curl/wget piped to sh is critical
        return (RiskLevel::Medium, format!("Network operation ({})", command));
    }

    // Destructive commands
    if info.is_destructive {
        let has_recursive = args.iter().any(|a| a.contains('r') && a.starts_with('-'));
        let has_force = args.iter().any(|a| a.contains('f') && a.starts_with('-'));

        if has_recursive && has_force {
            return (RiskLevel::Medium, "Recursive forced delete".to_string());
        }
        if has_recursive {
            return (RiskLevel::Medium, "Recursive delete".to_string());
        }
        return (RiskLevel::Low, "File deletion".to_string());
    }

    // Safe commands
    if SAFE_COMMANDS.contains(&command.as_str()) {
        return (RiskLevel::Safe, "Read-only operation".to_string());
    }

    // Default to low risk for unknown commands
    (RiskLevel::Low, "Unknown command".to_string())
}

fn is_blocked(command: &str, args: &[String]) -> bool {
    // rm -rf / or rm -rf /*
    if command == "rm" {
        let has_rf = args.iter().any(|a| a.starts_with('-') && a.contains('r') && a.contains('f'));
        let targets_root = args.iter().any(|a| a == "/" || a == "/*");
        if has_rf && targets_root {
            return true;
        }
    }
    false
}

fn is_critical(command: &str, args: &[String]) -> Option<String> {
    // rm -rf on home or other dangerous paths
    if command == "rm" {
        let has_rf = args.iter().any(|a| a.starts_with('-') && a.contains('r') && a.contains('f'));
        if has_rf {
            for arg in args {
                if arg == "~" || arg == "$HOME" || arg.starts_with("~/") && arg.len() <= 3 {
                    return Some("Recursive delete on home directory".to_string());
                }
            }
        }
    }

    // curl/wget piped to sh
    // This would need to check the full command with pipes, handled elsewhere

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_command() {
        let parsed = parse_command("ls -la");
        assert_eq!(parsed.risk_level, RiskLevel::Safe);
    }

    #[test]
    fn test_rm_single_file() {
        let parsed = parse_command("rm temp.txt");
        assert_eq!(parsed.risk_level, RiskLevel::Low);
    }

    #[test]
    fn test_rm_rf() {
        let parsed = parse_command("rm -rf ./target");
        assert_eq!(parsed.risk_level, RiskLevel::Medium);
    }

    #[test]
    fn test_blocked_rm_rf_root() {
        let parsed = parse_command("rm -rf /");
        assert_eq!(parsed.risk_level, RiskLevel::Blocked);
    }
}
```

**Step 4: Run tests**

Run: `cargo test`
Expected: All tests pass

**Step 5: Commit**

```bash
git add -A
git commit -m "feat: add shell command parser with risk assessment"
```

---

### Task 6: Permission Prompts

**Files:**
- Create: `src/safety/prompt.rs`
- Modify: `src/safety/mod.rs`
- Modify: `src/main.rs`

**Step 1: Create permission prompt module**

Add to `src/safety/mod.rs`:
```rust
mod parser;
mod prompt;

pub use parser::{parse_command, CommandInfo, ParsedCommand, RiskLevel};
pub use prompt::{prompt_for_permission, PermissionChoice};
```

**Step 2: Implement permission prompts**

Create `src/safety/prompt.rs`:
```rust
use crate::safety::{ParsedCommand, RiskLevel};
use crossterm::style::{Color, Print, ResetColor, SetForegroundColor};
use crossterm::ExecutableCommand;
use std::io::{self, Write};

#[derive(Debug, Clone, PartialEq)]
pub enum PermissionChoice {
    AllowOnce,
    AllowCommand,  // Allow this command pattern
    AllowHere,     // Allow all in this directory
    Deny,
}

pub fn prompt_for_permission(parsed: &ParsedCommand) -> io::Result<PermissionChoice> {
    let mut stdout = io::stdout();

    // Print warning
    stdout.execute(SetForegroundColor(Color::Yellow))?;
    writeln!(stdout, "\nnosh wants to run: {}", parsed.raw)?;
    stdout.execute(ResetColor)?;

    writeln!(stdout, "Risk: {} - {}",
        match parsed.risk_level {
            RiskLevel::Safe => "safe",
            RiskLevel::Low => "low",
            RiskLevel::Medium => "medium",
            RiskLevel::High => "high",
            RiskLevel::Critical => "CRITICAL",
            RiskLevel::Blocked => "BLOCKED",
        },
        parsed.risk_reason
    )?;

    writeln!(stdout)?;
    writeln!(stdout, "  [enter] Allow once")?;
    writeln!(stdout, "  [a] Always allow \"{}\" commands", parsed.info.command)?;
    writeln!(stdout, "  [d] Always allow here (this directory)")?;
    writeln!(stdout, "  [n] Don't run")?;
    writeln!(stdout)?;
    write!(stdout, "> ")?;
    stdout.flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim().to_lowercase();

    Ok(match input.as_str() {
        "" => PermissionChoice::AllowOnce,
        "a" => PermissionChoice::AllowCommand,
        "d" => PermissionChoice::AllowHere,
        "n" | "no" => PermissionChoice::Deny,
        _ => PermissionChoice::Deny, // Default to deny on unknown input
    })
}

pub fn print_blocked(parsed: &ParsedCommand) -> io::Result<()> {
    let mut stdout = io::stdout();
    stdout.execute(SetForegroundColor(Color::Red))?;
    writeln!(stdout, "\n✗ Command blocked: {}", parsed.raw)?;
    stdout.execute(ResetColor)?;
    writeln!(stdout, "Reason: {}", parsed.risk_reason)?;
    Ok(())
}

pub fn print_critical_warning(parsed: &ParsedCommand) -> io::Result<bool> {
    let mut stdout = io::stdout();
    stdout.execute(SetForegroundColor(Color::Red))?;
    writeln!(stdout, "\n⚠ CRITICAL: {}", parsed.raw)?;
    stdout.execute(ResetColor)?;
    writeln!(stdout, "Reason: {}", parsed.risk_reason)?;
    writeln!(stdout)?;
    write!(stdout, "Type 'yes' to proceed: ")?;
    stdout.flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    Ok(input.trim().to_lowercase() == "yes")
}
```

**Step 3: Integrate safety checks into main**

Update `src/main.rs`:
```rust
mod ai;
mod exec;
mod repl;
mod safety;

use ai::OllamaClient;
use anyhow::Result;
use exec::execute_command;
use repl::Repl;
use safety::{parse_command, prompt_for_permission, PermissionChoice, RiskLevel};

#[tokio::main]
async fn main() -> Result<()> {
    println!("nosh v{}", env!("CARGO_PKG_VERSION"));

    let ollama = OllamaClient::new("llama3.2");

    if !ollama.check_available().await {
        eprintln!("Warning: Ollama not available at localhost:11434");
        eprintln!("Start Ollama or configure a different backend.\n");
    }

    println!("Type 'exit' to quit.\n");

    let mut repl = Repl::new()?;
    repl.load_history();

    loop {
        match repl.readline()? {
            Some(line) if line == "exit" || line == "quit" => break,
            Some(line) => {
                let cwd = std::env::current_dir()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|_| ".".to_string());

                match ollama.translate(&line, &cwd).await {
                    Ok(command) => {
                        println!("⚡ {}", command);

                        // Parse and check safety
                        let parsed = parse_command(&command);

                        let should_execute = match parsed.risk_level {
                            RiskLevel::Safe => true,
                            RiskLevel::Blocked => {
                                safety::prompt::print_blocked(&parsed)?;
                                false
                            }
                            RiskLevel::Critical => {
                                safety::prompt::print_critical_warning(&parsed)?
                            }
                            _ => {
                                match prompt_for_permission(&parsed)? {
                                    PermissionChoice::AllowOnce => true,
                                    PermissionChoice::AllowCommand => {
                                        // TODO: persist this
                                        true
                                    }
                                    PermissionChoice::AllowHere => {
                                        // TODO: persist this
                                        true
                                    }
                                    PermissionChoice::Deny => false,
                                }
                            }
                        };

                        if should_execute {
                            if let Err(e) = execute_command(&command) {
                                eprintln!("Execution error: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("AI error: {}", e);
                    }
                }
            }
            None => break,
        }
    }

    repl.save_history();
    println!("Goodbye!");
    Ok(())
}
```

**Step 4: Make prompt module public**

Update `src/safety/mod.rs`:
```rust
mod parser;
pub mod prompt;

pub use parser::{parse_command, CommandInfo, ParsedCommand, RiskLevel};
pub use prompt::{prompt_for_permission, PermissionChoice};
```

**Step 5: Test safety prompts**

Run: `cargo run`
- Type "list all files" - should execute without prompt
- Type "delete all log files" - should show permission prompt
- Type "delete everything recursively" - should show CRITICAL warning

**Step 6: Commit**

```bash
git add -A
git commit -m "feat: add safety prompts for dangerous commands"
```

---

### Task 7: Permission Persistence

**Files:**
- Create: `src/safety/permissions.rs`
- Modify: `src/safety/mod.rs`
- Modify: `src/main.rs`
- Add to `Cargo.toml`: `toml = "0.8"`

**Step 1: Add toml dependency**

Add to `Cargo.toml`:
```toml
toml = "0.8"
```

**Step 2: Create permissions module**

Add to `src/safety/mod.rs`:
```rust
mod parser;
mod permissions;
pub mod prompt;

pub use parser::{parse_command, CommandInfo, ParsedCommand, RiskLevel};
pub use permissions::PermissionStore;
pub use prompt::{prompt_for_permission, PermissionChoice};
```

**Step 3: Implement permission persistence**

Create `src/safety/permissions.rs`:
```rust
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct PermissionStore {
    /// Commands that are always allowed (e.g., "rm", "git")
    #[serde(default)]
    pub allowed_commands: HashSet<String>,

    /// Directories where all operations are allowed
    #[serde(default)]
    pub allowed_directories: HashSet<String>,

    /// Session-only allowed commands (not persisted)
    #[serde(skip)]
    session_commands: HashSet<String>,

    /// Session-only allowed directories (not persisted)
    #[serde(skip)]
    session_directories: HashSet<String>,

    #[serde(skip)]
    path: PathBuf,
}

impl PermissionStore {
    pub fn load() -> Result<Self> {
        let path = Self::config_path();

        if path.exists() {
            let content = fs::read_to_string(&path)?;
            let mut store: PermissionStore = toml::from_str(&content)?;
            store.path = path;
            Ok(store)
        } else {
            Ok(Self {
                path,
                ..Default::default()
            })
        }
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        fs::write(&self.path, content)?;
        Ok(())
    }

    fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("nosh")
            .join("permissions.toml")
    }

    pub fn is_command_allowed(&self, command: &str) -> bool {
        self.allowed_commands.contains(command) || self.session_commands.contains(command)
    }

    pub fn is_directory_allowed(&self, directory: &str) -> bool {
        // Check if this directory or any parent is allowed
        let dir_path = PathBuf::from(directory);

        for allowed in self.allowed_directories.iter().chain(self.session_directories.iter()) {
            let allowed_path = PathBuf::from(allowed);
            if dir_path.starts_with(&allowed_path) {
                return true;
            }
        }
        false
    }

    pub fn allow_command(&mut self, command: &str, persist: bool) {
        if persist {
            self.allowed_commands.insert(command.to_string());
            let _ = self.save();
        } else {
            self.session_commands.insert(command.to_string());
        }
    }

    pub fn allow_directory(&mut self, directory: &str, persist: bool) {
        if persist {
            self.allowed_directories.insert(directory.to_string());
            let _ = self.save();
        } else {
            self.session_directories.insert(directory.to_string());
        }
    }
}
```

**Step 4: Integrate permissions into main**

Update `src/main.rs`:
```rust
mod ai;
mod exec;
mod repl;
mod safety;

use ai::OllamaClient;
use anyhow::Result;
use exec::execute_command;
use repl::Repl;
use safety::{parse_command, prompt_for_permission, PermissionChoice, PermissionStore, RiskLevel};

#[tokio::main]
async fn main() -> Result<()> {
    println!("nosh v{}", env!("CARGO_PKG_VERSION"));

    let ollama = OllamaClient::new("llama3.2");
    let mut permissions = PermissionStore::load().unwrap_or_default();

    if !ollama.check_available().await {
        eprintln!("Warning: Ollama not available at localhost:11434");
        eprintln!("Start Ollama or configure a different backend.\n");
    }

    println!("Type 'exit' to quit.\n");

    let mut repl = Repl::new()?;
    repl.load_history();

    loop {
        let cwd = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| ".".to_string());

        match repl.readline()? {
            Some(line) if line == "exit" || line == "quit" => break,
            Some(line) => {
                match ollama.translate(&line, &cwd).await {
                    Ok(command) => {
                        println!("⚡ {}", command);

                        let parsed = parse_command(&command);

                        let should_execute = match parsed.risk_level {
                            RiskLevel::Safe => true,
                            RiskLevel::Blocked => {
                                safety::prompt::print_blocked(&parsed)?;
                                false
                            }
                            RiskLevel::Critical => {
                                safety::prompt::print_critical_warning(&parsed)?
                            }
                            _ => {
                                // Check if already permitted
                                if permissions.is_command_allowed(&parsed.info.command) {
                                    true
                                } else if permissions.is_directory_allowed(&cwd) {
                                    true
                                } else {
                                    match prompt_for_permission(&parsed)? {
                                        PermissionChoice::AllowOnce => true,
                                        PermissionChoice::AllowCommand => {
                                            permissions.allow_command(&parsed.info.command, true);
                                            true
                                        }
                                        PermissionChoice::AllowHere => {
                                            permissions.allow_directory(&cwd, true);
                                            true
                                        }
                                        PermissionChoice::Deny => false,
                                    }
                                }
                            }
                        };

                        if should_execute {
                            if let Err(e) = execute_command(&command) {
                                eprintln!("Execution error: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("AI error: {}", e);
                    }
                }
            }
            None => break,
        }
    }

    repl.save_history();
    println!("Goodbye!");
    Ok(())
}
```

**Step 5: Test permission persistence**

Run: `cargo run`
- Trigger a permission prompt, choose [a]
- Exit and re-run
- Same command should not prompt again
- Check `~/.config/nosh/permissions.toml` exists

**Step 6: Commit**

```bash
git add -A
git commit -m "feat: add permission persistence"
```

---

## Phase 3: Configuration System

Add TOML config for AI backend, model, and behavior settings.

---

### Task 8: Config Schema

**Files:**
- Create: `src/config/mod.rs`
- Create: `src/config/schema.rs`
- Modify: `src/main.rs`

**Step 1: Create config module structure**

Create `src/config/mod.rs`:
```rust
mod schema;

pub use schema::Config;
```

**Step 2: Implement config schema**

Create `src/config/schema.rs`:
```rust
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub ai: AiConfig,
    pub behavior: BehaviorConfig,
    pub prompt: PromptConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AiConfig {
    /// AI backend: "ollama" or "cloud"
    pub backend: String,
    /// Model name for Ollama
    pub model: String,
    /// Ollama API base URL
    pub ollama_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BehaviorConfig {
    /// Show the translated command before running
    pub show_command: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PromptConfig {
    /// Theme name
    pub theme: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ai: AiConfig::default(),
            behavior: BehaviorConfig::default(),
            prompt: PromptConfig::default(),
        }
    }
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            backend: "ollama".to_string(),
            model: "llama3.2".to_string(),
            ollama_url: "http://localhost:11434".to_string(),
        }
    }
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        Self {
            show_command: true,
        }
    }
}

impl Default for PromptConfig {
    fn default() -> Self {
        Self {
            theme: "default".to_string(),
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = Self::config_path();

        if path.exists() {
            let content = fs::read_to_string(&path)?;
            let config: Config = toml::from_str(&content)?;
            Ok(config)
        } else {
            let config = Config::default();
            config.save()?;
            Ok(config)
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        fs::write(&path, content)?;
        Ok(())
    }

    fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("nosh")
            .join("config.toml")
    }
}
```

**Step 3: Use config in main**

Update `src/main.rs`:
```rust
mod ai;
mod config;
mod exec;
mod repl;
mod safety;

use ai::OllamaClient;
use anyhow::Result;
use config::Config;
use exec::execute_command;
use repl::Repl;
use safety::{parse_command, prompt_for_permission, PermissionChoice, PermissionStore, RiskLevel};

#[tokio::main]
async fn main() -> Result<()> {
    println!("nosh v{}", env!("CARGO_PKG_VERSION"));

    let config = Config::load().unwrap_or_default();
    let ollama = OllamaClient::new(&config.ai.model);
    let mut permissions = PermissionStore::load().unwrap_or_default();

    if config.ai.backend == "ollama" && !ollama.check_available().await {
        eprintln!("Warning: Ollama not available at localhost:11434");
        eprintln!("Start Ollama or configure a different backend.\n");
    }

    println!("Type 'exit' to quit.\n");

    let mut repl = Repl::new()?;
    repl.load_history();

    loop {
        let cwd = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| ".".to_string());

        match repl.readline()? {
            Some(line) if line == "exit" || line == "quit" => break,
            Some(line) => {
                match ollama.translate(&line, &cwd).await {
                    Ok(command) => {
                        if config.behavior.show_command {
                            println!("⚡ {}", command);
                        }

                        let parsed = parse_command(&command);

                        let should_execute = match parsed.risk_level {
                            RiskLevel::Safe => true,
                            RiskLevel::Blocked => {
                                safety::prompt::print_blocked(&parsed)?;
                                false
                            }
                            RiskLevel::Critical => {
                                safety::prompt::print_critical_warning(&parsed)?
                            }
                            _ => {
                                if permissions.is_command_allowed(&parsed.info.command) {
                                    true
                                } else if permissions.is_directory_allowed(&cwd) {
                                    true
                                } else {
                                    match prompt_for_permission(&parsed)? {
                                        PermissionChoice::AllowOnce => true,
                                        PermissionChoice::AllowCommand => {
                                            permissions.allow_command(&parsed.info.command, true);
                                            true
                                        }
                                        PermissionChoice::AllowHere => {
                                            permissions.allow_directory(&cwd, true);
                                            true
                                        }
                                        PermissionChoice::Deny => false,
                                    }
                                }
                            }
                        };

                        if should_execute {
                            if let Err(e) = execute_command(&command) {
                                eprintln!("Execution error: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("AI error: {}", e);
                    }
                }
            }
            None => break,
        }
    }

    repl.save_history();
    println!("Goodbye!");
    Ok(())
}
```

**Step 4: Test config**

Run: `cargo run`
- Check `~/.config/nosh/config.toml` was created
- Modify the model name, restart, verify it's used

**Step 5: Commit**

```bash
git add -A
git commit -m "feat: add TOML configuration system"
```

---

## Phase 4: Server Setup

Set up the TypeScript/Hono server project for Nosh Cloud.

---

### Task 9: Initialize Server Project

**Files:**
- Create: `server/package.json`
- Create: `server/tsconfig.json`
- Create: `server/src/index.ts`
- Create: `server/vercel.json`
- Create: `server/.gitignore`

**Step 1: Create server directory structure**

Run:
```bash
mkdir -p server/src/routes server/src/lib server/src/middleware
```

**Step 2: Create package.json**

Create `server/package.json`:
```json
{
  "name": "nosh-cloud",
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vercel dev",
    "build": "tsc",
    "typecheck": "tsc --noEmit"
  },
  "dependencies": {
    "@hono/node-server": "^1.13.7",
    "hono": "^4.6.17",
    "replicate": "^1.0.1",
    "resend": "^4.0.1",
    "stripe": "^17.5.0",
    "@vercel/postgres": "^0.10.0",
    "jose": "^5.9.6"
  },
  "devDependencies": {
    "@types/node": "^22.10.5",
    "typescript": "^5.7.2",
    "vercel": "^39.3.0"
  }
}
```

**Step 3: Create tsconfig.json**

Create `server/tsconfig.json`:
```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "outDir": "dist",
    "rootDir": "src",
    "declaration": true
  },
  "include": ["src/**/*"],
  "exclude": ["node_modules", "dist"]
}
```

**Step 4: Create vercel.json**

Create `server/vercel.json`:
```json
{
  "rewrites": [
    { "source": "/(.*)", "destination": "/api" }
  ]
}
```

**Step 5: Create main entry point**

Create `server/src/index.ts`:
```typescript
import { Hono } from 'hono';
import { handle } from 'hono/vercel';

const app = new Hono().basePath('/api');

app.get('/', (c) => {
  return c.json({
    name: 'nosh-cloud',
    version: '0.1.0',
    status: 'ok',
  });
});

app.get('/health', (c) => {
  return c.json({ status: 'healthy' });
});

export default handle(app);
```

**Step 6: Create Vercel API route**

Create `server/api/index.ts`:
```typescript
export { default } from '../src/index';
```

**Step 7: Create .gitignore**

Create `server/.gitignore`:
```
node_modules
dist
.vercel
.env
.env.local
```

**Step 8: Install dependencies and test**

Run:
```bash
cd server && npm install && npm run typecheck
```

**Step 9: Commit**

```bash
git add -A
git commit -m "feat: initialize Nosh Cloud server project"
```

---

### Task 10: Auth Routes (Magic Link)

**Files:**
- Create: `server/src/routes/auth.ts`
- Create: `server/src/lib/email.ts`
- Create: `server/src/lib/jwt.ts`
- Create: `server/src/lib/db.ts`
- Modify: `server/src/index.ts`

**Step 1: Create JWT utilities**

Create `server/src/lib/jwt.ts`:
```typescript
import * as jose from 'jose';

const JWT_SECRET = new TextEncoder().encode(
  process.env.JWT_SECRET || 'dev-secret-change-in-production'
);

export interface TokenPayload {
  userId: string;
  email: string;
}

export async function createToken(payload: TokenPayload): Promise<string> {
  return await new jose.SignJWT(payload)
    .setProtectedHeader({ alg: 'HS256' })
    .setIssuedAt()
    .setExpirationTime('30d')
    .sign(JWT_SECRET);
}

export async function verifyToken(token: string): Promise<TokenPayload | null> {
  try {
    const { payload } = await jose.jwtVerify(token, JWT_SECRET);
    return payload as unknown as TokenPayload;
  } catch {
    return null;
  }
}

export function createMagicLinkToken(email: string): Promise<string> {
  return new jose.SignJWT({ email })
    .setProtectedHeader({ alg: 'HS256' })
    .setIssuedAt()
    .setExpirationTime('15m')
    .sign(JWT_SECRET);
}

export async function verifyMagicLinkToken(token: string): Promise<string | null> {
  try {
    const { payload } = await jose.jwtVerify(token, JWT_SECRET);
    return (payload as { email?: string }).email || null;
  } catch {
    return null;
  }
}
```

**Step 2: Create email utilities**

Create `server/src/lib/email.ts`:
```typescript
import { Resend } from 'resend';

const resend = new Resend(process.env.RESEND_API_KEY);

export async function sendMagicLink(email: string, token: string): Promise<void> {
  const baseUrl = process.env.VERCEL_URL
    ? `https://${process.env.VERCEL_URL}`
    : 'http://localhost:3000';

  const magicLink = `${baseUrl}/api/auth/verify?token=${token}`;

  await resend.emails.send({
    from: 'nosh <noreply@nosh.sh>',
    to: email,
    subject: 'Sign in to nosh',
    html: `
      <h1>Sign in to nosh</h1>
      <p>Click the link below to sign in:</p>
      <a href="${magicLink}">Sign in to nosh</a>
      <p>This link expires in 15 minutes.</p>
      <p>If you didn't request this, you can safely ignore this email.</p>
    `,
  });
}
```

**Step 3: Create database utilities**

Create `server/src/lib/db.ts`:
```typescript
import { sql } from '@vercel/postgres';

export interface User {
  id: string;
  email: string;
  stripe_customer_id: string | null;
  created_at: Date;
}

export interface Credits {
  user_id: string;
  balance: number;
  monthly_allowance: number;
  resets_at: Date;
}

export async function findUserByEmail(email: string): Promise<User | null> {
  const result = await sql<User>`
    SELECT * FROM users WHERE email = ${email} LIMIT 1
  `;
  return result.rows[0] || null;
}

export async function createUser(email: string): Promise<User> {
  const result = await sql<User>`
    INSERT INTO users (email) VALUES (${email})
    RETURNING *
  `;
  return result.rows[0];
}

export async function findOrCreateUser(email: string): Promise<User> {
  const existing = await findUserByEmail(email);
  if (existing) return existing;
  return createUser(email);
}

export async function getUserCredits(userId: string): Promise<Credits | null> {
  const result = await sql<Credits>`
    SELECT * FROM credits WHERE user_id = ${userId} LIMIT 1
  `;
  return result.rows[0] || null;
}

export async function decrementCredits(userId: string): Promise<number> {
  const result = await sql<{ balance: number }>`
    UPDATE credits SET balance = balance - 1
    WHERE user_id = ${userId} AND balance > 0
    RETURNING balance
  `;
  return result.rows[0]?.balance ?? 0;
}
```

**Step 4: Create auth routes**

Create `server/src/routes/auth.ts`:
```typescript
import { Hono } from 'hono';
import { createMagicLinkToken, verifyMagicLinkToken, createToken } from '../lib/jwt';
import { sendMagicLink } from '../lib/email';
import { findOrCreateUser } from '../lib/db';

const auth = new Hono();

// POST /auth/login - Send magic link
auth.post('/login', async (c) => {
  const body = await c.req.json<{ email: string }>();
  const { email } = body;

  if (!email || !email.includes('@')) {
    return c.json({ error: 'Valid email required' }, 400);
  }

  const token = await createMagicLinkToken(email);

  try {
    await sendMagicLink(email, token);
    return c.json({ success: true, message: 'Magic link sent' });
  } catch (error) {
    console.error('Failed to send magic link:', error);
    return c.json({ error: 'Failed to send email' }, 500);
  }
});

// GET /auth/verify - Verify magic link and return JWT
auth.get('/verify', async (c) => {
  const token = c.req.query('token');

  if (!token) {
    return c.json({ error: 'Token required' }, 400);
  }

  const email = await verifyMagicLinkToken(token);

  if (!email) {
    return c.json({ error: 'Invalid or expired token' }, 401);
  }

  const user = await findOrCreateUser(email);
  const jwt = await createToken({ userId: user.id, email: user.email });

  // Return HTML that passes JWT back to CLI
  return c.html(`
    <!DOCTYPE html>
    <html>
      <head><title>nosh - Signed In</title></head>
      <body>
        <h1>Success!</h1>
        <p>You're now signed in to nosh.</p>
        <p>You can close this window and return to your terminal.</p>
        <pre style="background:#f0f0f0;padding:1em;border-radius:4px;">
Token: ${jwt}
        </pre>
        <p><small>Copy this token if the CLI didn't receive it automatically.</small></p>
      </body>
    </html>
  `);
});

export default auth;
```

**Step 5: Mount auth routes**

Update `server/src/index.ts`:
```typescript
import { Hono } from 'hono';
import { handle } from 'hono/vercel';
import auth from './routes/auth';

const app = new Hono().basePath('/api');

app.get('/', (c) => {
  return c.json({
    name: 'nosh-cloud',
    version: '0.1.0',
    status: 'ok',
  });
});

app.get('/health', (c) => {
  return c.json({ status: 'healthy' });
});

app.route('/auth', auth);

export default handle(app);
```

**Step 6: Typecheck**

Run: `cd server && npm run typecheck`

**Step 7: Commit**

```bash
git add -A
git commit -m "feat: add magic link authentication routes"
```

---

### Task 11: AI Proxy Route

**Files:**
- Create: `server/src/routes/ai.ts`
- Create: `server/src/middleware/auth.ts`
- Modify: `server/src/index.ts`

**Step 1: Create auth middleware**

Create `server/src/middleware/auth.ts`:
```typescript
import { Context, Next } from 'hono';
import { verifyToken, TokenPayload } from '../lib/jwt';

declare module 'hono' {
  interface ContextVariableMap {
    user: TokenPayload;
  }
}

export async function authMiddleware(c: Context, next: Next) {
  const authHeader = c.req.header('Authorization');

  if (!authHeader?.startsWith('Bearer ')) {
    return c.json({ error: 'Unauthorized' }, 401);
  }

  const token = authHeader.slice(7);
  const payload = await verifyToken(token);

  if (!payload) {
    return c.json({ error: 'Invalid token' }, 401);
  }

  c.set('user', payload);
  await next();
}
```

**Step 2: Create AI proxy route**

Create `server/src/routes/ai.ts`:
```typescript
import { Hono } from 'hono';
import Replicate from 'replicate';
import { authMiddleware } from '../middleware/auth';
import { getUserCredits, decrementCredits } from '../lib/db';

const ai = new Hono();
const replicate = new Replicate();

// All AI routes require authentication
ai.use('*', authMiddleware);

// POST /ai/complete - Translate natural language to shell command
ai.post('/complete', async (c) => {
  const user = c.get('user');
  const body = await c.req.json<{ input: string; cwd: string }>();
  const { input, cwd } = body;

  if (!input) {
    return c.json({ error: 'Input required' }, 400);
  }

  // Check credits
  const credits = await getUserCredits(user.userId);
  if (!credits || credits.balance <= 0) {
    return c.json({ error: 'No credits remaining', code: 'NO_CREDITS' }, 402);
  }

  const systemPrompt = `You are a shell command translator. Convert natural language to shell commands.

Current directory: ${cwd || '.'}

Rules:
1. Output ONLY the shell command, nothing else
2. No explanations, no markdown, no code blocks
3. If the input is already a valid shell command, output it unchanged
4. Use common Unix commands (ls, grep, find, etc.)

Examples:
- "list all files" -> ls -la
- "show disk usage" -> df -h
- "find all rust files" -> find . -name "*.rs"`;

  try {
    const output = await replicate.run('meta/meta-llama-3-8b-instruct', {
      input: {
        prompt: `${systemPrompt}\n\nUser: ${input}\nCommand:`,
        max_tokens: 200,
      },
    });

    // Decrement credits after successful request
    const newBalance = await decrementCredits(user.userId);

    const command = Array.isArray(output) ? output.join('').trim() : String(output).trim();

    return c.json({
      command,
      credits_remaining: newBalance,
    });
  } catch (error) {
    console.error('Replicate error:', error);
    return c.json({ error: 'AI request failed' }, 500);
  }
});

export default ai;
```

**Step 3: Mount AI routes**

Update `server/src/index.ts`:
```typescript
import { Hono } from 'hono';
import { handle } from 'hono/vercel';
import auth from './routes/auth';
import ai from './routes/ai';

const app = new Hono().basePath('/api');

app.get('/', (c) => {
  return c.json({
    name: 'nosh-cloud',
    version: '0.1.0',
    status: 'ok',
  });
});

app.get('/health', (c) => {
  return c.json({ status: 'healthy' });
});

app.route('/auth', auth);
app.route('/ai', ai);

export default handle(app);
```

**Step 4: Typecheck**

Run: `cd server && npm run typecheck`

**Step 5: Commit**

```bash
git add -A
git commit -m "feat: add AI proxy route with auth and credits"
```

---

### Task 12: Billing Routes (Stripe)

**Files:**
- Create: `server/src/routes/billing.ts`
- Create: `server/src/lib/stripe.ts`
- Modify: `server/src/index.ts`

**Step 1: Create Stripe utilities**

Create `server/src/lib/stripe.ts`:
```typescript
import Stripe from 'stripe';
import { sql } from '@vercel/postgres';

export const stripe = new Stripe(process.env.STRIPE_SECRET_KEY || '', {
  apiVersion: '2024-12-18.acacia',
});

export const PLANS = {
  starter: {
    priceId: process.env.STRIPE_STARTER_PRICE_ID || '',
    credits: 1000,
  },
  pro: {
    priceId: process.env.STRIPE_PRO_PRICE_ID || '',
    credits: 5000,
  },
} as const;

export const CREDIT_PACK = {
  priceId: process.env.STRIPE_CREDITS_PRICE_ID || '',
  credits: 500,
};

export async function createOrGetCustomer(userId: string, email: string): Promise<string> {
  // Check if user already has a customer ID
  const result = await sql<{ stripe_customer_id: string | null }>`
    SELECT stripe_customer_id FROM users WHERE id = ${userId}
  `;

  if (result.rows[0]?.stripe_customer_id) {
    return result.rows[0].stripe_customer_id;
  }

  // Create new customer
  const customer = await stripe.customers.create({ email });

  await sql`
    UPDATE users SET stripe_customer_id = ${customer.id} WHERE id = ${userId}
  `;

  return customer.id;
}

export async function addCredits(userId: string, amount: number): Promise<void> {
  await sql`
    UPDATE credits SET balance = balance + ${amount}
    WHERE user_id = ${userId}
  `;
}
```

**Step 2: Create billing routes**

Create `server/src/routes/billing.ts`:
```typescript
import { Hono } from 'hono';
import { authMiddleware } from '../middleware/auth';
import { stripe, PLANS, CREDIT_PACK, createOrGetCustomer, addCredits } from '../lib/stripe';

const billing = new Hono();

// Authenticated routes
billing.use('/portal', authMiddleware);
billing.use('/subscribe', authMiddleware);
billing.use('/buy-credits', authMiddleware);

// POST /billing/subscribe - Create subscription checkout
billing.post('/subscribe', async (c) => {
  const user = c.get('user');
  const body = await c.req.json<{ plan: 'starter' | 'pro' }>();
  const { plan } = body;

  if (!plan || !PLANS[plan]) {
    return c.json({ error: 'Invalid plan' }, 400);
  }

  const customerId = await createOrGetCustomer(user.userId, user.email);

  const session = await stripe.checkout.sessions.create({
    customer: customerId,
    mode: 'subscription',
    line_items: [{ price: PLANS[plan].priceId, quantity: 1 }],
    success_url: `${process.env.VERCEL_URL || 'http://localhost:3000'}/billing/success`,
    cancel_url: `${process.env.VERCEL_URL || 'http://localhost:3000'}/billing/cancel`,
  });

  return c.json({ url: session.url });
});

// POST /billing/buy-credits - One-time credit purchase
billing.post('/buy-credits', async (c) => {
  const user = c.get('user');
  const body = await c.req.json<{ quantity?: number }>();
  const quantity = body.quantity || 1;

  const customerId = await createOrGetCustomer(user.userId, user.email);

  const session = await stripe.checkout.sessions.create({
    customer: customerId,
    mode: 'payment',
    line_items: [{ price: CREDIT_PACK.priceId, quantity }],
    success_url: `${process.env.VERCEL_URL || 'http://localhost:3000'}/billing/success`,
    cancel_url: `${process.env.VERCEL_URL || 'http://localhost:3000'}/billing/cancel`,
    metadata: {
      userId: user.userId,
      credits: String(CREDIT_PACK.credits * quantity),
    },
  });

  return c.json({ url: session.url });
});

// POST /billing/portal - Stripe customer portal
billing.post('/portal', async (c) => {
  const user = c.get('user');

  const customerId = await createOrGetCustomer(user.userId, user.email);

  const session = await stripe.billingPortal.sessions.create({
    customer: customerId,
    return_url: `${process.env.VERCEL_URL || 'http://localhost:3000'}`,
  });

  return c.json({ url: session.url });
});

// POST /billing/webhook - Stripe webhooks
billing.post('/webhook', async (c) => {
  const sig = c.req.header('stripe-signature');
  const body = await c.req.text();

  if (!sig) {
    return c.json({ error: 'Missing signature' }, 400);
  }

  let event: ReturnType<typeof stripe.webhooks.constructEvent>;

  try {
    event = stripe.webhooks.constructEvent(
      body,
      sig,
      process.env.STRIPE_WEBHOOK_SECRET || ''
    );
  } catch {
    return c.json({ error: 'Invalid signature' }, 400);
  }

  switch (event.type) {
    case 'checkout.session.completed': {
      const session = event.data.object;
      // Handle credit purchase
      if (session.mode === 'payment' && session.metadata?.credits) {
        await addCredits(session.metadata.userId, parseInt(session.metadata.credits));
      }
      break;
    }
    case 'invoice.paid': {
      // Handle subscription renewal - reset monthly credits
      // TODO: implement monthly credit reset
      break;
    }
  }

  return c.json({ received: true });
});

export default billing;
```

**Step 3: Mount billing routes**

Update `server/src/index.ts`:
```typescript
import { Hono } from 'hono';
import { handle } from 'hono/vercel';
import auth from './routes/auth';
import ai from './routes/ai';
import billing from './routes/billing';

const app = new Hono().basePath('/api');

app.get('/', (c) => {
  return c.json({
    name: 'nosh-cloud',
    version: '0.1.0',
    status: 'ok',
  });
});

app.get('/health', (c) => {
  return c.json({ status: 'healthy' });
});

app.route('/auth', auth);
app.route('/ai', ai);
app.route('/billing', billing);

export default handle(app);
```

**Step 4: Typecheck**

Run: `cd server && npm run typecheck`

**Step 5: Commit**

```bash
git add -A
git commit -m "feat: add Stripe billing routes"
```

---

### Task 13: Database Schema

**Files:**
- Create: `server/scripts/setup-db.sql`

**Step 1: Create database setup script**

Create `server/scripts/setup-db.sql`:
```sql
-- Users table
CREATE TABLE IF NOT EXISTS users (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  email VARCHAR(255) UNIQUE NOT NULL,
  stripe_customer_id VARCHAR(255),
  created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Subscriptions table
CREATE TABLE IF NOT EXISTS subscriptions (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID REFERENCES users(id) ON DELETE CASCADE,
  stripe_subscription_id VARCHAR(255),
  plan VARCHAR(50) NOT NULL,
  status VARCHAR(50) NOT NULL DEFAULT 'active',
  current_period_end TIMESTAMP WITH TIME ZONE,
  created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
  UNIQUE(user_id)
);

-- Credits table
CREATE TABLE IF NOT EXISTS credits (
  user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
  balance INTEGER NOT NULL DEFAULT 0,
  monthly_allowance INTEGER NOT NULL DEFAULT 0,
  resets_at TIMESTAMP WITH TIME ZONE
);

-- Create credits row when user is created
CREATE OR REPLACE FUNCTION create_user_credits()
RETURNS TRIGGER AS $$
BEGIN
  INSERT INTO credits (user_id, balance, monthly_allowance)
  VALUES (NEW.id, 50, 0);  -- 50 free credits on signup
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER on_user_created
  AFTER INSERT ON users
  FOR EACH ROW
  EXECUTE FUNCTION create_user_credits();

-- Indexes
CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);
CREATE INDEX IF NOT EXISTS idx_subscriptions_user ON subscriptions(user_id);
CREATE INDEX IF NOT EXISTS idx_subscriptions_status ON subscriptions(status);
```

**Step 2: Commit**

```bash
git add -A
git commit -m "feat: add database schema"
```

---

## Phase 5: Cloud Integration in Client

Connect the Rust client to Nosh Cloud.

---

### Task 14: Cloud Client

**Files:**
- Create: `src/ai/cloud.rs`
- Modify: `src/ai/mod.rs`
- Create: `src/auth/mod.rs`
- Create: `src/auth/credentials.rs`

**Step 1: Create credentials module**

Create `src/auth/mod.rs`:
```rust
mod credentials;

pub use credentials::Credentials;
```

Create `src/auth/credentials.rs`:
```rust
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Credentials {
    pub token: Option<String>,
    pub email: Option<String>,
}

impl Credentials {
    pub fn load() -> Result<Self> {
        let path = Self::config_path();

        if path.exists() {
            let content = fs::read_to_string(&path)?;
            let creds: Credentials = toml::from_str(&content)?;
            Ok(creds)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        fs::write(&path, &content)?;

        // Set restrictive permissions (owner read/write only)
        let mut perms = fs::metadata(&path)?.permissions();
        perms.set_mode(0o600);
        fs::set_permissions(&path, perms)?;

        Ok(())
    }

    pub fn clear() -> Result<()> {
        let path = Self::config_path();
        if path.exists() {
            fs::remove_file(&path)?;
        }
        Ok(())
    }

    fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("nosh")
            .join("credentials.toml")
    }

    pub fn is_authenticated(&self) -> bool {
        self.token.is_some()
    }
}
```

**Step 2: Create cloud AI client**

Create `src/ai/cloud.rs`:
```rust
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct CompleteRequest {
    input: String,
    cwd: String,
}

#[derive(Deserialize)]
struct CompleteResponse {
    command: String,
    credits_remaining: i32,
}

#[derive(Deserialize)]
struct ErrorResponse {
    error: String,
    code: Option<String>,
}

pub struct CloudClient {
    client: Client,
    base_url: String,
    token: String,
}

impl CloudClient {
    pub fn new(token: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: std::env::var("NOSH_CLOUD_URL")
                .unwrap_or_else(|_| "https://nosh.sh/api".to_string()),
            token: token.to_string(),
        }
    }

    pub async fn translate(&self, input: &str, cwd: &str) -> Result<(String, i32)> {
        let request = CompleteRequest {
            input: input.to_string(),
            cwd: cwd.to_string(),
        };

        let response = self
            .client
            .post(format!("{}/ai/complete", self.base_url))
            .header("Authorization", format!("Bearer {}", self.token))
            .json(&request)
            .send()
            .await?;

        if response.status() == 402 {
            return Err(anyhow!("Out of credits"));
        }

        if !response.status().is_success() {
            let error: ErrorResponse = response.json().await?;
            return Err(anyhow!("Cloud error: {}", error.error));
        }

        let result: CompleteResponse = response.json().await?;
        Ok((result.command, result.credits_remaining))
    }

    pub async fn get_credits(&self) -> Result<i32> {
        let response = self
            .client
            .get(format!("{}/account/credits", self.base_url))
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Failed to get credits"));
        }

        #[derive(Deserialize)]
        struct CreditsResponse {
            balance: i32,
        }

        let result: CreditsResponse = response.json().await?;
        Ok(result.balance)
    }
}
```

**Step 3: Update AI module exports**

Update `src/ai/mod.rs`:
```rust
mod cloud;
mod ollama;

pub use cloud::CloudClient;
pub use ollama::OllamaClient;
```

**Step 4: Commit**

```bash
git add -A
git commit -m "feat: add cloud client and credentials management"
```

---

### Task 15: Onboarding Flow

**Files:**
- Create: `src/onboarding.rs`
- Modify: `src/main.rs`

**Step 1: Create onboarding module**

Create `src/onboarding.rs`:
```rust
use anyhow::Result;
use crossterm::style::{Color, Print, ResetColor, SetForegroundColor};
use crossterm::ExecutableCommand;
use std::io::{self, Write};
use crate::auth::Credentials;
use crate::config::Config;

pub enum OnboardingChoice {
    Ollama,
    Cloud,
    Quit,
}

pub fn run_onboarding() -> Result<OnboardingChoice> {
    let mut stdout = io::stdout();

    stdout.execute(SetForegroundColor(Color::Cyan))?;
    writeln!(stdout, "\nWelcome to nosh!")?;
    stdout.execute(ResetColor)?;

    writeln!(stdout)?;
    writeln!(stdout, "How would you like to power your shell?")?;
    writeln!(stdout)?;
    writeln!(stdout, "  [1] Ollama (free, runs locally)")?;
    writeln!(stdout, "      Requires Ollama installed with a model")?;
    writeln!(stdout)?;
    writeln!(stdout, "  [2] Nosh Cloud (subscription)")?;
    writeln!(stdout, "      No setup, works instantly")?;
    writeln!(stdout)?;
    writeln!(stdout, "  [q] Quit")?;
    writeln!(stdout)?;
    write!(stdout, "> ")?;
    stdout.flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim();

    match input {
        "1" => {
            setup_ollama()?;
            Ok(OnboardingChoice::Ollama)
        }
        "2" => {
            setup_cloud()?;
            Ok(OnboardingChoice::Cloud)
        }
        "q" | "Q" => Ok(OnboardingChoice::Quit),
        _ => {
            writeln!(stdout, "Invalid choice. Please try again.")?;
            run_onboarding()
        }
    }
}

fn setup_ollama() -> Result<()> {
    let mut stdout = io::stdout();

    writeln!(stdout)?;
    writeln!(stdout, "Setting up Ollama...")?;
    writeln!(stdout)?;
    writeln!(stdout, "Which model would you like to use?")?;
    writeln!(stdout, "(Press enter for default: llama3.2)")?;
    writeln!(stdout)?;
    write!(stdout, "Model: ")?;
    stdout.flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let model = input.trim();
    let model = if model.is_empty() { "llama3.2" } else { model };

    let mut config = Config::load().unwrap_or_default();
    config.ai.backend = "ollama".to_string();
    config.ai.model = model.to_string();
    config.save()?;

    writeln!(stdout)?;
    stdout.execute(SetForegroundColor(Color::Green))?;
    writeln!(stdout, "Ollama configured with model: {}", model)?;
    stdout.execute(ResetColor)?;
    writeln!(stdout)?;

    Ok(())
}

fn setup_cloud() -> Result<()> {
    let mut stdout = io::stdout();

    writeln!(stdout)?;
    writeln!(stdout, "Setting up Nosh Cloud...")?;
    writeln!(stdout)?;
    write!(stdout, "Enter your email: ")?;
    stdout.flush()?;

    let mut email = String::new();
    io::stdin().read_line(&mut email)?;
    let email = email.trim().to_string();

    if !email.contains('@') {
        writeln!(stdout, "Invalid email. Please try again.")?;
        return setup_cloud();
    }

    writeln!(stdout)?;
    writeln!(stdout, "Magic link sent! Check your inbox and click the link.")?;
    writeln!(stdout, "Waiting for authentication...")?;

    // TODO: Actually send magic link and poll for token
    // For now, ask user to paste token
    writeln!(stdout)?;
    write!(stdout, "Paste your token: ")?;
    stdout.flush()?;

    let mut token = String::new();
    io::stdin().read_line(&mut token)?;
    let token = token.trim().to_string();

    if token.is_empty() {
        writeln!(stdout, "No token provided. Please try again.")?;
        return setup_cloud();
    }

    let mut creds = Credentials::load().unwrap_or_default();
    creds.token = Some(token);
    creds.email = Some(email);
    creds.save()?;

    let mut config = Config::load().unwrap_or_default();
    config.ai.backend = "cloud".to_string();
    config.save()?;

    writeln!(stdout)?;
    stdout.execute(SetForegroundColor(Color::Green))?;
    writeln!(stdout, "Authenticated! You're ready to use nosh.")?;
    stdout.execute(ResetColor)?;
    writeln!(stdout)?;

    Ok(())
}

pub fn needs_onboarding(config: &Config, creds: &Credentials) -> bool {
    match config.ai.backend.as_str() {
        "cloud" => !creds.is_authenticated(),
        "ollama" => false, // Ollama doesn't need setup, just warn if not available
        _ => true, // Unknown backend, run onboarding
    }
}
```

**Step 2: Integrate onboarding into main**

Update `src/main.rs`:
```rust
mod ai;
mod auth;
mod config;
mod exec;
mod onboarding;
mod repl;
mod safety;

use ai::{CloudClient, OllamaClient};
use anyhow::Result;
use auth::Credentials;
use config::Config;
use exec::execute_command;
use onboarding::{needs_onboarding, run_onboarding, OnboardingChoice};
use repl::Repl;
use safety::{parse_command, prompt_for_permission, PermissionChoice, PermissionStore, RiskLevel};

#[tokio::main]
async fn main() -> Result<()> {
    println!("nosh v{}", env!("CARGO_PKG_VERSION"));

    let mut config = Config::load().unwrap_or_default();
    let mut creds = Credentials::load().unwrap_or_default();
    let mut permissions = PermissionStore::load().unwrap_or_default();

    // Run onboarding if needed
    if needs_onboarding(&config, &creds) {
        match run_onboarding()? {
            OnboardingChoice::Quit => return Ok(()),
            OnboardingChoice::Ollama => {
                config = Config::load().unwrap_or_default();
            }
            OnboardingChoice::Cloud => {
                config = Config::load().unwrap_or_default();
                creds = Credentials::load().unwrap_or_default();
            }
        }
    }

    // Check Ollama availability if using it
    if config.ai.backend == "ollama" {
        let ollama = OllamaClient::new(&config.ai.model);
        if !ollama.check_available().await {
            eprintln!("Warning: Ollama not available at localhost:11434");
            eprintln!("Start Ollama or run `nosh --setup` to reconfigure.\n");
        }
    }

    println!("Type 'exit' to quit.\n");

    let mut repl = Repl::new()?;
    repl.load_history();

    loop {
        let cwd = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| ".".to_string());

        match repl.readline()? {
            Some(line) if line == "exit" || line == "quit" => break,
            Some(line) => {
                let command_result = if config.ai.backend == "cloud" {
                    if let Some(token) = &creds.token {
                        let client = CloudClient::new(token);
                        client.translate(&line, &cwd).await.map(|(cmd, _)| cmd)
                    } else {
                        Err(anyhow::anyhow!("Not authenticated"))
                    }
                } else {
                    let client = OllamaClient::new(&config.ai.model);
                    client.translate(&line, &cwd).await
                };

                match command_result {
                    Ok(command) => {
                        if config.behavior.show_command {
                            println!("⚡ {}", command);
                        }

                        let parsed = parse_command(&command);

                        let should_execute = match parsed.risk_level {
                            RiskLevel::Safe => true,
                            RiskLevel::Blocked => {
                                safety::prompt::print_blocked(&parsed)?;
                                false
                            }
                            RiskLevel::Critical => {
                                safety::prompt::print_critical_warning(&parsed)?
                            }
                            _ => {
                                if permissions.is_command_allowed(&parsed.info.command) {
                                    true
                                } else if permissions.is_directory_allowed(&cwd) {
                                    true
                                } else {
                                    match prompt_for_permission(&parsed)? {
                                        PermissionChoice::AllowOnce => true,
                                        PermissionChoice::AllowCommand => {
                                            permissions.allow_command(&parsed.info.command, true);
                                            true
                                        }
                                        PermissionChoice::AllowHere => {
                                            permissions.allow_directory(&cwd, true);
                                            true
                                        }
                                        PermissionChoice::Deny => false,
                                    }
                                }
                            }
                        };

                        if should_execute {
                            if let Err(e) = execute_command(&command) {
                                eprintln!("Execution error: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("AI error: {}", e);
                    }
                }
            }
            None => break,
        }
    }

    repl.save_history();
    println!("Goodbye!");
    Ok(())
}
```

**Step 3: Verify it compiles**

Run: `cargo build`

**Step 4: Commit**

```bash
git add -A
git commit -m "feat: add onboarding flow with Ollama and Cloud setup"
```

---

## Summary

After completing all tasks, you'll have:

**Rust Client:**
- REPL with history and autocomplete
- AI translation via Ollama (local) or Cloud (paid)
- Smart command parsing with risk assessment
- Permission prompts with persistence
- TOML configuration
- Onboarding flow

**TypeScript Server:**
- Magic link authentication
- AI proxy to Replicate
- Stripe billing (subscriptions + credit top-ups)
- Credit metering
- Vercel serverless deployment

**Next phases (not covered):**
- Plugin system with themes
- Job control (ctrl+z, fg, bg)
- Community plugins
- nosh credits CLI command
- Improved AI prompts
