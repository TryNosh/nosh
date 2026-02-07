use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;

use crate::paths;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// User has completed or skipped onboarding
    #[serde(default)]
    pub onboarding_complete: bool,
    /// Welcome message shown on startup (empty = no message)
    #[serde(default)]
    pub welcome_message: String,
    pub ai: AiConfig,
    pub behavior: BehaviorConfig,
    pub prompt: PromptConfig,
    pub history: HistoryConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AiConfig {
    /// Number of recent exchanges to include as context (default: 10)
    pub context_size: usize,
    /// Enable agentic mode for investigative queries
    pub agentic_enabled: bool,
    /// Maximum command executions per agentic query
    pub max_iterations: usize,
    /// Timeout in seconds for agentic queries (0 = no timeout)
    pub timeout: u64,
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
            onboarding_complete: false,
            welcome_message: String::new(),
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
            context_size: 10,
            agentic_enabled: true,
            max_iterations: 10,
            timeout: 0, // 0 = no timeout
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
