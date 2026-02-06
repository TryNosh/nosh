//! Python project detection.

use std::fs;
use std::path::Path;
use std::process::Command;

use crate::context::ToolInfo;

/// Detect Python toolchain information.
pub fn detect(dir: &Path) -> Option<ToolInfo> {
    // Verify python project files exist
    let has_pyproject = dir.join("pyproject.toml").exists();
    let has_setup = dir.join("setup.py").exists();
    let has_requirements = dir.join("requirements.txt").exists();

    if !has_pyproject && !has_setup && !has_requirements {
        return None;
    }

    // Get python version
    let version = get_python_version()?;

    Some(ToolInfo { version })
}

/// Get Python version string.
fn get_python_version() -> Option<String> {
    // Try python3 first, then python
    let output = Command::new("python3")
        .args(["--version"])
        .output()
        .or_else(|_| Command::new("python").args(["--version"]).output())
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Parse "Python 3.11.6" -> "3.11.6"
    let version = stdout
        .split_whitespace()
        .nth(1)
        .map(|s| s.to_string())?;

    Some(version)
}

/// Get package info from pyproject.toml.
pub fn get_pyproject(dir: &Path) -> Option<(String, String)> {
    let pyproject_path = dir.join("pyproject.toml");
    let content = fs::read_to_string(pyproject_path).ok()?;
    let parsed: toml::Value = toml::from_str(&content).ok()?;

    // Try [project] section first (PEP 621)
    if let Some(project) = parsed.get("project") {
        let name = project.get("name")?.as_str()?.to_string();
        let version = project
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("0.0.0")
            .to_string();
        return Some((name, version));
    }

    // Try [tool.poetry] section
    if let Some(tool) = parsed.get("tool") {
        if let Some(poetry) = tool.get("poetry") {
            let name = poetry.get("name")?.as_str()?.to_string();
            let version = poetry
                .get("version")
                .and_then(|v| v.as_str())
                .unwrap_or("0.0.0")
                .to_string();
            return Some((name, version));
        }
    }

    None
}
