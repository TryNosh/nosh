use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

use crate::paths;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct PermissionStore {
    /// Commands/patterns that are always allowed globally.
    /// Can be a base command (e.g., "rm", "git") or a command with subcommand (e.g., "git log").
    /// - "git" allows all git subcommands (git log, git push, etc.)
    /// - "git log" only allows "git log" specifically
    #[serde(default)]
    pub allowed_commands: HashSet<String>,

    /// Directories where all operations are allowed
    #[serde(default)]
    pub allowed_directories: HashSet<String>,

    /// Command patterns allowed in specific directories.
    /// Key: command pattern (e.g., "rm", "git log")
    /// Value: set of directory paths where the command is allowed
    #[serde(default)]
    pub allowed_command_directories: HashMap<String, HashSet<String>>,

    /// Session-only allowed commands/patterns (not persisted)
    #[serde(skip)]
    session_commands: HashSet<String>,

    /// Session-only allowed directories (not persisted)
    #[serde(skip)]
    session_directories: HashSet<String>,

    /// Session-only command+directory permissions (not persisted)
    #[serde(skip)]
    session_command_directories: HashMap<String, HashSet<String>>,

    #[serde(skip)]
    path: PathBuf,
}

impl PermissionStore {
    pub fn load() -> Result<Self> {
        let path = paths::permissions_file();

        if path.exists() {
            let content = fs::read_to_string(&path)?;
            let mut store: PermissionStore = toml::from_str(&content)?;
            store.path = path;
            Ok(store)
        } else {
            Ok(Self {
                path,
                ..Default::default()
            })
        }
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        fs::write(&self.path, content)?;
        Ok(())
    }

    /// Check if a command pattern is allowed.
    ///
    /// For commands with subcommands (e.g., "git log"):
    /// - Checks if the full pattern "git log" is allowed
    /// - Also checks if the base command "git" is allowed (which allows all subcommands)
    ///
    /// For commands without subcommands (e.g., "rm"):
    /// - Just checks if "rm" is allowed
    pub fn is_command_allowed(&self, command: &str, command_pattern: &str) -> bool {
        // Check if the exact pattern is allowed (e.g., "git log")
        if self.allowed_commands.contains(command_pattern)
            || self.session_commands.contains(command_pattern) {
            return true;
        }

        // Check if the base command is allowed (e.g., "git" allows all git subcommands)
        if command != command_pattern {
            if self.allowed_commands.contains(command)
                || self.session_commands.contains(command) {
                return true;
            }
        }

        false
    }

    /// Legacy method for backward compatibility - checks only base command.
    /// Prefer using is_command_allowed(command, command_pattern) for subcommand support.
    #[allow(dead_code)]
    pub fn is_base_command_allowed(&self, command: &str) -> bool {
        self.allowed_commands.contains(command) || self.session_commands.contains(command)
    }

    pub fn is_directory_allowed(&self, directory: &str) -> bool {
        // Check if this directory or any parent is allowed
        let dir_path = PathBuf::from(directory);

        for allowed in self.allowed_directories.iter().chain(self.session_directories.iter()) {
            let allowed_path = PathBuf::from(allowed);
            if dir_path.starts_with(&allowed_path) {
                return true;
            }
        }
        false
    }

    /// Check if a command pattern is allowed in a specific directory.
    /// This allows for permissions like "rm is allowed in /path/to/project".
    pub fn is_command_allowed_in_directory(
        &self,
        command: &str,
        command_pattern: &str,
        directory: &str,
    ) -> bool {
        self.is_path_allowed_for_command(command, command_pattern, directory)
    }

    /// Check if a path is within allowed directories for a command.
    fn is_path_allowed_for_command(
        &self,
        command: &str,
        command_pattern: &str,
        path: &str,
    ) -> bool {
        // Extract directory from path (for files, get parent; for globs, get base)
        let check_path = if path.contains('*') || path.contains('?') {
            // For globs, extract the non-glob prefix as the directory to check
            let glob_start = path.find(|c| c == '*' || c == '?' || c == '[').unwrap_or(path.len());
            let base = &path[..glob_start].trim_end_matches('/');
            if base.is_empty() {
                PathBuf::from("/")
            } else {
                PathBuf::from(base)
            }
        } else {
            PathBuf::from(path)
        };

        // Check both persisted and session command+directory permissions
        for store in [&self.allowed_command_directories, &self.session_command_directories] {
            // Check exact pattern (e.g., "git log")
            if let Some(dirs) = store.get(command_pattern) {
                for allowed_dir in dirs {
                    let allowed_path = PathBuf::from(allowed_dir);
                    if check_path.starts_with(&allowed_path) {
                        return true;
                    }
                }
            }

            // Check base command (e.g., "git" allows all git subcommands in that dir)
            if command != command_pattern {
                if let Some(dirs) = store.get(command) {
                    for allowed_dir in dirs {
                        let allowed_path = PathBuf::from(allowed_dir);
                        if check_path.starts_with(&allowed_path) {
                            return true;
                        }
                    }
                }
            }
        }

        false
    }

    /// Check if ALL affected paths are allowed for a command.
    /// Returns true only if every path in affected_paths is within an allowed directory.
    /// If affected_paths is empty, falls back to checking the cwd.
    pub fn are_affected_paths_allowed(
        &self,
        command: &str,
        command_pattern: &str,
        affected_paths: &[String],
        cwd: &str,
    ) -> bool {
        if affected_paths.is_empty() {
            // No explicit paths - check cwd
            return self.is_path_allowed_for_command(command, command_pattern, cwd);
        }

        // ALL paths must be allowed
        affected_paths.iter().all(|path| {
            self.is_path_allowed_for_command(command, command_pattern, path)
        })
    }

    /// Allow a command or command pattern.
    ///
    /// The pattern can be:
    /// - A base command like "rm" or "git" (allows all uses of the command)
    /// - A command with subcommand like "git log" (only allows that specific subcommand)
    pub fn allow_command(&mut self, pattern: &str, persist: bool) {
        if persist {
            self.allowed_commands.insert(pattern.to_string());
            let _ = self.save();
        } else {
            self.session_commands.insert(pattern.to_string());
        }
    }

    pub fn allow_directory(&mut self, directory: &str, persist: bool) {
        if persist {
            self.allowed_directories.insert(directory.to_string());
            let _ = self.save();
        } else {
            self.session_directories.insert(directory.to_string());
        }
    }

    /// Allow a command pattern in a specific directory.
    /// E.g., allow "rm" in "/Users/pouya/Projects/nosh"
    pub fn allow_command_in_directory(&mut self, pattern: &str, directory: &str, persist: bool) {
        if persist {
            self.allowed_command_directories
                .entry(pattern.to_string())
                .or_default()
                .insert(directory.to_string());
            let _ = self.save();
        } else {
            self.session_command_directories
                .entry(pattern.to_string())
                .or_default()
                .insert(directory.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_store() -> PermissionStore {
        PermissionStore {
            allowed_commands: HashSet::new(),
            allowed_directories: HashSet::new(),
            allowed_command_directories: HashMap::new(),
            session_commands: HashSet::new(),
            session_directories: HashSet::new(),
            session_command_directories: HashMap::new(),
            path: PathBuf::from("/tmp/test_permissions.toml"),
        }
    }

    #[test]
    fn test_base_command_allows_all_subcommands() {
        let mut store = create_test_store();
        store.allow_command("git", false);

        // Base command "git" should allow all git subcommands
        assert!(store.is_command_allowed("git", "git log"));
        assert!(store.is_command_allowed("git", "git push"));
        assert!(store.is_command_allowed("git", "git commit"));
        assert!(store.is_command_allowed("git", "git")); // Just "git" itself
    }

    #[test]
    fn test_specific_subcommand_only_allows_that_subcommand() {
        let mut store = create_test_store();
        store.allow_command("git log", false);

        // "git log" should only allow "git log"
        assert!(store.is_command_allowed("git", "git log"));
        // But not other git subcommands
        assert!(!store.is_command_allowed("git", "git push"));
        assert!(!store.is_command_allowed("git", "git commit"));
        // And not just "git" itself
        assert!(!store.is_command_allowed("git", "git"));
    }

    #[test]
    fn test_command_without_subcommand() {
        let mut store = create_test_store();
        store.allow_command("rm", false);

        // For commands without subcommands, pattern equals command
        assert!(store.is_command_allowed("rm", "rm"));
    }

    #[test]
    fn test_multiple_patterns() {
        let mut store = create_test_store();
        store.allow_command("git log", false);
        store.allow_command("git status", false);
        store.allow_command("docker", false); // Allow all docker commands

        assert!(store.is_command_allowed("git", "git log"));
        assert!(store.is_command_allowed("git", "git status"));
        assert!(!store.is_command_allowed("git", "git push"));

        assert!(store.is_command_allowed("docker", "docker run"));
        assert!(store.is_command_allowed("docker", "docker ps"));
    }

    #[test]
    fn test_persisted_vs_session_commands() {
        let mut store = create_test_store();

        // Session command
        store.allow_command("git log", false);
        assert!(store.is_command_allowed("git", "git log"));

        // Persisted command (would save to file in real usage)
        store.allowed_commands.insert("cargo build".to_string());
        assert!(store.is_command_allowed("cargo", "cargo build"));
    }

    #[test]
    fn test_backward_compatibility() {
        let mut store = create_test_store();
        store.allow_command("rm", false);

        // Old-style check still works
        assert!(store.is_base_command_allowed("rm"));
        assert!(!store.is_base_command_allowed("git"));
    }

    #[test]
    fn test_command_allowed_in_directory() {
        let mut store = create_test_store();
        store.allow_command_in_directory("rm", "/home/user/project", false);

        // rm is allowed in /home/user/project and subdirs
        assert!(store.is_command_allowed_in_directory("rm", "rm", "/home/user/project"));
        assert!(store.is_command_allowed_in_directory("rm", "rm", "/home/user/project/src"));

        // rm is NOT allowed in other directories
        assert!(!store.is_command_allowed_in_directory("rm", "rm", "/home/user/other"));
        assert!(!store.is_command_allowed_in_directory("rm", "rm", "/home/user"));
    }

    #[test]
    fn test_subcommand_allowed_in_directory() {
        let mut store = create_test_store();
        store.allow_command_in_directory("git push", "/home/user/project", false);

        // "git push" is allowed in project
        assert!(store.is_command_allowed_in_directory("git", "git push", "/home/user/project"));

        // "git pull" is NOT allowed (only push was granted)
        assert!(!store.is_command_allowed_in_directory("git", "git pull", "/home/user/project"));
    }

    #[test]
    fn test_base_command_in_directory_allows_subcommands() {
        let mut store = create_test_store();
        store.allow_command_in_directory("git", "/home/user/project", false);

        // Base "git" permission in dir allows all git subcommands in that dir
        assert!(store.is_command_allowed_in_directory("git", "git push", "/home/user/project"));
        assert!(store.is_command_allowed_in_directory("git", "git pull", "/home/user/project"));
        assert!(store.is_command_allowed_in_directory("git", "git log", "/home/user/project"));

        // But not in other directories
        assert!(!store.is_command_allowed_in_directory("git", "git push", "/home/user/other"));
    }

    #[test]
    fn test_global_vs_directory_permissions() {
        let mut store = create_test_store();

        // Global "git log" permission
        store.allow_command("git log", false);

        // Directory-specific "rm" permission
        store.allow_command_in_directory("rm", "/home/user/project", false);

        // git log works everywhere (global)
        assert!(store.is_command_allowed("git", "git log"));

        // rm only works in the specific directory
        assert!(store.is_command_allowed_in_directory("rm", "rm", "/home/user/project"));
        assert!(!store.is_command_allowed_in_directory("rm", "rm", "/other/path"));

        // rm is not globally allowed
        assert!(!store.is_command_allowed("rm", "rm"));
    }

    #[test]
    fn test_affected_paths_all_must_be_allowed() {
        let mut store = create_test_store();
        store.allow_command_in_directory("rm", "/home/user/project", false);

        // All paths within allowed directory - should pass
        let paths_ok = vec![
            "/home/user/project/file.txt".to_string(),
            "/home/user/project/src/main.rs".to_string(),
        ];
        assert!(store.are_affected_paths_allowed("rm", "rm", &paths_ok, "/home/user/project"));

        // One path outside allowed directory - should fail
        let paths_bad = vec![
            "/home/user/project/file.txt".to_string(),
            "/home/user/other/secret.txt".to_string(),
        ];
        assert!(!store.are_affected_paths_allowed("rm", "rm", &paths_bad, "/home/user/project"));
    }

    #[test]
    fn test_glob_paths_check_base_directory() {
        let mut store = create_test_store();
        store.allow_command_in_directory("rm", "/home/user/project", false);

        // Glob within allowed directory
        let paths_ok = vec!["/home/user/project/logs/*.txt".to_string()];
        assert!(store.are_affected_paths_allowed("rm", "rm", &paths_ok, "/home/user/project"));

        // Glob escaping to parent (../../**/logs resolved to outside)
        let paths_bad = vec!["/home/user/*.txt".to_string()];
        assert!(!store.are_affected_paths_allowed("rm", "rm", &paths_bad, "/home/user/project"));
    }

    #[test]
    fn test_empty_affected_paths_uses_cwd() {
        let mut store = create_test_store();
        store.allow_command_in_directory("rm", "/home/user/project", false);

        // No affected paths - uses cwd
        let empty: Vec<String> = vec![];
        assert!(store.are_affected_paths_allowed("rm", "rm", &empty, "/home/user/project"));
        assert!(!store.are_affected_paths_allowed("rm", "rm", &empty, "/home/user/other"));
    }
}
