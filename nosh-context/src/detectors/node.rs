//! Node.js project detection.

use std::fs;
use std::path::Path;
use std::process::Command;

use crate::context::ToolInfo;

/// Detect Node.js toolchain information.
pub fn detect(dir: &Path) -> Option<ToolInfo> {
    // Verify package.json exists
    if !dir.join("package.json").exists() {
        return None;
    }

    // Get node version
    let version = get_node_version()?;

    Some(ToolInfo { version })
}

/// Get Node.js version string.
fn get_node_version() -> Option<String> {
    let output = Command::new("node").args(["--version"]).output().ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Parse "v20.10.0" -> "20.10.0"
    let version = stdout.trim().trim_start_matches('v').to_string();

    Some(version)
}

/// Get package info from package.json.
pub fn get_package_json(dir: &Path) -> Option<(String, String)> {
    let package_path = dir.join("package.json");
    let content = fs::read_to_string(package_path).ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&content).ok()?;

    let name = parsed.get("name")?.as_str()?.to_string();
    let version = parsed.get("version")?.as_str()?.to_string();

    Some((name, version))
}
