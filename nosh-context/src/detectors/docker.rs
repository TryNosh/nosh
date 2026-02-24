//! Docker project detection.

use std::collections::HashSet;
use std::path::Path;
use std::process::Command;

use crate::context::ToolInfo;

/// Detect Docker toolchain information.
pub fn detect(_dir: &Path, files: &HashSet<String>) -> Option<ToolInfo> {
    // Check for Docker project indicators
    let has_dockerfile =
        files.contains("Dockerfile") || files.iter().any(|f| f.starts_with("Dockerfile."));
    let has_compose = files.contains("docker-compose.yml")
        || files.contains("docker-compose.yaml")
        || files.contains("compose.yml")
        || files.contains("compose.yaml");
    let has_dockerignore = files.contains(".dockerignore");

    if !has_dockerfile && !has_compose && !has_dockerignore {
        return None;
    }

    // Get docker version
    let version = get_docker_version()?;

    Some(ToolInfo { version })
}

/// Get Docker version string.
fn get_docker_version() -> Option<String> {
    let output = Command::new("docker").args(["--version"]).output().ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Parse "Docker version 24.0.7, build afdd53b"
    let version = stdout
        .split(',')
        .next()?
        .trim()
        .strip_prefix("Docker version ")?
        .to_string();

    Some(version)
}
