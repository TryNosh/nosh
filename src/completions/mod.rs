//! TOML-based completion system for nosh.
//!
//! Completions are defined in TOML files and loaded lazily on-demand.
//! Files are searched in `~/.config/nosh/completions/` and `~/.config/nosh/plugins/`.

mod builtins;
mod manager;
mod zsh_convert;

pub use builtins::BuiltinCompleter;
pub use manager::CompletionManager;
pub use zsh_convert::convert_zsh_file;

use serde::Deserialize;
use std::collections::HashMap;

/// Context for completion - determines what type of completion is needed.
#[derive(Debug, Clone)]
pub enum CompletionContext {
    /// Completing command name (first word)
    Command { prefix: String },
    /// Completing subcommand
    Subcommand { command: String, prefix: String },
    /// Completing option (starting with - or --)
    Option {
        command: String,
        subcommand: Option<String>,
        prefix: String,
    },
    /// Completing option value
    OptionValue {
        command: String,
        subcommand: Option<String>,
        option: String,
        prefix: String,
    },
    /// Completing positional argument
    Positional {
        command: String,
        subcommand: Option<String>,
        prefix: String,
    },
}

/// A completion candidate.
#[derive(Debug, Clone)]
pub struct Completion {
    /// The text to insert
    pub text: String,
    /// Display text (may include formatting)
    pub display: String,
    /// Optional description
    pub description: Option<String>,
}

impl Completion {
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        Self {
            display: text.clone(),
            text,
            description: None,
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }
}

/// Root structure for parsing completion TOML files.
#[derive(Debug, Deserialize)]
pub struct CompletionFile {
    /// Map of command name to its completion definition
    pub completions: HashMap<String, CommandCompletionDef>,
}

/// Definition of completions for a command (TOML structure).
#[derive(Debug, Deserialize, Clone)]
pub struct CommandCompletionDef {
    /// Command description
    pub description: Option<String>,
    /// Simple subcommands with descriptions
    #[serde(default)]
    pub subcommands: HashMap<String, SubcommandValue>,
    /// Options for the main command
    #[serde(default)]
    pub options: HashMap<String, OptionValue>,
    /// Built-in or dynamic completer name for positional args
    pub positional: Option<String>,
    /// Dynamic completers (run shell commands)
    #[serde(default)]
    pub dynamic: HashMap<String, DynamicCompleterDef>,
}

/// Value for a subcommand - can be a simple string or detailed definition.
#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum SubcommandValue {
    /// Simple description string
    Simple(String),
    /// Detailed subcommand definition
    Detailed(SubcommandDef),
}

/// Detailed subcommand definition with options and positional completers.
#[derive(Debug, Deserialize, Clone)]
pub struct SubcommandDef {
    /// Subcommand description
    pub description: Option<String>,
    /// Options for this subcommand
    #[serde(default)]
    pub options: Vec<OptionDef>,
    /// Built-in or dynamic completer name for positional args
    pub positional: Option<String>,
}

/// Value for an option - can be a simple string or detailed definition.
#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum OptionValue {
    /// Simple description string
    Simple(String),
    /// Detailed option definition
    Detailed(OptionDetailedDef),
}

impl OptionValue {
    pub fn description(&self) -> Option<&str> {
        match self {
            OptionValue::Simple(s) => Some(s.as_str()),
            OptionValue::Detailed(d) => d.description.as_deref(),
        }
    }

    pub fn takes_value(&self) -> bool {
        match self {
            OptionValue::Simple(_) => false,
            OptionValue::Detailed(d) => d.takes_value.unwrap_or(false),
        }
    }

    pub fn value_completer(&self) -> Option<&str> {
        match self {
            OptionValue::Simple(_) => None,
            OptionValue::Detailed(d) => d.value_completer.as_deref(),
        }
    }
}

/// Detailed option definition.
#[derive(Debug, Deserialize, Clone)]
pub struct OptionDetailedDef {
    pub description: Option<String>,
    #[serde(default)]
    pub takes_value: Option<bool>,
    /// Completer for the option value (built-in or dynamic name)
    pub value_completer: Option<String>,
}

/// Option definition in a list format (for subcommand options).
#[derive(Debug, Deserialize, Clone)]
pub struct OptionDef {
    /// Option name (e.g., "-m", "--message")
    pub name: String,
    /// Option description
    pub description: Option<String>,
    /// Whether this option takes a value
    #[serde(default)]
    pub takes_value: bool,
    /// Completer for the option value
    pub value_completer: Option<String>,
}

/// Dynamic completer that runs a shell command.
#[derive(Debug, Deserialize, Clone)]
pub struct DynamicCompleterDef {
    /// Shell command to run
    pub command: String,
    /// Cache duration in seconds (default: no cache)
    pub cache_seconds: Option<u64>,
}

/// Resolved command completion (after parsing TOML).
#[derive(Debug, Clone)]
pub struct CommandCompletion {
    /// Command description (from TOML)
    pub description: Option<String>,
    pub subcommands: HashMap<String, SubcommandCompletion>,
    pub options: Vec<OptionCompletion>,
    pub positional: Option<String>,
    pub dynamic: HashMap<String, DynamicCompleterDef>,
}

/// Resolved subcommand completion.
#[derive(Debug, Clone)]
pub struct SubcommandCompletion {
    pub description: Option<String>,
    pub options: Vec<OptionCompletion>,
    pub positional: Option<String>,
}

/// Resolved option completion.
#[derive(Debug, Clone)]
pub struct OptionCompletion {
    pub name: String,
    pub description: Option<String>,
    pub takes_value: bool,
    pub value_completer: Option<String>,
}

impl CommandCompletion {
    /// Parse from TOML definition.
    pub fn from_def(def: CommandCompletionDef) -> Self {
        let options = def
            .options
            .iter()
            .map(|(name, val)| OptionCompletion {
                name: name.clone(),
                description: val.description().map(|s| s.to_string()),
                takes_value: val.takes_value(),
                value_completer: val.value_completer().map(|s| s.to_string()),
            })
            .collect();

        let subcommands = def
            .subcommands
            .iter()
            .map(|(name, val)| {
                let sub = match val {
                    SubcommandValue::Simple(desc) => SubcommandCompletion {
                        description: Some(desc.clone()),
                        options: vec![],
                        positional: None,
                    },
                    SubcommandValue::Detailed(d) => SubcommandCompletion {
                        description: d.description.clone(),
                        options: d
                            .options
                            .iter()
                            .map(|o| OptionCompletion {
                                name: o.name.clone(),
                                description: o.description.clone(),
                                takes_value: o.takes_value,
                                value_completer: o.value_completer.clone(),
                            })
                            .collect(),
                        positional: d.positional.clone(),
                    },
                };
                (name.clone(), sub)
            })
            .collect();

        Self {
            description: def.description,
            subcommands,
            options,
            positional: def.positional,
            dynamic: def.dynamic,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_completion_file() {
        let toml = r#"
[completions.test]
description = "Test command"

[completions.test.subcommands]
sub1 = "First subcommand"

[completions.test.options]
"--help" = "Show help"
"#;

        let file: CompletionFile = toml::from_str(toml).unwrap();
        let def = file.completions.get("test").unwrap();

        assert_eq!(def.description.as_deref(), Some("Test command"));
        assert!(def.subcommands.contains_key("sub1"));
        assert!(def.options.contains_key("--help"));
    }
}
