//! Generic package information detection.

use std::collections::HashSet;
use std::path::Path;

use crate::context::PackageInfo;
use crate::detectors::{go, node, python, rust};

/// Detect package information from any supported project type.
///
/// Tries project files in order of preference:
/// 1. Cargo.toml (Rust)
/// 2. package.json (Node.js)
/// 3. pyproject.toml (Python)
/// 4. go.mod (Go)
pub fn detect(dir: &Path, files: &HashSet<String>) -> Option<PackageInfo> {
    // Try Rust first
    if files.contains("Cargo.toml") {
        if let Some((name, version)) = rust::get_cargo_package(dir) {
            return Some(PackageInfo { name, version });
        }
    }

    // Try Node.js
    if files.contains("package.json") {
        if let Some((name, version)) = node::get_package_json(dir) {
            return Some(PackageInfo { name, version });
        }
    }

    // Try Python
    if files.contains("pyproject.toml") {
        if let Some((name, version)) = python::get_pyproject(dir) {
            return Some(PackageInfo { name, version });
        }
    }

    // Try Go
    if files.contains("go.mod") {
        if let Some((name, version)) = go::get_go_mod(dir) {
            return Some(PackageInfo { name, version });
        }
    }

    None
}
