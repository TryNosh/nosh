//! Package management for nosh.
//!
//! Handles installing, upgrading, and removing theme/plugin packages from Git repositories.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::process::Command;
use std::time::SystemTime;

use crate::paths;

/// Get current timestamp as a string.
fn get_timestamp() -> String {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

/// A package installed via Git.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    pub name: String,
    pub source: String,
    pub installed_at: String,
    pub last_updated: String,
}

/// Registry of installed packages.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct PackageRegistry {
    #[serde(default)]
    packages: HashMap<String, Package>,
}

impl PackageRegistry {
    /// Load the package registry from disk.
    pub fn load() -> Result<Self> {
        let path = paths::packages_file();
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(&path)?;
        let registry: PackageRegistry = toml::from_str(&content)?;
        Ok(registry)
    }

    /// Save the package registry to disk.
    pub fn save(&self) -> Result<()> {
        let path = paths::packages_file();
        let content = toml::to_string_pretty(self)?;
        fs::write(&path, content)?;
        Ok(())
    }

    /// Add a package to the registry.
    pub fn add(&mut self, package: Package) {
        self.packages.insert(package.name.clone(), package);
    }

    /// Remove a package from the registry.
    pub fn remove(&mut self, name: &str) {
        self.packages.remove(name);
    }

    /// List all packages.
    pub fn list(&self) -> Vec<&Package> {
        self.packages.values().collect()
    }

    /// Check if a package exists.
    pub fn contains(&self, name: &str) -> bool {
        self.packages.contains_key(name)
    }
}

/// Parse an install source into (URL, package name).
///
/// - `user/repo` → `https://github.com/user/repo.git`, `repo`
/// - `https://github.com/user/repo` → `https://github.com/user/repo.git`, `repo`
/// - `https://github.com/user/repo.git` → as-is, `repo`
pub fn parse_install_source(input: &str) -> Result<(String, String)> {
    let input = input.trim();

    if input.is_empty() {
        return Err(anyhow!("Package source cannot be empty"));
    }

    // Check if it's a full URL
    if input.starts_with("https://") || input.starts_with("http://") {
        let mut url = input.to_string();

        // Ensure .git suffix
        if !url.ends_with(".git") {
            url.push_str(".git");
        }

        // Extract repo name from URL
        let name = extract_repo_name(&url)?;
        Ok((url, name))
    } else if input.contains('/') {
        // Assume user/repo format
        let parts: Vec<&str> = input.splitn(2, '/').collect();
        if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
            return Err(anyhow!(
                "Invalid format. Use 'user/repo' or a full URL."
            ));
        }

        let user = parts[0];
        let repo = parts[1];
        let url = format!("https://github.com/{}/{}.git", user, repo);
        Ok((url, repo.to_string()))
    } else {
        Err(anyhow!(
            "Invalid format. Use 'user/repo' or a full URL."
        ))
    }
}

/// Extract the repository name from a Git URL.
fn extract_repo_name(url: &str) -> Result<String> {
    let url = url.trim_end_matches(".git");
    let name = url
        .rsplit('/')
        .next()
        .ok_or_else(|| anyhow!("Could not extract repository name from URL"))?;

    if name.is_empty() {
        return Err(anyhow!("Repository name is empty"));
    }

    Ok(name.to_string())
}

/// Check if git is available.
pub fn check_git_available() -> Result<()> {
    let output = Command::new("git").arg("--version").output();

    match output {
        Ok(out) if out.status.success() => Ok(()),
        _ => Err(anyhow!(
            "Git is not installed. Please install git to use package management."
        )),
    }
}

/// Install a package from a Git repository.
///
/// Returns the package name on success.
pub fn install_package(source: &str) -> Result<String> {
    check_git_available()?;

    let (url, name) = parse_install_source(source)?;

    // Check if already installed
    let mut registry = PackageRegistry::load()?;
    if registry.contains(&name) {
        return Err(anyhow!(
            "Package '{}' is already installed. Use /upgrade to update it.",
            name
        ));
    }

    // Create packages directory if needed
    let packages_dir = paths::packages_dir();
    fs::create_dir_all(&packages_dir)?;

    // Clone the repository
    let target_dir = packages_dir.join(&name);
    let output = Command::new("git")
        .args(["clone", "--depth", "1", &url])
        .arg(&target_dir)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!(
            "Could not clone repository. Check the URL and your internet connection.\n{}",
            stderr.trim()
        ));
    }

    // Register the package
    let timestamp = get_timestamp();

    let package = Package {
        name: name.clone(),
        source: url,
        installed_at: timestamp.clone(),
        last_updated: timestamp,
    };

    registry.add(package);
    registry.save()?;

    Ok(name)
}

/// Upgrade a specific package.
///
/// Returns true if changes were pulled, false if already up to date.
pub fn upgrade_package(name: &str) -> Result<bool> {
    check_git_available()?;

    let mut registry = PackageRegistry::load()?;
    if !registry.contains(name) {
        return Err(anyhow!("Package '{}' is not installed.", name));
    }

    let package_dir = paths::packages_dir().join(name);
    if !package_dir.exists() {
        return Err(anyhow!(
            "Package directory not found. Try reinstalling with /install."
        ));
    }

    // Run git pull
    let output = Command::new("git")
        .args(["pull", "--ff-only"])
        .current_dir(&package_dir)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("Failed to update package: {}", stderr.trim()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let updated = !stdout.contains("Already up to date");

    // Update timestamp in registry
    if updated {
        if let Some(pkg) = registry.packages.get_mut(name) {
            pkg.last_updated = get_timestamp();
        }
        registry.save()?;
    }

    Ok(updated)
}

/// Upgrade all installed packages.
///
/// Returns a list of (package name, was_updated) tuples.
pub fn upgrade_all() -> Result<Vec<(String, bool)>> {
    check_git_available()?;

    let registry = PackageRegistry::load()?;
    let packages: Vec<String> = registry.packages.keys().cloned().collect();

    if packages.is_empty() {
        return Ok(Vec::new());
    }

    let mut results = Vec::new();
    for name in packages {
        match upgrade_package(&name) {
            Ok(updated) => results.push((name, updated)),
            Err(e) => {
                eprintln!("Error upgrading '{}': {}", name, e);
                results.push((name, false));
            }
        }
    }

    Ok(results)
}

/// Remove a package.
pub fn remove_package(name: &str) -> Result<()> {
    let mut registry = PackageRegistry::load()?;
    if !registry.contains(name) {
        return Err(anyhow!("Package '{}' is not installed.", name));
    }

    // Remove the directory
    let package_dir = paths::packages_dir().join(name);
    if package_dir.exists() {
        fs::remove_dir_all(&package_dir)?;
    }

    // Remove from registry
    registry.remove(name);
    registry.save()?;

    Ok(())
}

/// Get info about what a package contains (themes, plugins).
pub fn get_package_contents(name: &str) -> (Vec<String>, Vec<String>) {
    let package_dir = paths::packages_dir().join(name);
    let mut themes = Vec::new();
    let mut plugins = Vec::new();

    // Check for themes
    let themes_dir = package_dir.join("themes");
    if themes_dir.exists() {
        if let Ok(entries) = fs::read_dir(&themes_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "toml") {
                    if let Some(stem) = path.file_stem() {
                        themes.push(stem.to_string_lossy().to_string());
                    }
                }
            }
        }
    }

    // Check for plugins
    let plugins_dir = package_dir.join("plugins");
    if plugins_dir.exists() {
        if let Ok(entries) = fs::read_dir(&plugins_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "toml") {
                    if let Some(stem) = path.file_stem() {
                        plugins.push(stem.to_string_lossy().to_string());
                    }
                }
            }
        }
    }

    (themes, plugins)
}
