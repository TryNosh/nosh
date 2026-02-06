use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;

use crate::paths;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub ai: AiConfig,
    pub behavior: BehaviorConfig,
    pub prompt: PromptConfig,
    pub history: HistoryConfig,
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
    /// Number of recent exchanges to include as context (default: 10)
    pub context_size: usize,
    /// Include command output in context (uses more tokens)
    pub include_output: bool,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HistoryConfig {
    /// Number of recent commands to load on startup for arrow-key navigation.
    /// Full history is always available in SQLite for search.
    pub load_count: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ai: AiConfig::default(),
            behavior: BehaviorConfig::default(),
            prompt: PromptConfig::default(),
            history: HistoryConfig::default(),
        }
    }
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            backend: "ollama".to_string(),
            model: "llama3.2".to_string(),
            ollama_url: "http://localhost:11434".to_string(),
            context_size: 10,
            include_output: false,
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

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            load_count: 200,
        }
    }
}

impl Config {
    /// Check if config file exists (for determining if onboarding is needed)
    pub fn exists() -> bool {
        paths::config_file().exists()
    }

    pub fn load() -> Result<Self> {
        let path = paths::config_file();

        if path.exists() {
            let content = fs::read_to_string(&path)?;
            let config: Config = toml::from_str(&content)?;
            Ok(config)
        } else {
            // Return default but don't save yet - let onboarding handle it
            Ok(Config::default())
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = paths::config_file();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        fs::write(&path, content)?;
        Ok(())
    }
}
