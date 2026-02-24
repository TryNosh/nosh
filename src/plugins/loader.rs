//! Plugin loader for nosh.
//!
//! Loads plugins from the plugins directory and executes their commands.
//! Supports async parallel execution with soft/hard timeouts.

use anyhow::Result;
use nosh_context::ContextCache;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::Instant;

use super::{CacheDuration, Plugin, VariableProvider, parse_duration};
use crate::paths;

/// Soft timeout - use cached value after this duration.
const SOFT_TIMEOUT: Duration = Duration::from_millis(100);

/// Hard timeout - kill task after this duration.
const HARD_TIMEOUT: Duration = Duration::from_secs(5);

/// Default cache duration for variable values.
const CACHE_DURATION: Duration = Duration::from_millis(500);

/// Cache entry for a variable value.
#[derive(Clone)]
struct CacheEntry {
    value: String,
    /// When the cache expires. None means never expires.
    expires_at: Option<Instant>,
}

/// State for a running plugin task.
struct RunningTask {
    handle: JoinHandle<Option<String>>,
    started_at: Instant,
}

/// Plugin manager that loads and executes plugins.
pub struct PluginManager {
    plugins: HashMap<String, Plugin>,
    cache: Arc<Mutex<HashMap<String, CacheEntry>>>,
    running_tasks: Arc<Mutex<HashMap<String, RunningTask>>>,
    last_command_duration: Option<Duration>,
    context_cache: ContextCache,
}

impl PluginManager {
    /// Create a new plugin manager.
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            cache: Arc::new(Mutex::new(HashMap::new())),
            running_tasks: Arc::new(Mutex::new(HashMap::new())),
            last_command_duration: None,
            context_cache: ContextCache::new(),
        }
    }

    /// Load all plugins from plugins directory and packages.
    pub fn load_plugins(&mut self) -> Result<()> {
        // Load from community subdirectory (user's local plugins from /create)
        let community_dir = paths::plugins_dir().join("community");
        if community_dir.exists() {
            self.load_from_directory(&community_dir, None)?;
        }

        // Load from packages (includes builtins and git-installed packages)
        let packages_dir = paths::packages_dir();
        if packages_dir.exists()
            && let Ok(entries) = fs::read_dir(&packages_dir)
        {
            for entry in entries.flatten() {
                let package_path = entry.path();
                if package_path.is_dir()
                    && let Some(package_name) = package_path.file_name().and_then(|n| n.to_str())
                {
                    let plugins_path = package_path.join("plugins");
                    if plugins_path.exists() {
                        // Load plugins with "package_name/" prefix
                        self.load_from_directory(&plugins_path, Some(package_name))?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Load plugins from a specific directory.
    ///
    /// If `package_prefix` is provided, plugins are registered with the name "package/plugin".
    fn load_from_directory(&mut self, dir: &Path, package_prefix: Option<&str>) -> Result<()> {
        if !dir.exists() {
            return Ok(());
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().is_some_and(|ext| ext == "toml")
                && let Ok(mut plugin) = self.load_plugin(&path)
            {
                // Apply package prefix if provided
                let name = if let Some(prefix) = package_prefix {
                    format!("{}/{}", prefix, plugin.plugin.name)
                } else {
                    plugin.plugin.name.clone()
                };
                plugin.plugin.name = name.clone();
                self.plugins.insert(name, plugin);
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
    pub fn set_last_command_duration(&mut self, duration: std::time::Duration) {
        self.last_command_duration = Some(duration);
    }

    /// Get all variables needed for prompt, with parallel execution and per-variable timeout.
    /// Returns a map of variable key -> value.
    pub async fn get_variables(&mut self, keys: Vec<String>) -> HashMap<String, String> {
        // First, clean up any stale tasks
        self.cleanup_stale_tasks().await;

        let mut results = HashMap::new();
        let mut tasks_to_spawn: Vec<(String, Duration)> = Vec::new(); // (key, timeout)
        let mut internal_keys: Vec<String> = Vec::new();

        // Phase 1: Categorize keys and check running/cached state
        // We separate internal keys to process them outside the lock
        {
            let running = self.running_tasks.lock().await;
            let cache = self.cache.lock().await;

            for key in &keys {
                // Identify internal variables (will be processed later)
                if self.is_internal_variable(key) {
                    internal_keys.push(key.clone());
                    continue;
                }

                // Check if already running from previous prompt
                if running.contains_key(key) {
                    // Use cached value if available, don't spawn new task
                    if let Some(entry) = cache.get(key) {
                        results.insert(key.clone(), entry.value.clone());
                    }
                    continue;
                }

                // Check cache - use if not expired
                if let Some(entry) = cache.get(key) {
                    let is_valid = match entry.expires_at {
                        None => true, // Never expires
                        Some(expires) => expires > Instant::now(),
                    };
                    if is_valid {
                        results.insert(key.clone(), entry.value.clone());
                        continue;
                    }
                }

                // Need to spawn a task for this variable
                let timeout = self.get_variable_timeout(key);
                tasks_to_spawn.push((key.clone(), timeout));
            }
        }

        // Process internal variables (needs &mut self, done outside locks)
        for key in internal_keys {
            if let Some(value) = self.get_internal_variable(&key) {
                results.insert(key, value);
            }
        }

        // Phase 2: Spawn tasks for variables that need fetching
        for (key, _) in &tasks_to_spawn {
            self.spawn_variable_task(key.clone()).await;
        }

        // Phase 3: Wait for tasks with shared deadline
        if !tasks_to_spawn.is_empty() {
            // Use the maximum non-zero timeout as the shared deadline
            let max_timeout = tasks_to_spawn
                .iter()
                .map(|(_, t)| *t)
                .filter(|t| !t.is_zero())
                .max()
                .unwrap_or(SOFT_TIMEOUT);
            let deadline = Instant::now() + max_timeout;

            for (key, timeout) in &tasks_to_spawn {
                if timeout.is_zero() {
                    // Timeout = 0: fully async, don't wait, just use cached value
                    let cache = self.cache.lock().await;
                    let value = cache.get(key).map(|e| e.value.clone()).unwrap_or_default();
                    results.insert(key.clone(), value);
                } else {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    if remaining.is_zero() {
                        // Shared deadline exceeded, use cache or empty
                        let cache = self.cache.lock().await;
                        let value = cache.get(key).map(|e| e.value.clone()).unwrap_or_default();
                        results.insert(key.clone(), value);
                    } else {
                        // Try to get result within remaining time
                        if let Some(value) = self.try_get_result(key, remaining).await {
                            results.insert(key.clone(), value);
                        } else {
                            // Task didn't complete in time - use cached value or empty
                            let cache = self.cache.lock().await;
                            let value = cache.get(key).map(|e| e.value.clone()).unwrap_or_default();
                            results.insert(key.clone(), value);
                        }
                    }
                }
            }
        }

        results
    }

    /// Get the timeout duration for a variable.
    fn get_variable_timeout(&self, key: &str) -> Duration {
        let parts: Vec<&str> = key.split(':').collect();
        if parts.len() != 2 {
            return SOFT_TIMEOUT;
        }

        let plugin_name = parts[0];
        let var_name = parts[1];

        if let Some(plugin) = self.plugins.get(plugin_name)
            && let Some(VariableProvider::Command { timeout, .. }) = plugin.provides.get(var_name)
            && let Some(timeout_str) = timeout
        {
            return parse_duration(timeout_str).unwrap_or(SOFT_TIMEOUT);
        }

        SOFT_TIMEOUT
    }

    /// Get the cache duration setting for a variable.
    fn get_variable_cache_duration(&self, key: &str) -> CacheDuration {
        let parts: Vec<&str> = key.split(':').collect();
        if parts.len() != 2 {
            return CacheDuration::Duration(CACHE_DURATION);
        }

        let plugin_name = parts[0];
        let var_name = parts[1];

        if let Some(plugin) = self.plugins.get(plugin_name)
            && let Some(VariableProvider::Command { cache, .. }) = plugin.provides.get(var_name)
            && let Some(cache_str) = cache
        {
            return CacheDuration::parse(cache_str)
                .unwrap_or(CacheDuration::Duration(CACHE_DURATION));
        }

        CacheDuration::Duration(CACHE_DURATION)
    }

    /// Check if a variable key refers to an internal (synchronous) variable.
    fn is_internal_variable(&self, key: &str) -> bool {
        let parts: Vec<&str> = key.split(':').collect();
        if parts.len() != 2 {
            return false;
        }

        let plugin_name = parts[0];
        let var_name = parts[1];

        // Context plugin is handled separately (uses nosh-context library)
        // Support both "context" (local) and "builtins/context" (package) names
        if plugin_name == "context" || plugin_name == "builtins/context" {
            return true;
        }

        // Check if it's an internal provider
        if let Some(plugin) = self.plugins.get(plugin_name)
            && let Some(provider) = plugin.provides.get(var_name)
        {
            return matches!(provider, VariableProvider::Internal { .. });
        }

        false
    }

    /// Get an internal variable value (synchronous).
    fn get_internal_variable(&mut self, key: &str) -> Option<String> {
        let parts: Vec<&str> = key.split(':').collect();
        if parts.len() != 2 {
            return None;
        }

        let plugin_name = parts[0];
        let var_name = parts[1];

        // Handle context plugin specially (uses nosh-context library)
        // Support both "context" (local) and "builtins/context" (package) names
        if plugin_name == "context" || plugin_name == "builtins/context" {
            return self.get_context_variable(var_name);
        }

        // Handle internal providers
        let plugin = self.plugins.get(plugin_name)?;
        let provider = plugin.provides.get(var_name)?;

        if let VariableProvider::Internal { source } = provider
            && source == "internal"
        {
            // Handle internal variables like exec_time:duration, exec_time:took
            if (var_name == "duration" || var_name == "took")
                && let Some(duration) = self.last_command_duration
            {
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

    /// Spawn a background task to fetch a variable value.
    async fn spawn_variable_task(&self, key: String) {
        let cache = Arc::clone(&self.cache);
        let running = Arc::clone(&self.running_tasks);

        // Get plugin info needed for the task
        let parts: Vec<&str> = key.split(':').collect();
        if parts.len() != 2 {
            return;
        }

        let plugin_name = parts[0];
        let var_name = parts[1];

        let plugin = match self.plugins.get(plugin_name) {
            Some(p) => p.clone(),
            None => return,
        };

        let provider = match plugin.provides.get(var_name) {
            Some(p) => p.clone(),
            None => return,
        };

        // Get cache duration for this variable
        let cache_duration = self.get_variable_cache_duration(&key);

        let key_clone = key.clone();

        let var_name_owned = var_name.to_string();
        let handle = tokio::spawn(async move {
            let result = execute_provider_async(&plugin, &var_name_owned, &provider).await;

            // Update cache based on cache duration setting
            if let Some(ref value) = result {
                let expires_at = match cache_duration {
                    CacheDuration::Always => Some(Instant::now()), // Expires immediately
                    CacheDuration::Never => None,                  // Never expires
                    CacheDuration::Duration(d) => Some(Instant::now() + d),
                };

                let mut cache = cache.lock().await;
                cache.insert(
                    key_clone.clone(),
                    CacheEntry {
                        value: value.clone(),
                        expires_at,
                    },
                );
            }

            // Remove from running tasks
            running.lock().await.remove(&key_clone);

            result
        });

        // Add to running tasks
        self.running_tasks.lock().await.insert(
            key,
            RunningTask {
                handle,
                started_at: Instant::now(),
            },
        );
    }

    /// Try to get a result for a key within a timeout.
    /// Returns the value if the task completes in time, None otherwise.
    async fn try_get_result(&self, key: &str, timeout: Duration) -> Option<String> {
        let task = {
            let mut running = self.running_tasks.lock().await;
            running.remove(key)
        };

        if let Some(task) = task {
            match tokio::time::timeout(timeout, task.handle).await {
                Ok(Ok(result)) => result,
                Ok(Err(_)) => None, // Task panicked
                Err(_) => {
                    // Timeout - task is still running, put it back
                    // Note: We can't put the original task back since we consumed it,
                    // but that's OK - the task continues running in the background
                    // and will update the cache when done
                    None
                }
            }
        } else {
            // Task already completed and removed itself, check cache
            let cache = self.cache.lock().await;
            cache.get(key).map(|e| e.value.clone())
        }
    }

    /// Clean up tasks that have exceeded hard timeout.
    async fn cleanup_stale_tasks(&self) {
        let mut running = self.running_tasks.lock().await;
        let now = Instant::now();

        let stale_keys: Vec<String> = running
            .iter()
            .filter(|(_, task)| now.duration_since(task.started_at) > HARD_TIMEOUT)
            .map(|(key, _)| key.clone())
            .collect();

        for key in stale_keys {
            if let Some(task) = running.remove(&key) {
                task.handle.abort();
            }
        }
    }

    /// Get a variable value from a plugin (synchronous, for backward compatibility).
    ///
    /// Format: "plugin_name:variable_name" (e.g., "git:branch")
    #[allow(dead_code)]
    pub fn get_variable(&mut self, key: &str) -> Option<String> {
        let parts: Vec<&str> = key.split(':').collect();
        if parts.len() != 2 {
            return None;
        }

        let plugin_name = parts[0];
        let var_name = parts[1];

        // Handle context plugin specially (uses nosh-context library)
        // Support both "context" (local) and "builtins/context" (package) names
        if plugin_name == "context" || plugin_name == "builtins/context" {
            return self.get_context_variable(var_name);
        }

        // Get from plugin
        let plugin = self.plugins.get(plugin_name)?;
        let provider = plugin.provides.get(var_name)?;

        let value = self.execute_provider_sync(plugin, var_name, provider)?;

        Some(value)
    }

    /// Execute a variable provider synchronously (for backward compatibility).
    fn execute_provider_sync(
        &self,
        plugin: &Plugin,
        var_name: &str,
        provider: &VariableProvider,
    ) -> Option<String> {
        match provider {
            VariableProvider::Command {
                command, transform, ..
            } => {
                let output = std::process::Command::new("sh")
                    .arg("-c")
                    .arg(command)
                    .output()
                    .ok()?;

                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();

                // Apply transform
                match transform.as_deref() {
                    Some("non_empty") => {
                        if stdout.is_empty() {
                            plugin.icons.get("clean").cloned()
                        } else {
                            plugin.icons.get("dirty").cloned()
                        }
                    }
                    Some("with_icon") => {
                        if stdout.is_empty() {
                            None // Hide entirely when empty
                        } else if let Some(icon) = plugin.icons.get(var_name) {
                            Some(format!("{} {}", icon, stdout))
                        } else {
                            Some(stdout)
                        }
                    }
                    Some("trim") => Some(stdout),
                    _ => Some(stdout),
                }
            }
            VariableProvider::Internal { source } => match source.as_str() {
                "internal" => {
                    if (var_name == "duration" || var_name == "took")
                        && let Some(duration) = self.last_command_duration
                    {
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
                    None
                }
                _ => None,
            },
        }
    }

    /// Get list of loaded plugins with their info.
    pub fn list_plugins(&self) -> Vec<(&str, &str, Vec<&str>)> {
        self.plugins
            .iter()
            .map(|(name, plugin)| {
                let desc = plugin.plugin.description.as_str();
                let vars: Vec<&str> = plugin.provides.keys().map(|s| s.as_str()).collect();
                (name.as_str(), desc, vars)
            })
            .collect()
    }

    /// Debug a plugin by running all its variables and returning results.
    pub async fn debug_plugin(
        &self,
        plugin_name: &str,
    ) -> Option<Vec<(String, String, Result<String, String>)>> {
        let plugin = self.plugins.get(plugin_name)?;
        let mut results = Vec::new();

        for (var_name, provider) in &plugin.provides {
            let (provider_desc, result) = match provider {
                VariableProvider::Command {
                    command,
                    transform,
                    timeout,
                    cache,
                } => {
                    let mut desc = format!("command: {}", command);
                    if let Some(t) = transform {
                        desc.push_str(&format!(" (transform: {})", t));
                    }
                    if let Some(t) = timeout {
                        desc.push_str(&format!(" (timeout: {})", t));
                    }
                    if let Some(c) = cache {
                        desc.push_str(&format!(" (cache: {})", c));
                    }

                    let output = tokio::process::Command::new("sh")
                        .arg("-c")
                        .arg(command)
                        .output()
                        .await;

                    let result = match output {
                        Ok(out) => {
                            if out.status.success() {
                                let stdout =
                                    String::from_utf8_lossy(&out.stdout).trim().to_string();
                                let stderr =
                                    String::from_utf8_lossy(&out.stderr).trim().to_string();
                                if stdout.is_empty() && !stderr.is_empty() {
                                    Err(format!("stderr: {}", stderr))
                                } else if stdout.is_empty() {
                                    Ok("(empty)".to_string())
                                } else {
                                    // Apply transform for display
                                    match transform.as_deref() {
                                        Some("non_empty") => {
                                            let icon = if stdout.is_empty() {
                                                plugin
                                                    .icons
                                                    .get("clean")
                                                    .cloned()
                                                    .unwrap_or_default()
                                            } else {
                                                plugin
                                                    .icons
                                                    .get("dirty")
                                                    .cloned()
                                                    .unwrap_or_default()
                                            };
                                            Ok(format!("{} (raw: {})", icon, stdout))
                                        }
                                        Some("with_icon") => {
                                            if stdout.is_empty() {
                                                Ok("(empty - will be hidden)".to_string())
                                            } else if let Some(icon) = plugin.icons.get(var_name) {
                                                Ok(format!("{} {}", icon, stdout))
                                            } else {
                                                Ok(format!("{} (no icon defined)", stdout))
                                            }
                                        }
                                        _ => Ok(stdout),
                                    }
                                }
                            } else {
                                let stderr =
                                    String::from_utf8_lossy(&out.stderr).trim().to_string();
                                Err(format!(
                                    "exit {}: {}",
                                    out.status.code().unwrap_or(-1),
                                    stderr
                                ))
                            }
                        }
                        Err(e) => Err(format!("failed to run: {}", e)),
                    };

                    (desc, result)
                }
                VariableProvider::Internal { source } => {
                    let desc = format!("internal: {}", source);
                    let result = Ok("(internal variable)".to_string());
                    (desc, result)
                }
            };

            results.push((var_name.clone(), provider_desc, result));
        }

        Some(results)
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Execute a variable provider asynchronously.
async fn execute_provider_async(
    plugin: &Plugin,
    var_name: &str,
    provider: &VariableProvider,
) -> Option<String> {
    match provider {
        VariableProvider::Command {
            command, transform, ..
        } => {
            let output = tokio::process::Command::new("sh")
                .arg("-c")
                .arg(command)
                .output()
                .await
                .ok()?;

            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();

            // Apply transform
            match transform.as_deref() {
                Some("non_empty") => {
                    if stdout.is_empty() {
                        plugin.icons.get("clean").cloned()
                    } else {
                        plugin.icons.get("dirty").cloned()
                    }
                }
                Some("with_icon") => {
                    if stdout.is_empty() {
                        None // Hide entirely when empty
                    } else if let Some(icon) = plugin.icons.get(var_name) {
                        Some(format!("{} {}", icon, stdout))
                    } else {
                        Some(stdout)
                    }
                }
                Some("trim") => Some(stdout),
                _ => Some(stdout),
            }
        }
        VariableProvider::Internal { .. } => {
            // Internal providers should be handled synchronously
            None
        }
    }
}

/// Format a duration for display.
fn format_duration(duration: std::time::Duration) -> String {
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
