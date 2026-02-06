//! Bun runtime detection.

use std::path::Path;
use std::process::Command;

use crate::context::ToolInfo;

/// Detect Bun runtime information.
pub fn detect(dir: &Path) -> Option<ToolInfo> {
    // Verify bun project files exist
    let has_bun_lock = dir.join("bun.lockb").exists() || dir.join("bun.lock").exists();
    let has_bunfig = dir.join("bunfig.toml").exists();

    if !has_bun_lock && !has_bunfig {
        return None;
    }

    // Get bun version
    let version = get_bun_version()?;

    Some(ToolInfo { version })
}

/// Get Bun version string.
fn get_bun_version() -> Option<String> {
    let output = Command::new("bun").args(["--version"]).output().ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Parse "1.0.0" directly
    Some(stdout.trim().to_string())
}
