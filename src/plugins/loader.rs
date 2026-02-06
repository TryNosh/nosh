//! Plugin loader for nosh.
//!
//! Loads plugins from the plugins directory and executes their commands.

use anyhow::Result;
use nosh_context::ContextCache;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use super::{Plugin, VariableProvider};
use crate::paths;

/// Cache entry for a variable value.
struct CacheEntry {
    value: String,
    expires_at: Instant,
}

/// Plugin manager that loads and executes plugins.
pub struct PluginManager {
    plugins: HashMap<String, Plugin>,
    cache: HashMap<String, CacheEntry>,
    cache_duration: Duration,
    last_command_duration: Option<Duration>,
    context_cache: ContextCache,
}

impl PluginManager {
    /// Create a new plugin manager with default cache duration.
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            cache: HashMap::new(),
            cache_duration: Duration::from_millis(500),
            last_command_duration: None,
            context_cache: ContextCache::new(),
        }
    }

    /// Load all plugins from the plugins directory.
    pub fn load_plugins(&mut self) -> Result<()> {
        let plugins_dir = paths::plugins_dir();

        // Load from builtin subdirectory
        let builtin_dir = plugins_dir.join("builtin");
        if builtin_dir.exists() {
            self.load_from_directory(&builtin_dir)?;
        }

        // Load from community subdirectory
        let community_dir = plugins_dir.join("community");
        if community_dir.exists() {
            self.load_from_directory(&community_dir)?;
        }

        Ok(())
    }

    /// Load plugins from a specific directory.
    fn load_from_directory(&mut self, dir: &Path) -> Result<()> {
        if !dir.exists() {
            return Ok(());
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().map_or(false, |ext| ext == "toml") {
                if let Ok(plugin) = self.load_plugin(&path) {
                    self.plugins.insert(plugin.plugin.name.clone(), plugin);
                }
            }
        }

        Ok(())
    }

    /// Load a single plugin from a TOML file.
    fn load_plugin(&self, path: &Path) -> Result<Plugin> {
        let content = fs::read_to_string(path)?;
        let plugin: Plugin = toml::from_str(&content)?;
        Ok(plugin)
    }

    /// Set the duration of the last executed command.
    pub fn set_last_command_duration(&mut self, duration: Duration) {
        self.last_command_duration = Some(duration);
    }

    /// Get a variable value from a plugin.
    ///
    /// Format: "plugin_name:variable_name" (e.g., "git:branch")
    pub fn get_variable(&mut self, key: &str) -> Option<String> {
        let parts: Vec<&str> = key.split(':').collect();
        if parts.len() != 2 {
            return None;
        }

        let plugin_name = parts[0];
        let var_name = parts[1];

        // Handle context plugin specially (uses nosh-context library)
        if plugin_name == "context" {
            return self.get_context_variable(var_name);
        }

        // Check cache first
        let cache_key = key.to_string();
        if let Some(entry) = self.cache.get(&cache_key) {
            if entry.expires_at > Instant::now() {
                return Some(entry.value.clone());
            }
        }

        // Get from plugin
        let plugin = self.plugins.get(plugin_name)?;
        let provider = plugin.provides.get(var_name)?;

        let value = self.execute_provider(plugin, var_name, provider)?;

        // Cache the result
        self.cache.insert(
            cache_key,
            CacheEntry {
                value: value.clone(),
                expires_at: Instant::now() + self.cache_duration,
            },
        );

        Some(value)
    }

    /// Get a context variable from nosh-context library.
    fn get_context_variable(&mut self, var_name: &str) -> Option<String> {
        let dir = std::env::current_dir().ok()?;
        let ctx = self.context_cache.get(&dir);

        match var_name {
            // Git information
            "git_branch" => ctx.git.as_ref().map(|g| g.branch.clone()),
            "git_status" => ctx.git.as_ref().map(|g| g.status_indicator()),

            // Package information
            "package_name" => ctx.package.as_ref().map(|p| p.name.clone()),
            "package_version" => ctx.package.as_ref().map(|p| p.version.clone()),
            "package_icon" => ctx.package.as_ref().map(|_| "📦".to_string()),

            // Rust
            "rust_version" => ctx.rust.as_ref().map(|r| r.version.clone()),
            "rust_icon" => ctx.rust.as_ref().map(|_| "🦀".to_string()),

            // Node.js
            "node_version" => ctx.node.as_ref().map(|n| n.version.clone()),
            "node_icon" => ctx.node.as_ref().map(|_| "⬢".to_string()),

            // Bun
            "bun_version" => ctx.bun.as_ref().map(|b| b.version.clone()),
            "bun_icon" => ctx.bun.as_ref().map(|_| "🥟".to_string()),

            // Go
            "go_version" => ctx.go.as_ref().map(|g| g.version.clone()),
            "go_icon" => ctx.go.as_ref().map(|_| "🐹".to_string()),

            // Python
            "python_version" => ctx.python.as_ref().map(|p| p.version.clone()),
            "python_icon" => ctx.python.as_ref().map(|_| "🐍".to_string()),

            // C++
            "cpp_version" => ctx.cpp.as_ref().map(|c| c.version.clone()),
            "cpp_icon" => ctx.cpp.as_ref().map(|_| "⚙️".to_string()),

            // Docker
            "docker_version" => ctx.docker.as_ref().map(|d| d.version.clone()),
            "docker_icon" => ctx.docker.as_ref().map(|_| "🐳".to_string()),

            _ => None,
        }
    }

    /// Execute a variable provider and return its value.
    fn execute_provider(
        &self,
        plugin: &Plugin,
        var_name: &str,
        provider: &VariableProvider,
    ) -> Option<String> {
        match provider {
            VariableProvider::Command { command, transform } => {
                let output = Command::new("sh")
                    .arg("-c")
                    .arg(command)
                    .output()
                    .ok()?;

                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();

                // Apply transform
                match transform.as_deref() {
                    Some("non_empty") => {
                        if stdout.is_empty() {
                            // Return empty icon for clean
                            plugin.icons.get("clean").cloned()
                        } else {
                            // Return dirty icon
                            plugin.icons.get("dirty").cloned()
                        }
                    }
                    Some("trim") => Some(stdout),
                    _ => Some(stdout),
                }
            }
            VariableProvider::Internal { source } => {
                match source.as_str() {
                    "internal" => {
                        // Handle internal variables like exec_time:duration, exec_time:took
                        if var_name == "duration" || var_name == "took" {
                            if let Some(duration) = self.last_command_duration {
                                // Get min_ms from plugin config
                                let min_ms = plugin
                                    .config
                                    .get("min_ms")
                                    .and_then(|v| v.as_integer())
                                    .unwrap_or(500) as u64;

                                let ms = duration.as_millis() as u64;
                                if ms >= min_ms {
                                    let formatted = format_duration(duration);
                                    if var_name == "took" {
                                        return Some(format!("took {}", formatted));
                                    }
                                    return Some(formatted);
                                }
                            }
                        }
                        None
                    }
                    _ => None,
                }
            }
        }
    }

    /// Invalidate the cache.
    pub fn invalidate_cache(&mut self) {
        self.cache.clear();
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Format a duration for display.
fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs();
    let ms = duration.subsec_millis();

    if secs >= 60 {
        let mins = secs / 60;
        let remaining_secs = secs % 60;
        format!("{}m{}s", mins, remaining_secs)
    } else if secs > 0 {
        format!("{}.{}s", secs, ms / 100)
    } else {
        format!("{}ms", ms)
    }
}
