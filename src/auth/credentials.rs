use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Credentials {
    pub token: Option<String>,
    pub email: Option<String>,
}

impl Credentials {
    pub fn load() -> Result<Self> {
        let path = Self::config_path();

        if path.exists() {
            let content = fs::read_to_string(&path)?;
            let creds: Credentials = toml::from_str(&content)?;
            Ok(creds)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        fs::write(&path, &content)?;

        // Set restrictive permissions (owner read/write only)
        #[cfg(unix)]
        {
            let mut perms = fs::metadata(&path)?.permissions();
            perms.set_mode(0o600);
            fs::set_permissions(&path, perms)?;
        }

        Ok(())
    }

    pub fn clear() -> Result<()> {
        let path = Self::config_path();
        if path.exists() {
            fs::remove_file(&path)?;
        }
        Ok(())
    }

    fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("nosh")
            .join("credentials.toml")
    }

    pub fn is_authenticated(&self) -> bool {
        self.token.is_some()
    }
}
