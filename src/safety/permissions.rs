use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct PermissionStore {
    /// Commands that are always allowed (e.g., "rm", "git")
    #[serde(default)]
    pub allowed_commands: HashSet<String>,

    /// Directories where all operations are allowed
    #[serde(default)]
    pub allowed_directories: HashSet<String>,

    /// Session-only allowed commands (not persisted)
    #[serde(skip)]
    session_commands: HashSet<String>,

    /// Session-only allowed directories (not persisted)
    #[serde(skip)]
    session_directories: HashSet<String>,

    #[serde(skip)]
    path: PathBuf,
}

impl PermissionStore {
    pub fn load() -> Result<Self> {
        let path = Self::config_path();

        if path.exists() {
            let content = fs::read_to_string(&path)?;
            let mut store: PermissionStore = toml::from_str(&content)?;
            store.path = path;
            Ok(store)
        } else {
            Ok(Self {
                path,
                ..Default::default()
            })
        }
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        fs::write(&self.path, content)?;
        Ok(())
    }

    fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("nosh")
            .join("permissions.toml")
    }

    pub fn is_command_allowed(&self, command: &str) -> bool {
        self.allowed_commands.contains(command) || self.session_commands.contains(command)
    }

    pub fn is_directory_allowed(&self, directory: &str) -> bool {
        // Check if this directory or any parent is allowed
        let dir_path = PathBuf::from(directory);

        for allowed in self.allowed_directories.iter().chain(self.session_directories.iter()) {
            let allowed_path = PathBuf::from(allowed);
            if dir_path.starts_with(&allowed_path) {
                return true;
            }
        }
        false
    }

    pub fn allow_command(&mut self, command: &str, persist: bool) {
        if persist {
            self.allowed_commands.insert(command.to_string());
            let _ = self.save();
        } else {
            self.session_commands.insert(command.to_string());
        }
    }

    pub fn allow_directory(&mut self, directory: &str, persist: bool) {
        if persist {
            self.allowed_directories.insert(directory.to_string());
            let _ = self.save();
        } else {
            self.session_directories.insert(directory.to_string());
        }
    }
}
