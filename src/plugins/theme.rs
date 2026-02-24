//! Theme system for nosh.
//!
//! Handles theme loading, format string expansion, and ANSI color application.

use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

use super::loader::PluginManager;
use crate::paths;

/// ANSI reset escape code.
pub const RESET: &str = "\x1b[0m";

/// A color rule with conditions for conditional coloring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorRule {
    /// Regex pattern to match against the value
    #[serde(default, rename = "match")]
    pub match_pattern: Option<String>,
    /// Check if value contains this string
    #[serde(default)]
    pub contains: Option<String>,
    /// Check if value is empty
    #[serde(default)]
    pub empty: Option<bool>,
    /// Check if value is not empty
    #[serde(default)]
    pub not_empty: Option<bool>,
    /// Check if numeric value is above this threshold
    #[serde(default)]
    pub above: Option<f64>,
    /// Check if numeric value is below this threshold
    #[serde(default)]
    pub below: Option<f64>,
    /// The color to apply if conditions match
    pub color: String,
}

/// A conditional color definition with rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConditionalColor {
    /// Default color if no rules match
    pub default: String,
    /// Rules to evaluate (first matching rule wins)
    #[serde(default)]
    pub rules: Vec<ColorRule>,
}

impl ColorRule {
    /// Check if this rule matches the given value.
    pub fn matches(&self, value: &str) -> bool {
        // Check empty condition
        if let Some(empty) = self.empty
            && empty != value.is_empty()
        {
            return false;
        }

        // Check not_empty condition
        if let Some(not_empty) = self.not_empty
            && not_empty == value.is_empty()
        {
            return false;
        }

        // Check contains condition
        if let Some(ref needle) = self.contains
            && !value.contains(needle)
        {
            return false;
        }

        // Check regex match condition
        if let Some(ref pattern) = self.match_pattern {
            if let Ok(re) = Regex::new(pattern) {
                if !re.is_match(value) {
                    return false;
                }
            } else {
                return false; // Invalid regex doesn't match
            }
        }

        // Check numeric conditions (extract number from value)
        if self.above.is_some() || self.below.is_some() {
            if let Some(num) = extract_number(value) {
                if let Some(above) = self.above
                    && num <= above
                {
                    return false;
                }
                if let Some(below) = self.below
                    && num >= below
                {
                    return false;
                }
            } else {
                // No number found, numeric conditions fail
                return false;
            }
        }

        true
    }
}

impl ConditionalColor {
    /// Resolve the color based on the value.
    pub fn resolve(&self, value: &str) -> &str {
        for rule in &self.rules {
            if rule.matches(value) {
                return &rule.color;
            }
        }
        &self.default
    }
}

/// Extract a number from a string (handles formats like "+5°C", "-10.5°F", "85%").
fn extract_number(s: &str) -> Option<f64> {
    let mut num_str = String::new();
    let mut has_digit = false;
    let mut has_decimal = false;

    for c in s.chars() {
        if c == '-' || c == '+' {
            if num_str.is_empty() {
                num_str.push(c);
            } else {
                break; // Sign in middle means end of number
            }
        } else if c == '.' {
            if !has_decimal {
                num_str.push(c);
                has_decimal = true;
            } else {
                break; // Second decimal means end of number
            }
        } else if c.is_ascii_digit() {
            num_str.push(c);
            has_digit = true;
        } else if has_digit {
            break; // Non-digit after digits means end of number
        }
    }

    if has_digit {
        num_str.parse().ok()
    } else {
        None
    }
}

/// Convert color name or hex to ANSI escape code.
pub fn color_to_ansi(color: &str) -> String {
    // Handle multiple space-separated modifiers (e.g., "blue bold")
    let parts: Vec<&str> = color.split_whitespace().collect();
    let mut codes = Vec::new();

    for part in parts {
        let code = match part.to_lowercase().as_str() {
            "black" => "\x1b[30m",
            "red" => "\x1b[31m",
            "green" => "\x1b[32m",
            "yellow" => "\x1b[33m",
            "blue" => "\x1b[34m",
            "purple" | "magenta" => "\x1b[35m",
            "cyan" => "\x1b[36m",
            "white" => "\x1b[37m",
            "bold" => "\x1b[1m",
            "dim" => "\x1b[2m",
            "italic" => "\x1b[3m",
            "underline" => "\x1b[4m",
            hex if hex.starts_with('#') => {
                codes.push(hex_to_ansi(hex));
                continue;
            }
            _ => "",
        };
        if !code.is_empty() {
            codes.push(code.to_string());
        }
    }

    codes.join("")
}

/// Convert hex color (#RRGGBB) to ANSI 24-bit color escape code.
fn hex_to_ansi(hex: &str) -> String {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return String::new();
    }

    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);

    format!("\x1b[38;2;{};{};{}m", r, g, b)
}

/// A nosh theme configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    /// Parent theme to inherit from (e.g., "builtins/default")
    #[serde(default)]
    pub extends: Option<String>,
    pub prompt: PromptConfig,
    #[serde(default)]
    pub plugins: HashMap<String, PluginConfig>,
    #[serde(default)]
    pub colors: ColorConfig,
}

/// Prompt configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptConfig {
    pub format: String,
    /// Prompt character (default: "❯")
    #[serde(default = "default_prompt_char")]
    pub char: String,
    /// Prompt character shown after failed command (default: "❯")
    #[serde(default = "default_prompt_char")]
    pub char_error: String,
}

fn default_prompt_char() -> String {
    "❯".to_string()
}

/// Per-plugin configuration in the theme.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub style: Option<String>,
    #[serde(default)]
    pub min_ms: Option<u64>,
}

fn default_true() -> bool {
    true
}

/// Color configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ColorConfig {
    // Simple named colors (backward compatible)
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub git_clean: Option<String>,
    #[serde(default)]
    pub git_dirty: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub warning: Option<String>,
    #[serde(default)]
    pub success: Option<String>,
    #[serde(default)]
    pub info: Option<String>,
    #[serde(default)]
    pub ai_command: Option<String>,

    // Conditional colors (new feature)
    #[serde(flatten)]
    pub conditional: HashMap<String, ConditionalColor>,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            extends: None,
            prompt: PromptConfig {
                format: "{cwd_short} $ ".to_string(),
                char: default_prompt_char(),
                char_error: default_prompt_char(),
            },
            plugins: HashMap::new(),
            colors: ColorConfig::default(),
        }
    }
}

impl Theme {
    /// Extract all plugin variable keys from the format string.
    /// Returns keys like "plugin_name:variable_name" for async fetching.
    pub fn get_plugin_variables(&self) -> Vec<String> {
        let mut vars = Vec::new();
        let mut start = 0;
        let format = &self.prompt.format;

        while let Some(open) = format[start..].find('{') {
            let open_idx = start + open;
            if let Some(close) = format[open_idx..].find('}') {
                let close_idx = open_idx + close;
                let var = &format[open_idx + 1..close_idx];

                // Check if it's a plugin variable (contains ':')
                if var.contains(':') {
                    let parts: Vec<&str> = var.split(':').collect();
                    if parts.len() == 2 {
                        let plugin_name = parts[0];
                        // Only include if plugin is enabled
                        if self.is_plugin_enabled(plugin_name) {
                            vars.push(var.to_string());
                        }
                    }
                }

                start = close_idx + 1;
            } else {
                break;
            }
        }

        vars
    }

    /// Format the prompt string using pre-fetched plugin values and built-in variables.
    pub fn format_prompt_with_values(
        &self,
        values: &HashMap<String, String>,
        plugin_manager: &mut PluginManager,
    ) -> String {
        let mut result = self.prompt.format.clone();

        // Expand built-in variables
        result = self.expand_builtin_vars(&result);

        // Expand plugin variables using pre-fetched values
        result = self.expand_plugin_vars_with_values(&result, values, plugin_manager);

        // Apply styled segments [text](color) -> ANSI colored text
        result = self.expand_styled_segments(&result);

        // Clean up empty segments and extra whitespace
        result = self.cleanup_empty_segments(&result);

        result
    }

    /// Expand plugin variables using pre-fetched values.
    fn expand_plugin_vars_with_values(
        &self,
        format: &str,
        values: &HashMap<String, String>,
        plugin_manager: &mut PluginManager,
    ) -> String {
        let mut result = format.to_string();
        let mut start = 0;

        while let Some(open) = result[start..].find('{') {
            let open_idx = start + open;
            if let Some(close) = result[open_idx..].find('}') {
                let close_idx = open_idx + close;
                let var = &result[open_idx + 1..close_idx];

                // Check if it's a plugin variable (contains ':')
                if var.contains(':') {
                    let parts: Vec<&str> = var.split(':').collect();
                    if parts.len() == 2 {
                        let plugin_name = parts[0];

                        // Only expand if plugin is enabled
                        if self.is_plugin_enabled(plugin_name) {
                            // First check pre-fetched values, then fall back to sync get_variable
                            let value = values
                                .get(var)
                                .cloned()
                                .or_else(|| plugin_manager.get_variable(var))
                                .unwrap_or_default();

                            result = format!(
                                "{}{}{}",
                                &result[..open_idx],
                                value,
                                &result[close_idx + 1..]
                            );
                            // Don't advance start, we need to re-check this position
                            continue;
                        } else {
                            // Plugin disabled, remove the placeholder
                            result = format!("{}{}", &result[..open_idx], &result[close_idx + 1..]);
                            continue;
                        }
                    }
                }

                start = close_idx + 1;
            } else {
                break;
            }
        }

        // Clean up multiple spaces
        while result.contains("  ") {
            result = result.replace("  ", " ");
        }

        result
    }

    /// Load a theme by name from the themes directory.
    ///
    /// Supports two formats:
    /// - `mytheme` - loads from `~/.config/nosh/themes/mytheme.toml`
    /// - `package/theme` - loads from `~/.config/nosh/packages/package/themes/theme.toml`
    ///
    /// Themes can inherit from other themes using the `extends` field.
    pub fn load(name: &str) -> Result<Self> {
        Self::load_with_depth(name, 0)
    }

    /// Load a theme with inheritance depth tracking to prevent infinite loops.
    fn load_with_depth(name: &str, depth: usize) -> Result<Self> {
        const MAX_INHERITANCE_DEPTH: usize = 10;
        if depth > MAX_INHERITANCE_DEPTH {
            anyhow::bail!(
                "Theme inheritance too deep (max {}). Check for circular inheritance.",
                MAX_INHERITANCE_DEPTH
            );
        }

        let theme_path = if name.contains('/') {
            // Package theme: package/theme format
            let parts: Vec<&str> = name.splitn(2, '/').collect();
            if parts.len() != 2 {
                anyhow::bail!("Invalid theme format. Use 'package/theme' or 'theme'.");
            }
            let (package_name, theme_name) = (parts[0], parts[1]);
            paths::packages_dir()
                .join(package_name)
                .join("themes")
                .join(format!("{}.toml", theme_name))
        } else {
            // Local theme
            paths::themes_dir().join(format!("{}.toml", name))
        };

        if theme_path.exists() {
            let content = fs::read_to_string(&theme_path)?;
            let mut theme: Theme = toml::from_str(&content)?;

            // Handle inheritance
            if let Some(ref parent_name) = theme.extends.clone() {
                let parent = Self::load_with_depth(parent_name, depth + 1)?;
                theme = theme.merge_with_parent(parent);
            }

            Ok(theme)
        } else if name.contains('/') {
            // Package theme not found - give specific error
            let parts: Vec<&str> = name.splitn(2, '/').collect();
            anyhow::bail!(
                "Theme '{}/{}' not found. Make sure the package is installed with /install",
                parts[0],
                parts[1]
            );
        } else {
            // Return default theme for local themes
            Ok(Theme::default())
        }
    }

    /// Merge this theme with a parent theme. Child values override parent values.
    fn merge_with_parent(mut self, parent: Theme) -> Self {
        // Prompt: use child's values, but keep parent's if child doesn't specify
        // For prompt, we consider empty strings as "not specified"
        if self.prompt.format.is_empty() {
            self.prompt.format = parent.prompt.format;
        }
        if self.prompt.char == default_prompt_char() && parent.prompt.char != default_prompt_char()
        {
            self.prompt.char = parent.prompt.char;
        }
        if self.prompt.char_error == default_prompt_char()
            && parent.prompt.char_error != default_prompt_char()
        {
            self.prompt.char_error = parent.prompt.char_error;
        }

        // Plugins: merge, child overrides parent for same key
        let mut merged_plugins = parent.plugins;
        for (key, value) in self.plugins {
            merged_plugins.insert(key, value);
        }
        self.plugins = merged_plugins;

        // Colors: merge simple colors
        if self.colors.path.is_none() {
            self.colors.path = parent.colors.path;
        }
        if self.colors.git_clean.is_none() {
            self.colors.git_clean = parent.colors.git_clean;
        }
        if self.colors.git_dirty.is_none() {
            self.colors.git_dirty = parent.colors.git_dirty;
        }
        if self.colors.error.is_none() {
            self.colors.error = parent.colors.error;
        }
        if self.colors.warning.is_none() {
            self.colors.warning = parent.colors.warning;
        }
        if self.colors.success.is_none() {
            self.colors.success = parent.colors.success;
        }
        if self.colors.info.is_none() {
            self.colors.info = parent.colors.info;
        }
        if self.colors.ai_command.is_none() {
            self.colors.ai_command = parent.colors.ai_command;
        }

        // Colors: merge conditional colors, child overrides parent for same key
        let mut merged_conditional = parent.colors.conditional;
        for (key, value) in self.colors.conditional {
            merged_conditional.insert(key, value);
        }
        self.colors.conditional = merged_conditional;

        self
    }

    /// Check if a plugin is enabled in this theme.
    pub fn is_plugin_enabled(&self, name: &str) -> bool {
        self.plugins.get(name).map(|p| p.enabled).unwrap_or(true) // Enabled by default
    }

    /// Format the prompt string using plugin variables and built-in variables.
    /// Note: Prefer `format_prompt_with_values` for async operation with pre-fetched values.
    #[allow(dead_code)]
    pub fn format_prompt(&self, plugin_manager: &mut PluginManager) -> String {
        let mut result = self.prompt.format.clone();

        // Expand built-in variables
        result = self.expand_builtin_vars(&result);

        // Expand plugin variables
        result = self.expand_plugin_vars(&result, plugin_manager);

        // Apply styled segments [text](color) -> ANSI colored text
        result = self.expand_styled_segments(&result);

        // Clean up empty segments and extra whitespace
        result = self.cleanup_empty_segments(&result);

        result
    }

    /// Expand built-in variables like {cwd}, {cwd_short}, {user}, {host}, {newline}, {dir}, {prompt:char}.
    fn expand_builtin_vars(&self, format: &str) -> String {
        let mut result = format.to_string();

        // {newline} - line break
        result = result.replace("{newline}", "\n");
        // Also support escaped newlines in TOML multiline strings
        result = result.replace("\\n", "\n");

        // {cwd} - full path
        if result.contains("{cwd}") {
            let cwd = std::env::current_dir()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| "~".to_string());
            result = result.replace("{cwd}", &cwd);
        }

        // {cwd_short} - last component or ~ for home
        if result.contains("{cwd_short}") {
            let cwd_short = self.get_short_dir();
            result = result.replace("{cwd_short}", &cwd_short);
        }

        // {dir} - alias for cwd_short (Starship compatibility)
        if result.contains("{dir}") {
            let dir = self.get_short_dir();
            result = result.replace("{dir}", &dir);
        }

        // {user} - username
        if result.contains("{user}") {
            let user = std::env::var("USER")
                .or_else(|_| std::env::var("USERNAME"))
                .unwrap_or_else(|_| "user".to_string());
            result = result.replace("{user}", &user);
        }

        // {host} - hostname
        if result.contains("{host}") {
            let host = hostname::get()
                .map(|h| h.to_string_lossy().to_string())
                .unwrap_or_else(|_| "localhost".to_string());
            result = result.replace("{host}", &host);
        }

        // {prompt:char} - prompt character
        if result.contains("{prompt:char}") {
            result = result.replace("{prompt:char}", &self.prompt.char);
        }

        result
    }

    /// Get the shortened directory name (last component or ~ for home).
    fn get_short_dir(&self) -> String {
        std::env::current_dir()
            .ok()
            .and_then(|p| {
                // Check if it's the home directory
                if let Some(home) = dirs::home_dir()
                    && p == home
                {
                    return Some("~".to_string());
                }
                p.file_name().map(|s| s.to_string_lossy().to_string())
            })
            .unwrap_or_else(|| "~".to_string())
    }

    /// Expand plugin variables like {git:branch}, {git:dirty}, {exec_time:duration}.
    /// Note: Prefer `expand_plugin_vars_with_values` for async operation with pre-fetched values.
    #[allow(dead_code)]
    fn expand_plugin_vars(&self, format: &str, plugin_manager: &mut PluginManager) -> String {
        let mut result = format.to_string();
        let mut start = 0;

        while let Some(open) = result[start..].find('{') {
            let open_idx = start + open;
            if let Some(close) = result[open_idx..].find('}') {
                let close_idx = open_idx + close;
                let var = &result[open_idx + 1..close_idx];

                // Check if it's a plugin variable (contains ':')
                if var.contains(':') {
                    let parts: Vec<&str> = var.split(':').collect();
                    if parts.len() == 2 {
                        let plugin_name = parts[0];

                        // Only expand if plugin is enabled
                        if self.is_plugin_enabled(plugin_name) {
                            let value = plugin_manager.get_variable(var).unwrap_or_default();

                            result = format!(
                                "{}{}{}",
                                &result[..open_idx],
                                value,
                                &result[close_idx + 1..]
                            );
                            // Don't advance start, we need to re-check this position
                            continue;
                        } else {
                            // Plugin disabled, remove the placeholder
                            result = format!("{}{}", &result[..open_idx], &result[close_idx + 1..]);
                            continue;
                        }
                    }
                }

                start = close_idx + 1;
            } else {
                break;
            }
        }

        // Clean up multiple spaces
        while result.contains("  ") {
            result = result.replace("  ", " ");
        }

        result
    }

    /// Expand styled segments: [content](color) -> ANSI colored content.
    fn expand_styled_segments(&self, format: &str) -> String {
        let re = Regex::new(r"\[([^\]]*)\]\(([^)]+)\)").unwrap();
        re.replace_all(format, |caps: &regex::Captures| {
            let content = &caps[1];
            let color_name = &caps[2];

            // Skip empty content
            if content.is_empty() || content.chars().all(|c| c.is_whitespace()) {
                return String::new();
            }

            // Resolve the color (may be conditional based on content)
            let resolved_color = self.resolve_color(color_name, content);

            format!("{}{}{}", color_to_ansi(&resolved_color), content, RESET)
        })
        .to_string()
    }

    /// Resolve a color name, potentially using conditional color rules.
    fn resolve_color(&self, color_name: &str, content: &str) -> String {
        // Check if it's a conditional color
        if let Some(conditional) = self.colors.conditional.get(color_name) {
            conditional.resolve(content).to_string()
        } else {
            // Return the color name as-is (simple color)
            color_name.to_string()
        }
    }

    /// Clean up empty segments and excessive whitespace.
    fn cleanup_empty_segments(&self, format: &str) -> String {
        let mut result = format.to_string();

        // Remove any remaining empty styled segments (shouldn't happen, but just in case)
        let empty_re = Regex::new(r"\[\s*\]\([^)]+\)").unwrap();
        result = empty_re.replace_all(&result, "").to_string();

        // Clean up multiple spaces (but preserve intentional newlines)
        while result.contains("  ") {
            result = result.replace("  ", " ");
        }

        // Clean up spaces at the start of lines (after newlines)
        let line_start_re = Regex::new(r"\n +").unwrap();
        result = line_start_re.replace_all(&result, "\n").to_string();

        result
    }
}
