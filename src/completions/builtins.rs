//! Built-in completers for common completion scenarios.

use std::collections::HashSet;
use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::Completion;

/// Built-in completer types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinCompleter {
    /// All files in current directory
    Files,
    /// Only directories
    Directories,
    /// Commands in PATH
    Executables,
    /// Environment variables
    EnvVars,
    /// System users
    Users,
    /// System groups
    Groups,
    /// SSH known hosts
    Hosts,
    /// Running process names/PIDs
    Processes,
    /// Signal names
    Signals,
}

impl BuiltinCompleter {
    /// Parse a builtin completer name.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "files" => Some(Self::Files),
            "directories" => Some(Self::Directories),
            "executables" => Some(Self::Executables),
            "env_vars" => Some(Self::EnvVars),
            "users" => Some(Self::Users),
            "groups" => Some(Self::Groups),
            "hosts" => Some(Self::Hosts),
            "processes" => Some(Self::Processes),
            "signals" => Some(Self::Signals),
            _ => None,
        }
    }

    /// Get completions for the given prefix.
    pub fn complete(&self, prefix: &str) -> Vec<Completion> {
        match self {
            Self::Files => complete_files(prefix, false),
            Self::Directories => complete_files(prefix, true),
            Self::Executables => complete_executables(prefix),
            Self::EnvVars => complete_env_vars(prefix),
            Self::Users => complete_users(prefix),
            Self::Groups => complete_groups(prefix),
            Self::Hosts => complete_hosts(prefix),
            Self::Processes => complete_processes(prefix),
            Self::Signals => complete_signals(prefix),
        }
    }
}

/// Complete file or directory paths.
fn complete_files(prefix: &str, dirs_only: bool) -> Vec<Completion> {
    let mut completions = Vec::new();

    // Determine the directory and file prefix to search
    let (dir, file_prefix) = if prefix.is_empty() {
        (PathBuf::from("."), String::new())
    } else {
        let path = Path::new(prefix);
        if prefix.ends_with('/') || prefix.ends_with(std::path::MAIN_SEPARATOR) {
            (path.to_path_buf(), String::new())
        } else if path.is_dir() && !prefix.ends_with('.') {
            // Ambiguous: could be completing inside dir or completing the dir name
            // Try completing inside the directory
            (path.to_path_buf(), String::new())
        } else {
            let parent = path.parent().unwrap_or(Path::new("."));
            // Empty parent means current directory
            let parent = if parent.as_os_str().is_empty() {
                Path::new(".")
            } else {
                parent
            };
            let file_name = path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            (parent.to_path_buf(), file_name)
        }
    };

    // Expand tilde
    let dir = expand_tilde(&dir);

    // Read directory entries
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();

            // Skip hidden files unless prefix starts with dot
            if name.starts_with('.') && !file_prefix.starts_with('.') {
                continue;
            }

            // Check if name matches prefix
            if !name.starts_with(&file_prefix) {
                continue;
            }

            // Check if directory-only filter applies
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if dirs_only && !is_dir {
                continue;
            }

            // Build the completion text
            let mut completion_text = if prefix.is_empty() || prefix == "." {
                name.clone()
            } else if prefix.ends_with('/') || prefix.ends_with(std::path::MAIN_SEPARATOR) {
                format!("{}{}", prefix, name)
            } else if let Some(parent) = Path::new(prefix).parent() {
                if parent == Path::new("") {
                    name.clone()
                } else {
                    format!("{}/{}", parent.display(), name)
                }
            } else {
                name.clone()
            };

            // Add trailing slash for directories
            if is_dir && !completion_text.ends_with('/') {
                completion_text.push('/');
            }

            let desc = if is_dir { "directory" } else { "file" };
            completions.push(Completion::new(completion_text).with_description(desc));
        }
    }

    completions.sort_by(|a, b| a.text.cmp(&b.text));
    completions
}

/// Expand ~ to home directory.
fn expand_tilde(path: &Path) -> PathBuf {
    if path.starts_with("~") {
        if let Some(home) = dirs::home_dir() {
            let rest = path.strip_prefix("~").unwrap();
            return home.join(rest);
        }
    }
    path.to_path_buf()
}

/// Complete executable commands from PATH.
fn complete_executables(prefix: &str) -> Vec<Completion> {
    let mut completions = Vec::new();
    let mut seen = HashSet::new();

    if let Some(path_var) = env::var_os("PATH") {
        for dir in env::split_paths(&path_var) {
            if let Ok(entries) = fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();

                    // Check prefix match
                    if !name.starts_with(prefix) {
                        continue;
                    }

                    // Skip duplicates
                    if seen.contains(&name) {
                        continue;
                    }

                    // Check if executable
                    if let Ok(metadata) = entry.metadata() {
                        let mode = metadata.permissions().mode();
                        if mode & 0o111 != 0 {
                            seen.insert(name.clone());
                            completions.push(Completion::new(name).with_description("command"));
                        }
                    }
                }
            }
        }
    }

    completions.sort_by(|a, b| a.text.cmp(&b.text));
    completions
}

/// Complete environment variable names.
fn complete_env_vars(prefix: &str) -> Vec<Completion> {
    let prefix = prefix.strip_prefix('$').unwrap_or(prefix);
    let mut completions: Vec<_> = env::vars()
        .filter(|(name, _)| name.starts_with(prefix))
        .map(|(name, value)| {
            let display_val = if value.len() > 30 {
                format!("{}...", &value[..27])
            } else {
                value
            };
            Completion::new(format!("${}", name)).with_description(display_val)
        })
        .collect();

    completions.sort_by(|a, b| a.text.cmp(&b.text));
    completions
}

/// Complete system users.
fn complete_users(prefix: &str) -> Vec<Completion> {
    let mut completions = Vec::new();

    // Read /etc/passwd on Unix systems
    if let Ok(content) = fs::read_to_string("/etc/passwd") {
        for line in content.lines() {
            if let Some(user) = line.split(':').next() {
                if user.starts_with(prefix) {
                    completions.push(Completion::new(user).with_description("user"));
                }
            }
        }
    }

    completions.sort_by(|a, b| a.text.cmp(&b.text));
    completions
}

/// Complete system groups.
fn complete_groups(prefix: &str) -> Vec<Completion> {
    let mut completions = Vec::new();

    // Read /etc/group on Unix systems
    if let Ok(content) = fs::read_to_string("/etc/group") {
        for line in content.lines() {
            if let Some(group) = line.split(':').next() {
                if group.starts_with(prefix) {
                    completions.push(Completion::new(group).with_description("group"));
                }
            }
        }
    }

    completions.sort_by(|a, b| a.text.cmp(&b.text));
    completions
}

/// Complete SSH known hosts.
fn complete_hosts(prefix: &str) -> Vec<Completion> {
    let mut completions = Vec::new();
    let mut seen = HashSet::new();

    // Read ~/.ssh/known_hosts
    if let Some(home) = dirs::home_dir() {
        let known_hosts = home.join(".ssh").join("known_hosts");
        if let Ok(content) = fs::read_to_string(known_hosts) {
            for line in content.lines() {
                // Skip comments and empty lines
                if line.starts_with('#') || line.trim().is_empty() {
                    continue;
                }

                // First field is the host(s), may be comma-separated
                if let Some(hosts_field) = line.split_whitespace().next() {
                    for host in hosts_field.split(',') {
                        // Skip hashed hosts (start with |)
                        if host.starts_with('|') {
                            continue;
                        }

                        // Remove [host]:port format brackets
                        let host = host
                            .trim_start_matches('[')
                            .split(']')
                            .next()
                            .unwrap_or(host);

                        if host.starts_with(prefix) && seen.insert(host.to_string()) {
                            completions.push(Completion::new(host).with_description("host"));
                        }
                    }
                }
            }
        }
    }

    // Also read /etc/hosts
    if let Ok(content) = fs::read_to_string("/etc/hosts") {
        for line in content.lines() {
            if line.starts_with('#') || line.trim().is_empty() {
                continue;
            }

            // Skip IP address, get hostname(s)
            for host in line.split_whitespace().skip(1) {
                if host.starts_with(prefix) && seen.insert(host.to_string()) {
                    completions.push(Completion::new(host).with_description("host"));
                }
            }
        }
    }

    completions.sort_by(|a, b| a.text.cmp(&b.text));
    completions
}

/// Complete running process names.
fn complete_processes(prefix: &str) -> Vec<Completion> {
    let mut completions = Vec::new();
    let mut seen = HashSet::new();

    // Use ps command to get process list
    if let Ok(output) = Command::new("ps").args(["-axo", "pid,comm"]).output() {
        if let Ok(stdout) = String::from_utf8(output.stdout) {
            for line in stdout.lines().skip(1) {
                // Skip header
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let pid = parts[0];
                    let name = parts[1..].join(" ");

                    // Extract just the command name (not full path)
                    let short_name = Path::new(&name)
                        .file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or(name.clone());

                    // Match by name
                    if short_name.starts_with(prefix) && seen.insert(short_name.clone()) {
                        completions
                            .push(Completion::new(&short_name).with_description(format!("pid {}", pid)));
                    }

                    // Match by PID
                    if pid.starts_with(prefix) {
                        completions.push(Completion::new(pid).with_description(&short_name));
                    }
                }
            }
        }
    }

    completions.sort_by(|a, b| a.text.cmp(&b.text));
    completions
}

/// Complete signal names.
fn complete_signals(prefix: &str) -> Vec<Completion> {
    const SIGNALS: &[(&str, &str)] = &[
        ("SIGHUP", "Hangup"),
        ("SIGINT", "Interrupt"),
        ("SIGQUIT", "Quit"),
        ("SIGILL", "Illegal instruction"),
        ("SIGTRAP", "Trace trap"),
        ("SIGABRT", "Abort"),
        ("SIGBUS", "Bus error"),
        ("SIGFPE", "Floating point exception"),
        ("SIGKILL", "Kill"),
        ("SIGUSR1", "User defined signal 1"),
        ("SIGSEGV", "Segmentation fault"),
        ("SIGUSR2", "User defined signal 2"),
        ("SIGPIPE", "Broken pipe"),
        ("SIGALRM", "Alarm clock"),
        ("SIGTERM", "Termination"),
        ("SIGCHLD", "Child status changed"),
        ("SIGCONT", "Continue"),
        ("SIGSTOP", "Stop"),
        ("SIGTSTP", "Terminal stop"),
        ("SIGTTIN", "Background read"),
        ("SIGTTOU", "Background write"),
        ("SIGURG", "Urgent data"),
        ("SIGXCPU", "CPU time limit"),
        ("SIGXFSZ", "File size limit"),
        ("SIGVTALRM", "Virtual timer"),
        ("SIGPROF", "Profiling timer"),
        ("SIGWINCH", "Window size change"),
        ("SIGIO", "I/O possible"),
        ("SIGSYS", "Bad system call"),
    ];

    let prefix_upper = prefix.to_uppercase();
    let prefix_no_sig = prefix_upper.strip_prefix("SIG").unwrap_or(&prefix_upper);

    SIGNALS
        .iter()
        .filter(|(name, _)| {
            name.starts_with(&prefix_upper) || name.strip_prefix("SIG").unwrap().starts_with(prefix_no_sig)
        })
        .map(|(name, desc)| Completion::new(*name).with_description(*desc))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_from_name() {
        assert_eq!(BuiltinCompleter::from_name("files"), Some(BuiltinCompleter::Files));
        assert_eq!(BuiltinCompleter::from_name("directories"), Some(BuiltinCompleter::Directories));
        assert_eq!(BuiltinCompleter::from_name("unknown"), None);
    }

    #[test]
    fn test_complete_env_vars() {
        // PATH should always exist
        let completions = complete_env_vars("PAT");
        assert!(completions.iter().any(|c| c.text == "$PATH"));
    }

    #[test]
    fn test_complete_signals() {
        let completions = complete_signals("SIGK");
        assert!(completions.iter().any(|c| c.text == "SIGKILL"));
    }
}
