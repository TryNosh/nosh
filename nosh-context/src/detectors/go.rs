//! Go project detection.

use std::fs;
use std::path::Path;
use std::process::Command;

use crate::context::ToolInfo;

/// Detect Go toolchain information.
pub fn detect(dir: &Path) -> Option<ToolInfo> {
    // Verify go.mod exists
    if !dir.join("go.mod").exists() {
        return None;
    }

    // Get go version
    let version = get_go_version()?;

    Some(ToolInfo { version })
}

/// Get Go version string.
fn get_go_version() -> Option<String> {
    let output = Command::new("go").args(["version"]).output().ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Parse "go version go1.21.5 darwin/arm64" -> "1.21.5"
    let version = stdout
        .split_whitespace()
        .nth(2)
        .and_then(|s| s.strip_prefix("go"))
        .map(|s| s.to_string())?;

    Some(version)
}

/// Get module info from go.mod.
pub fn get_go_mod(dir: &Path) -> Option<(String, String)> {
    let go_mod_path = dir.join("go.mod");
    let content = fs::read_to_string(go_mod_path).ok()?;

    // Parse first line: "module github.com/user/project"
    let first_line = content.lines().next()?;
    let module_name = first_line.strip_prefix("module ")?.trim().to_string();

    // Go modules don't have versions in go.mod, use empty string
    Some((module_name, String::new()))
}
