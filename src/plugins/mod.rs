//! Plugin system for nosh.
//!
//! Plugins provide prompt variables via commands or internal sources.

pub mod builtins;
pub mod loader;
pub mod theme;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A nosh plugin that provides prompt variables.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plugin {
    pub plugin: PluginMeta,
    #[serde(default)]
    pub provides: HashMap<String, VariableProvider>,
    #[serde(default)]
    pub icons: HashMap<String, String>,
    #[serde(default)]
    pub config: HashMap<String, toml::Value>,
}

/// Plugin metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMeta {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

/// Defines how a variable is provided.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum VariableProvider {
    /// Variable provided by running a shell command.
    Command {
        command: String,
        #[serde(default)]
        transform: Option<String>,
    },
    /// Variable provided internally by nosh.
    Internal {
        source: String,
    },
}

