//! Configuration path resolution for nosh.
//!
//! Prefers `~/.config/nosh/` with `~/.nosh/` fallback (all OSes).

use std::path::PathBuf;

/// Returns the nosh configuration directory.
///
/// Prefers `~/.config/nosh/` if it exists or if `~/.nosh/` doesn't exist.
/// Falls back to `~/.nosh/` if it exists and `~/.config/nosh/` doesn't.
pub fn nosh_config_dir() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));

    let primary = home.join(".config").join("nosh");
    let fallback = home.join(".nosh");

    if primary.exists() || !fallback.exists() {
        primary
    } else {
        fallback
    }
}

/// Returns the path to the main config file.
/// `~/.config/nosh/config.toml`
pub fn config_file() -> PathBuf {
    nosh_config_dir().join("config.toml")
}

/// Returns the path to the credentials file.
/// `~/.config/nosh/credentials.toml`
pub fn credentials_file() -> PathBuf {
    nosh_config_dir().join("credentials.toml")
}

/// Returns the path to the command history database.
/// `~/.config/nosh/history.db`
pub fn history_db() -> PathBuf {
    nosh_config_dir().join("history.db")
}

/// Returns the path to the legacy history file (for migration).
/// `~/.config/nosh/history`
#[allow(dead_code)]
pub fn history_file_legacy() -> PathBuf {
    nosh_config_dir().join("history")
}

/// Returns the path to the permissions file.
/// `~/.config/nosh/permissions.toml`
pub fn permissions_file() -> PathBuf {
    nosh_config_dir().join("permissions.toml")
}

/// Returns the path to the plugins directory.
/// `~/.config/nosh/plugins/`
pub fn plugins_dir() -> PathBuf {
    nosh_config_dir().join("plugins")
}

/// Returns the path to the themes directory.
/// `~/.config/nosh/themes/`
pub fn themes_dir() -> PathBuf {
    nosh_config_dir().join("themes")
}

/// Returns the path to the shell init script.
/// `~/.config/nosh/init.sh`
pub fn init_file() -> PathBuf {
    nosh_config_dir().join("init.sh")
}
