use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub ai: AiConfig,
    pub behavior: BehaviorConfig,
    pub prompt: PromptConfig,
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

impl Default for Config {
    fn default() -> Self {
        Self {
            ai: AiConfig::default(),
            behavior: BehaviorConfig::default(),
            prompt: PromptConfig::default(),
        }
    }
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            backend: "ollama".to_string(),
            model: "llama3.2".to_string(),
            ollama_url: "http://localhost:11434".to_string(),
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

impl Config {
    /// Check if config file exists (for determining if onboarding is needed)
    pub fn exists() -> bool {
        Self::config_path().exists()
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path();

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
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        fs::write(&path, content)?;
        Ok(())
    }

    fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("nosh")
            .join("config.toml")
    }
}
