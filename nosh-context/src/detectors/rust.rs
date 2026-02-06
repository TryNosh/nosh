//! Rust project detection.

use std::fs;
use std::path::Path;
use std::process::Command;

use crate::context::ToolInfo;

/// Detect Rust toolchain information.
pub fn detect(dir: &Path) -> Option<ToolInfo> {
    // Verify Cargo.toml exists
    if !dir.join("Cargo.toml").exists() {
        return None;
    }

    // Get rustc version
    let version = get_rustc_version()?;

    Some(ToolInfo { version })
}

/// Get rustc version string.
fn get_rustc_version() -> Option<String> {
    let output = Command::new("rustc").args(["--version"]).output().ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Parse "rustc 1.75.0 (82e1608df 2023-12-21)" -> "1.75.0"
    let version = stdout
        .split_whitespace()
        .nth(1)
        .map(|s| s.to_string())?;

    Some(version)
}

/// Get package info from Cargo.toml.
pub fn get_cargo_package(dir: &Path) -> Option<(String, String)> {
    let cargo_path = dir.join("Cargo.toml");
    let content = fs::read_to_string(cargo_path).ok()?;
    let parsed: toml::Value = toml::from_str(&content).ok()?;

    let package = parsed.get("package")?;
    let name = package.get("name")?.as_str()?.to_string();
    let version = package.get("version")?.as_str()?.to_string();

    Some((name, version))
}
