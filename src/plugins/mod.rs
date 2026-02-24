//! Plugin system for nosh.
//!
//! Plugins provide prompt variables via commands or internal sources.

pub mod builtins;
pub mod loader;
pub mod theme;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

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
        /// How long to wait for the command before using cached value.
        /// "0" = don't wait (fully async), default = "100ms"
        #[serde(default)]
        timeout: Option<String>,
        /// How long to cache the value.
        /// "always" = no caching (always fetch fresh), "never" = cache forever, default = "500ms"
        #[serde(default)]
        cache: Option<String>,
    },
    /// Variable provided internally by nosh.
    Internal { source: String },
}

/// Parse a duration string like "100ms", "1s", "5m", "1h".
/// Returns None for invalid formats.
pub fn parse_duration(s: &str) -> Option<Duration> {
    let s = s.trim();

    // Try to find where the number ends and unit begins
    let num_end = s
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(s.len());
    let (num_str, unit) = s.split_at(num_end);

    let num: f64 = num_str.parse().ok()?;

    let multiplier = match unit.trim().to_lowercase().as_str() {
        "" | "ms" => 1.0,
        "s" => 1000.0,
        "m" => 60_000.0,
        "h" => 3_600_000.0,
        _ => return None,
    };

    Some(Duration::from_millis((num * multiplier) as u64))
}

/// Cache duration setting.
#[derive(Debug, Clone, Copy)]
pub enum CacheDuration {
    /// Always fetch fresh (no caching)
    Always,
    /// Never expire (cache forever)
    Never,
    /// Custom duration
    Duration(Duration),
}

impl CacheDuration {
    /// Parse a cache duration string.
    /// "always" = no caching, "never" = cache forever, otherwise parse as duration.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "always" => Some(CacheDuration::Always),
            "never" => Some(CacheDuration::Never),
            _ => parse_duration(s).map(CacheDuration::Duration),
        }
    }
}
