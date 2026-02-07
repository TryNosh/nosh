//! Built-in plugins for nosh.
//!
//! These plugins are embedded in the binary and installed on first run.

use anyhow::Result;
use std::fs;
use std::path::Path;

use crate::paths;

/// Embedded built-in plugin files.
pub const GIT_PLUGIN: &str = include_str!("data/git.toml");
pub const EXEC_TIME_PLUGIN: &str = include_str!("data/exec_time.toml");
pub const CONTEXT_PLUGIN: &str = include_str!("data/context.toml");
pub const DEFAULT_THEME: &str = include_str!("data/default_theme.toml");
pub const INIT_SCRIPT: &str = include_str!("data/init.sh");

/// Embedded completion files.
pub const GIT_COMPLETION: &str = include_str!("../completions/data/git.toml");
pub const CARGO_COMPLETION: &str = include_str!("../completions/data/cargo.toml");
pub const NPM_COMPLETION: &str = include_str!("../completions/data/npm.toml");
pub const DOCKER_COMPLETION: &str = include_str!("../completions/data/docker.toml");

/// Install built-in plugins to the user's plugins directory.
pub fn install_builtins() -> Result<()> {
    let plugins_dir = paths::plugins_dir();
    let builtin_dir = plugins_dir.join("builtin");
    let themes_dir = paths::themes_dir();
    let completions_dir = paths::completions_dir();

    // Create directories
    fs::create_dir_all(&builtin_dir)?;
    fs::create_dir_all(&themes_dir)?;
    fs::create_dir_all(&completions_dir)?;

    // Install plugins (only if they don't exist)
    install_if_missing(&builtin_dir.join("git.toml"), GIT_PLUGIN)?;
    install_if_missing(&builtin_dir.join("exec_time.toml"), EXEC_TIME_PLUGIN)?;
    install_if_missing(&builtin_dir.join("context.toml"), CONTEXT_PLUGIN)?;

    // Install default theme
    install_if_missing(&themes_dir.join("default.toml"), DEFAULT_THEME)?;

    // Install init script
    install_if_missing(&paths::init_file(), INIT_SCRIPT)?;

    // Install completions
    install_if_missing(&completions_dir.join("git.toml"), GIT_COMPLETION)?;
    install_if_missing(&completions_dir.join("cargo.toml"), CARGO_COMPLETION)?;
    install_if_missing(&completions_dir.join("npm.toml"), NPM_COMPLETION)?;
    install_if_missing(&completions_dir.join("docker.toml"), DOCKER_COMPLETION)?;

    Ok(())
}

/// Install a file only if it doesn't already exist.
fn install_if_missing(path: &Path, content: &str) -> Result<()> {
    if !path.exists() {
        fs::write(path, content)?;
    }
    Ok(())
}

/// Updatable config file types.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConfigFile {
    Theme,
    GitPlugin,
    ExecTimePlugin,
    ContextPlugin,
    InitScript,
    GitCompletion,
    CargoCompletion,
    NpmCompletion,
    DockerCompletion,
}

impl ConfigFile {
    /// Get the file path for this config file.
    pub fn path(&self) -> std::path::PathBuf {
        match self {
            ConfigFile::Theme => paths::themes_dir().join("default.toml"),
            ConfigFile::GitPlugin => paths::plugins_dir().join("builtin").join("git.toml"),
            ConfigFile::ExecTimePlugin => paths::plugins_dir().join("builtin").join("exec_time.toml"),
            ConfigFile::ContextPlugin => paths::plugins_dir().join("builtin").join("context.toml"),
            ConfigFile::InitScript => paths::init_file(),
            ConfigFile::GitCompletion => paths::completions_dir().join("git.toml"),
            ConfigFile::CargoCompletion => paths::completions_dir().join("cargo.toml"),
            ConfigFile::NpmCompletion => paths::completions_dir().join("npm.toml"),
            ConfigFile::DockerCompletion => paths::completions_dir().join("docker.toml"),
        }
    }

    /// Get the built-in content for this config file.
    pub fn content(&self) -> &'static str {
        match self {
            ConfigFile::Theme => DEFAULT_THEME,
            ConfigFile::GitPlugin => GIT_PLUGIN,
            ConfigFile::ExecTimePlugin => EXEC_TIME_PLUGIN,
            ConfigFile::ContextPlugin => CONTEXT_PLUGIN,
            ConfigFile::InitScript => INIT_SCRIPT,
            ConfigFile::GitCompletion => GIT_COMPLETION,
            ConfigFile::CargoCompletion => CARGO_COMPLETION,
            ConfigFile::NpmCompletion => NPM_COMPLETION,
            ConfigFile::DockerCompletion => DOCKER_COMPLETION,
        }
    }

    /// Get a display name for this config file.
    pub fn display_name(&self) -> &'static str {
        match self {
            ConfigFile::Theme => "Default theme",
            ConfigFile::GitPlugin => "Git plugin",
            ConfigFile::ExecTimePlugin => "Exec time plugin",
            ConfigFile::ContextPlugin => "Context plugin",
            ConfigFile::InitScript => "Init script",
            ConfigFile::GitCompletion => "Git completions",
            ConfigFile::CargoCompletion => "Cargo completions",
            ConfigFile::NpmCompletion => "npm completions",
            ConfigFile::DockerCompletion => "Docker completions",
        }
    }

    /// All updatable config files.
    pub fn all() -> &'static [ConfigFile] {
        &[
            ConfigFile::Theme,
            ConfigFile::GitPlugin,
            ConfigFile::ExecTimePlugin,
            ConfigFile::ContextPlugin,
            ConfigFile::InitScript,
            ConfigFile::GitCompletion,
            ConfigFile::CargoCompletion,
            ConfigFile::NpmCompletion,
            ConfigFile::DockerCompletion,
        ]
    }
}

/// Update a config file to the latest built-in version.
pub fn update_config(file: ConfigFile) -> Result<()> {
    let path = file.path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, file.content())?;
    Ok(())
}

/// Check if a config file differs from the built-in version.
pub fn config_needs_update(file: ConfigFile) -> bool {
    let path = file.path();
    if !path.exists() {
        return true;
    }
    match fs::read_to_string(&path) {
        Ok(content) => content != file.content(),
        Err(_) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedded_files_exist() {
        // Verify all embedded files are non-empty
        assert!(!GIT_PLUGIN.is_empty());
        assert!(!EXEC_TIME_PLUGIN.is_empty());
        assert!(!CONTEXT_PLUGIN.is_empty());
        assert!(!DEFAULT_THEME.is_empty());
        assert!(!INIT_SCRIPT.is_empty());
    }

    #[test]
    fn test_context_plugin_valid_toml() {
        let plugin: Result<crate::plugins::Plugin, _> = toml::from_str(CONTEXT_PLUGIN);
        assert!(plugin.is_ok(), "context.toml should be valid TOML");
        let plugin = plugin.unwrap();
        assert_eq!(plugin.plugin.name, "context");
    }

    #[test]
    fn test_git_plugin_valid_toml() {
        let plugin: Result<crate::plugins::Plugin, _> = toml::from_str(GIT_PLUGIN);
        assert!(plugin.is_ok(), "git.toml should be valid TOML");
        let plugin = plugin.unwrap();
        assert_eq!(plugin.plugin.name, "git");
    }

    #[test]
    fn test_exec_time_plugin_valid_toml() {
        let plugin: Result<crate::plugins::Plugin, _> = toml::from_str(EXEC_TIME_PLUGIN);
        assert!(plugin.is_ok(), "exec_time.toml should be valid TOML");
        let plugin = plugin.unwrap();
        assert_eq!(plugin.plugin.name, "exec_time");
    }

    #[test]
    fn test_default_theme_valid_toml() {
        let theme: Result<crate::plugins::theme::Theme, _> = toml::from_str(DEFAULT_THEME);
        assert!(theme.is_ok(), "default_theme.toml should be valid TOML");
    }

    #[test]
    fn test_init_script_content() {
        // Init script should source ~/.bashrc
        assert!(INIT_SCRIPT.contains(".bashrc"));
    }
}
