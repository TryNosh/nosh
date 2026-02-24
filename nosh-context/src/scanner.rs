//! One-pass directory scanner for project context detection.

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use crate::context::ProjectContext;
use crate::detectors::{bun, cpp, docker, git, go, node, package, python, rust};

/// Detect project context from a directory.
///
/// This performs a single directory scan and then conditionally
/// parses only the detected project files.
pub fn detect(dir: &Path) -> ProjectContext {
    let dir_str = dir.display().to_string();

    // 1. Single readdir - collect all filenames
    let files = read_dir_names(dir);

    // 2. Check indicators (no I/O, just HashSet lookups)
    let has_cargo = files.contains("Cargo.toml");
    let has_package_json = files.contains("package.json");
    let has_bun =
        files.contains("bun.lockb") || files.contains("bun.lock") || files.contains("bunfig.toml");
    let has_go_mod = files.contains("go.mod");
    let has_python = files.contains("pyproject.toml")
        || files.contains("setup.py")
        || files.contains("requirements.txt");
    let has_cpp = files.contains("CMakeLists.txt")
        || files.contains("meson.build")
        || files.contains("conanfile.txt")
        || files.contains("conanfile.py")
        || files
            .iter()
            .any(|f| f.ends_with(".cpp") || f.ends_with(".cc") || f.ends_with(".cxx"));
    let has_docker = files.contains("Dockerfile")
        || files.contains(".dockerignore")
        || files.contains("docker-compose.yml")
        || files.contains("docker-compose.yaml")
        || files.contains("compose.yml")
        || files.contains("compose.yaml")
        || files.iter().any(|f| f.starts_with("Dockerfile."));
    let has_git = files.contains(".git") || is_in_git_repo(dir);

    // 3. Parse only detected files
    let git_info = if has_git { git::detect(dir) } else { None };
    let package_info = package::detect(dir, &files);
    let rust_info = if has_cargo { rust::detect(dir) } else { None };
    let node_info = if has_package_json {
        node::detect(dir)
    } else {
        None
    };
    let bun_info = if has_bun { bun::detect(dir) } else { None };
    let go_info = if has_go_mod { go::detect(dir) } else { None };
    let python_info = if has_python {
        python::detect(dir)
    } else {
        None
    };
    let cpp_info = if has_cpp {
        cpp::detect(dir, &files)
    } else {
        None
    };
    let docker_info = if has_docker {
        docker::detect(dir, &files)
    } else {
        None
    };

    ProjectContext {
        dir: dir_str,
        git: git_info,
        package: package_info,
        rust: rust_info,
        node: node_info,
        bun: bun_info,
        go: go_info,
        python: python_info,
        cpp: cpp_info,
        docker: docker_info,
    }
}

/// Read all filenames in a directory into a HashSet.
fn read_dir_names(dir: &Path) -> HashSet<String> {
    let mut names = HashSet::new();

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                names.insert(name.to_string());
            }
        }
    }

    names
}

/// Check if we're inside a git repository by looking for .git in parent directories.
fn is_in_git_repo(dir: &Path) -> bool {
    let mut current = dir.to_path_buf();
    loop {
        if current.join(".git").exists() {
            return true;
        }
        if !current.pop() {
            break;
        }
    }
    false
}
