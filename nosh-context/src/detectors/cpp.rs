//! C++ project detection.

use std::collections::HashSet;
use std::path::Path;
use std::process::Command;

use crate::context::ToolInfo;

/// Detect C++ toolchain information.
pub fn detect(_dir: &Path, files: &HashSet<String>) -> Option<ToolInfo> {
    // Check for C++ project indicators
    let has_cmake = files.contains("CMakeLists.txt");
    let has_makefile = files.contains("Makefile") || files.contains("makefile");
    let has_cpp_files = files.iter().any(|f| {
        f.ends_with(".cpp")
            || f.ends_with(".cc")
            || f.ends_with(".cxx")
            || f.ends_with(".hpp")
            || f.ends_with(".hxx")
    });
    let has_meson = files.contains("meson.build");
    let has_conan = files.contains("conanfile.txt") || files.contains("conanfile.py");

    if !has_cmake && !has_cpp_files && !has_meson && !has_conan && !has_makefile {
        return None;
    }

    // For Makefile-only projects, verify there are actually C++ files
    if has_makefile && !has_cmake && !has_cpp_files && !has_meson && !has_conan {
        return None;
    }

    // Get compiler version
    let version = get_cpp_version()?;

    Some(ToolInfo { version })
}

/// Get C++ compiler version string.
fn get_cpp_version() -> Option<String> {
    // Try clang++ first (common on macOS), then g++
    if let Some(version) = get_clang_version() {
        return Some(version);
    }

    get_gpp_version()
}

fn get_clang_version() -> Option<String> {
    let output = Command::new("clang++")
        .args(["--version"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Parse "Apple clang version 15.0.0" or "clang version 17.0.0"
    for line in stdout.lines() {
        if line.contains("clang version") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            for (i, part) in parts.iter().enumerate() {
                if *part == "version" && i + 1 < parts.len() {
                    return Some(format!("clang {}", parts[i + 1]));
                }
            }
        }
    }
    None
}

fn get_gpp_version() -> Option<String> {
    let output = Command::new("g++").args(["--version"]).output().ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Parse "g++ (GCC) 13.2.0" or similar
    let first_line = stdout.lines().next()?;
    let parts: Vec<&str> = first_line.split_whitespace().collect();
    // Last part is usually the version
    parts.last().map(|v| format!("g++ {}", v))
}
