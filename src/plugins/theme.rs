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
}

impl Default for Theme {
    fn default() -> Self {
        Self {
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
    /// Load a theme by name from the themes directory.
    pub fn load(name: &str) -> Result<Self> {
        let themes_dir = paths::themes_dir();
        let theme_path = themes_dir.join(format!("{}.toml", name));

        if theme_path.exists() {
            let content = fs::read_to_string(&theme_path)?;
            let theme: Theme = toml::from_str(&content)?;
            Ok(theme)
        } else {
            // Return default theme
            Ok(Theme::default())
        }
    }

    /// Check if a plugin is enabled in this theme.
    pub fn is_plugin_enabled(&self, name: &str) -> bool {
        self.plugins
            .get(name)
            .map(|p| p.enabled)
            .unwrap_or(true) // Enabled by default
    }

    /// Format the prompt string using plugin variables and built-in variables.
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
                if let Some(home) = dirs::home_dir() {
                    if p == home {
                        return Some("~".to_string());
                    }
                }
                p.file_name().map(|s| s.to_string_lossy().to_string())
            })
            .unwrap_or_else(|| "~".to_string())
    }

    /// Expand plugin variables like {git:branch}, {git:dirty}, {exec_time:duration}.
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
                            let value = plugin_manager
                                .get_variable(var)
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
                            result = format!(
                                "{}{}",
                                &result[..open_idx],
                                &result[close_idx + 1..]
                            );
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
            let color = &caps[2];

            // Skip empty content
            if content.is_empty() || content.chars().all(|c| c.is_whitespace()) {
                return String::new();
            }

            format!("{}{}{}", color_to_ansi(color), content, RESET)
        })
        .to_string()
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

