//! Completion manager with lazy loading and caching.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::Result;

use super::{
    BuiltinCompleter, CommandCompletion, Completion, CompletionContext, CompletionFile,
    DynamicCompleterDef,
};
use crate::paths;

/// Cache entry for dynamic completer results.
struct DynamicCache {
    results: Vec<String>,
    created: Instant,
    ttl: Duration,
}

impl DynamicCache {
    fn is_valid(&self) -> bool {
        self.created.elapsed() < self.ttl
    }
}

/// Manager for lazy-loading and caching completions.
pub struct CompletionManager {
    /// Loaded command completions (lazily populated)
    commands: RefCell<HashMap<String, CommandCompletion>>,
    /// Cache for dynamic completer results
    dynamic_cache: RefCell<HashMap<String, DynamicCache>>,
    /// Paths to search for completion files
    search_paths: Vec<PathBuf>,
}

impl Default for CompletionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl CompletionManager {
    pub fn new() -> Self {
        // Build search paths from packages
        let mut search_paths = Vec::new();

        // Scan packages directory for completions
        let packages_dir = paths::packages_dir();
        if packages_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&packages_dir) {
                for entry in entries.flatten() {
                    let package_path = entry.path();
                    if package_path.is_dir() {
                        let completions_path = package_path.join("completions");
                        if completions_path.exists() {
                            search_paths.push(completions_path);
                        }
                    }
                }
            }
        }

        Self {
            commands: RefCell::new(HashMap::new()),
            dynamic_cache: RefCell::new(HashMap::new()),
            search_paths,
        }
    }

    /// Get completions for given input line and cursor position.
    pub fn complete(&self, line: &str, pos: usize) -> Vec<Completion> {
        let context = self.parse_context(line, pos);
        self.complete_with_context(&context)
    }

    /// Parse the input line to determine completion context.
    pub fn parse_context(&self, line: &str, pos: usize) -> CompletionContext {
        let line = &line[..pos];

        // Parse words, handling quotes
        let words = match shell_words::split(line) {
            Ok(w) => w,
            Err(_) => {
                // Unclosed quote - try to parse anyway
                line.split_whitespace().map(|s| s.to_string()).collect()
            }
        };

        // Find current word prefix
        let prefix = if line.ends_with(' ') || line.ends_with('\t') {
            String::new()
        } else {
            words.last().cloned().unwrap_or_default()
        };

        // Empty line or completing first word = command completion
        if words.is_empty() || (words.len() == 1 && !line.ends_with(' ')) {
            return CompletionContext::Command { prefix };
        }

        let command = words[0].clone();

        // Completing an option (starts with -)
        if prefix.starts_with('-') {
            let subcommand = self.find_subcommand(&words, &command);
            return CompletionContext::Option {
                command,
                subcommand,
                prefix,
            };
        }

        // Check if previous word was an option that takes a value
        if words.len() >= 2 {
            let prev = &words[words.len() - if prefix.is_empty() { 1 } else { 2 }];
            if prev.starts_with('-') {
                let subcommand = self.find_subcommand(&words, &command);
                if self.option_takes_value(&command, subcommand.as_deref(), prev) {
                    return CompletionContext::OptionValue {
                        command,
                        subcommand,
                        option: prev.clone(),
                        prefix,
                    };
                }
            }
        }

        // Check if we're completing a subcommand
        let subcommand = self.find_subcommand(&words, &command);

        if subcommand.is_none() {
            // Try to complete subcommand if we have one loaded
            self.ensure_loaded(&command);
            if let Some(cmd) = self.commands.borrow().get(&command) {
                if !cmd.subcommands.is_empty() {
                    // Could be completing a subcommand
                    return CompletionContext::Subcommand { command, prefix };
                }
            }
        }

        // Positional argument completion
        CompletionContext::Positional {
            command,
            subcommand,
            prefix,
        }
    }

    /// Find subcommand in the word list.
    fn find_subcommand(&self, words: &[String], command: &str) -> Option<String> {
        self.ensure_loaded(command);

        if let Some(cmd) = self.commands.borrow().get(command) {
            for word in words.iter().skip(1) {
                if !word.starts_with('-') && cmd.subcommands.contains_key(word) {
                    return Some(word.clone());
                }
            }
        }
        None
    }

    /// Check if an option takes a value.
    fn option_takes_value(&self, command: &str, subcommand: Option<&str>, option: &str) -> bool {
        self.ensure_loaded(command);

        if let Some(cmd) = self.commands.borrow().get(command) {
            // Check subcommand options first
            if let Some(sub_name) = subcommand {
                if let Some(sub) = cmd.subcommands.get(sub_name) {
                    for opt in &sub.options {
                        if opt.name == option && opt.takes_value {
                            return true;
                        }
                    }
                }
            }

            // Check command options
            for opt in &cmd.options {
                if opt.name == option && opt.takes_value {
                    return true;
                }
            }
        }

        false
    }

    /// Complete based on parsed context.
    fn complete_with_context(&self, context: &CompletionContext) -> Vec<Completion> {
        match context {
            CompletionContext::Command { prefix } => self.complete_command(prefix),

            CompletionContext::Subcommand { command, prefix } => {
                self.complete_subcommand(command, prefix)
            }

            CompletionContext::Option {
                command,
                subcommand,
                prefix,
            } => self.complete_option(command, subcommand.as_deref(), prefix),

            CompletionContext::OptionValue {
                command,
                subcommand,
                option,
                prefix,
            } => self.complete_option_value(command, subcommand.as_deref(), option, prefix),

            CompletionContext::Positional {
                command,
                subcommand,
                prefix,
                ..
            } => self.complete_positional(command, subcommand.as_deref(), prefix),
        }
    }

    /// Complete command names (executables from PATH).
    /// Enhances results with descriptions from TOML files when available.
    fn complete_command(&self, prefix: &str) -> Vec<Completion> {
        let mut completions = BuiltinCompleter::Executables.complete(prefix);

        // Enhance with descriptions from our completion files
        for completion in &mut completions {
            self.ensure_loaded(&completion.text);
            if let Some(cmd) = self.commands.borrow().get(&completion.text) {
                if let Some(desc) = &cmd.description {
                    completion.description = Some(desc.clone());
                }
            }
        }

        completions
    }

    /// Complete subcommand names.
    fn complete_subcommand(&self, command: &str, prefix: &str) -> Vec<Completion> {
        self.ensure_loaded(command);

        if let Some(cmd) = self.commands.borrow().get(command) {
            cmd.subcommands
                .iter()
                .filter(|(name, _)| name.starts_with(prefix))
                .map(|(name, sub)| {
                    let mut c = Completion::new(name);
                    if let Some(desc) = &sub.description {
                        c = c.with_description(desc);
                    }
                    c
                })
                .collect()
        } else {
            // No subcommands defined - fall back to file completion
            BuiltinCompleter::Files.complete(prefix)
        }
    }

    /// Complete option names.
    fn complete_option(
        &self,
        command: &str,
        subcommand: Option<&str>,
        prefix: &str,
    ) -> Vec<Completion> {
        self.ensure_loaded(command);

        let mut completions = Vec::new();

        if let Some(cmd) = self.commands.borrow().get(command) {
            // Get subcommand options if present
            if let Some(sub_name) = subcommand {
                if let Some(sub) = cmd.subcommands.get(sub_name) {
                    for opt in &sub.options {
                        if opt.name.starts_with(prefix) {
                            let mut c = Completion::new(&opt.name);
                            if let Some(desc) = &opt.description {
                                c = c.with_description(desc);
                            }
                            completions.push(c);
                        }
                    }
                }
            }

            // Add command-level options
            for opt in &cmd.options {
                if opt.name.starts_with(prefix) {
                    let mut c = Completion::new(&opt.name);
                    if let Some(desc) = &opt.description {
                        c = c.with_description(desc);
                    }
                    completions.push(c);
                }
            }
        }

        completions
    }

    /// Complete option value.
    fn complete_option_value(
        &self,
        command: &str,
        subcommand: Option<&str>,
        option: &str,
        prefix: &str,
    ) -> Vec<Completion> {
        self.ensure_loaded(command);

        if let Some(cmd) = self.commands.borrow().get(command) {
            // Find the option's value completer
            let completer_name = self.find_option_completer(cmd, subcommand, option);

            if let Some(name) = completer_name {
                return self.run_completer(command, &name, prefix);
            }
        }

        // Default to file completion for option values
        BuiltinCompleter::Files.complete(prefix)
    }

    /// Find the completer for an option value.
    fn find_option_completer(
        &self,
        cmd: &CommandCompletion,
        subcommand: Option<&str>,
        option: &str,
    ) -> Option<String> {
        // Check subcommand options first
        if let Some(sub_name) = subcommand {
            if let Some(sub) = cmd.subcommands.get(sub_name) {
                for opt in &sub.options {
                    if opt.name == option {
                        return opt.value_completer.clone();
                    }
                }
            }
        }

        // Check command options
        for opt in &cmd.options {
            if opt.name == option {
                return opt.value_completer.clone();
            }
        }

        None
    }

    /// Complete positional argument.
    fn complete_positional(
        &self,
        command: &str,
        subcommand: Option<&str>,
        prefix: &str,
    ) -> Vec<Completion> {
        self.ensure_loaded(command);

        if let Some(cmd) = self.commands.borrow().get(command) {
            // Check subcommand's positional completer
            if let Some(sub_name) = subcommand {
                if let Some(sub) = cmd.subcommands.get(sub_name) {
                    if let Some(ref completer) = sub.positional {
                        return self.run_completer(command, completer, prefix);
                    }
                }
            }

            // Check command's positional completer
            if let Some(ref completer) = cmd.positional {
                return self.run_completer(command, completer, prefix);
            }
        }

        // Default to file completion
        BuiltinCompleter::Files.complete(prefix)
    }

    /// Run a completer by name (builtin or dynamic).
    fn run_completer(&self, command: &str, completer: &str, prefix: &str) -> Vec<Completion> {
        // Check if it's a builtin
        if let Some(builtin) = BuiltinCompleter::from_name(completer) {
            return builtin.complete(prefix);
        }

        // Check if it's a dynamic completer
        if let Some(cmd) = self.commands.borrow().get(command) {
            if let Some(dynamic) = cmd.dynamic.get(completer) {
                return self.run_dynamic_completer(completer, dynamic, prefix);
            }
        }

        // Unknown completer - default to files
        BuiltinCompleter::Files.complete(prefix)
    }

    /// Run a dynamic completer (executes shell command).
    fn run_dynamic_completer(
        &self,
        name: &str,
        def: &DynamicCompleterDef,
        prefix: &str,
    ) -> Vec<Completion> {
        let cache_key = name.to_string();

        // Check cache
        {
            let cache = self.dynamic_cache.borrow();
            if let Some(entry) = cache.get(&cache_key) {
                if entry.is_valid() {
                    return entry
                        .results
                        .iter()
                        .filter(|s| s.starts_with(prefix))
                        .map(|s| Completion::new(s))
                        .collect();
                }
            }
        }

        // Run the command
        let results = self.execute_dynamic_command(&def.command);

        // Cache the results
        let ttl = Duration::from_secs(def.cache_seconds.unwrap_or(5));
        self.dynamic_cache.borrow_mut().insert(
            cache_key,
            DynamicCache {
                results: results.clone(),
                created: Instant::now(),
                ttl,
            },
        );

        results
            .iter()
            .filter(|s| s.starts_with(prefix))
            .map(|s| Completion::new(s))
            .collect()
    }

    /// Execute a shell command and return lines of output.
    fn execute_dynamic_command(&self, cmd: &str) -> Vec<String> {
        let output = Command::new("sh").args(["-c", cmd]).output();

        match output {
            Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout)
                .lines()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            _ => Vec::new(),
        }
    }

    /// Ensure completions for a command are loaded.
    fn ensure_loaded(&self, command: &str) {
        if self.commands.borrow().contains_key(command) {
            return; // Already loaded
        }

        // Search for completion file
        for path in &self.search_paths {
            let file = path.join(format!("{}.toml", command));
            if file.exists() {
                if let Ok(completion) = self.load_file(&file, command) {
                    self.commands
                        .borrow_mut()
                        .insert(command.to_string(), completion);
                    return;
                }
            }
        }
    }

    /// Load completion from a TOML file.
    fn load_file(&self, path: &Path, command: &str) -> Result<CommandCompletion> {
        let content = fs::read_to_string(path)?;
        let file: CompletionFile = toml::from_str(&content)?;

        // Find the completion for this command
        if let Some(def) = file.completions.get(command) {
            Ok(CommandCompletion::from_def(def.clone()))
        } else {
            // Try to use first completion in file
            if let Some((_name, def)) = file.completions.into_iter().next() {
                Ok(CommandCompletion::from_def(def))
            } else {
                anyhow::bail!("No completions found in {}", path.display())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_context_empty() {
        let mgr = CompletionManager::new();
        let ctx = mgr.parse_context("", 0);
        assert!(matches!(ctx, CompletionContext::Command { .. }));
    }

    #[test]
    fn test_parse_context_command() {
        let mgr = CompletionManager::new();
        let ctx = mgr.parse_context("gi", 2);
        match ctx {
            CompletionContext::Command { prefix } => assert_eq!(prefix, "gi"),
            _ => panic!("Expected Command context"),
        }
    }

    #[test]
    fn test_parse_context_option() {
        let mgr = CompletionManager::new();
        let ctx = mgr.parse_context("git commit -", 12);
        match ctx {
            CompletionContext::Option { prefix, .. } => assert_eq!(prefix, "-"),
            _ => panic!("Expected Option context"),
        }
    }
}
