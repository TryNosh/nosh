use std::env;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub enum RiskLevel {
    Safe,       // echo, pwd, ls (no writes)
    Low,        // single file write, git operations
    Medium,     // glob deletes, recursive operations
    High,       // sudo, system modifications
    Critical,   // rm -rf ~, rm -rf /, curl | sh
    Blocked,    // absolutely never allow
}

/// Commands that have subcommands (e.g., "git log", "docker run")
const COMMANDS_WITH_SUBCOMMANDS: &[&str] = &[
    "git", "docker", "cargo", "npm", "npx", "yarn", "pnpm",
    "kubectl", "brew", "apt", "apt-get", "dnf", "yum", "pacman",
    "systemctl", "journalctl", "ip", "az", "aws", "gcloud",
    "terraform", "helm", "podman", "minikube", "kind",
    "go", "rustup", "pip", "poetry", "uv", "conda",
];

#[derive(Debug, Clone)]
pub struct CommandInfo {
    pub command: String,
    /// The subcommand if applicable (e.g., "log" for "git log")
    pub subcommand: Option<String>,
    /// Combined command pattern for permission matching (e.g., "git log" or just "rm")
    pub command_pattern: String,
    #[allow(dead_code)]
    pub args: Vec<String>,
    pub is_destructive: bool,
    pub is_network: bool,
    pub is_privileged: bool,
    #[allow(dead_code)]
    pub affected_paths: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ParsedCommand {
    pub raw: String,
    pub info: CommandInfo,
    pub risk_level: RiskLevel,
    pub risk_reason: String,
}

const DESTRUCTIVE_COMMANDS: &[&str] = &["rm", "rmdir", "mv", "unlink"];
const NETWORK_COMMANDS: &[&str] = &["curl", "wget", "ssh", "scp", "rsync", "nc", "netcat"];
const PRIVILEGED_COMMANDS: &[&str] = &["sudo", "su", "doas"];
const SAFE_COMMANDS: &[&str] = &[
    "echo", "pwd", "ls", "cat", "head", "tail", "grep", "find", "which", "whereis",
    "whoami", "date", "cal", "uptime", "hostname", "uname", "env", "printenv",
    "wc", "sort", "uniq", "diff", "less", "more", "file", "stat", "tree",
];

/// Resolve a path argument to an absolute path.
/// For glob patterns, resolves the base directory portion.
/// E.g., "../../**/logs" -> "/resolved/base/**/logs"
fn resolve_path(path: &str) -> String {
    // Find where glob patterns start
    let glob_chars = ['*', '?', '['];
    let glob_start = path.find(|c| glob_chars.contains(&c));

    match glob_start {
        Some(0) => {
            // Glob at start (e.g., "*.txt") - relative to cwd
            if let Ok(cwd) = env::current_dir() {
                format!("{}/{}", cwd.display(), path)
            } else {
                path.to_string()
            }
        }
        Some(pos) => {
            // Glob in middle (e.g., "../logs/*.txt" or "../../**/logs")
            // Resolve the base path before the glob
            let (base, glob_part) = path.split_at(pos);
            let base_trimmed = base.trim_end_matches('/');

            if base_trimmed.is_empty() {
                return path.to_string();
            }

            let base_path = PathBuf::from(base_trimmed);
            if let Ok(resolved) = base_path.canonicalize() {
                format!("{}/{}", resolved.display(), glob_part)
            } else if let Ok(cwd) = env::current_dir() {
                // Path doesn't exist yet, but resolve relative parts
                let joined = cwd.join(base_trimmed);
                format!("{}/{}", normalize_path(&joined).display(), glob_part)
            } else {
                path.to_string()
            }
        }
        None => {
            // No glob, just a regular path
            let p = PathBuf::from(path);
            if p.is_absolute() {
                path.to_string()
            } else if let Ok(resolved) = p.canonicalize() {
                resolved.to_string_lossy().to_string()
            } else if let Ok(cwd) = env::current_dir() {
                normalize_path(&cwd.join(path)).to_string_lossy().to_string()
            } else {
                path.to_string()
            }
        }
    }
}

/// Normalize a path by resolving `.` and `..` components without requiring the path to exist.
fn normalize_path(path: &Path) -> PathBuf {
    let mut components = Vec::new();

    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                components.pop();
            }
            std::path::Component::CurDir => {}
            c => components.push(c),
        }
    }

    components.iter().collect()
}

pub fn parse_command(raw: &str) -> ParsedCommand {
    let words = shell_words::split(raw).unwrap_or_else(|_| vec![raw.to_string()]);

    let (command, args) = if words.is_empty() {
        (String::new(), vec![])
    } else {
        (words[0].clone(), words[1..].to_vec())
    };

    // Extract subcommand for commands that have subcommands
    let (subcommand, command_pattern) = extract_subcommand(&command, &args);

    let is_destructive = DESTRUCTIVE_COMMANDS.contains(&command.as_str());
    let is_network = NETWORK_COMMANDS.contains(&command.as_str());
    let is_privileged = PRIVILEGED_COMMANDS.contains(&command.as_str());

    let affected_paths: Vec<String> = args
        .iter()
        .filter(|a| !a.starts_with('-'))
        .filter(|a| Path::new(a).exists() || a.contains('*') || a.contains('/'))
        .map(|a| resolve_path(a))
        .collect();

    let info = CommandInfo {
        command: command.clone(),
        subcommand,
        command_pattern,
        args: args.clone(),
        is_destructive,
        is_network,
        is_privileged,
        affected_paths,
    };

    let (risk_level, risk_reason) = assess_risk(&command, &args, &info);

    ParsedCommand {
        raw: raw.to_string(),
        info,
        risk_level,
        risk_reason,
    }
}

/// Extract subcommand from commands that support subcommands.
/// Returns (subcommand, command_pattern) where:
/// - subcommand is Some("log") for "git log -5"
/// - command_pattern is "git log" for "git log -5" or just "rm" for "rm file.txt"
fn extract_subcommand(command: &str, args: &[String]) -> (Option<String>, String) {
    // Check if this command has subcommands
    if !COMMANDS_WITH_SUBCOMMANDS.contains(&command) {
        return (None, command.to_string());
    }

    // Find the first non-flag argument as the subcommand
    for arg in args {
        // Skip flags (arguments starting with -)
        if arg.starts_with('-') {
            continue;
        }
        // Found a subcommand
        let subcommand = arg.clone();
        let command_pattern = format!("{} {}", command, subcommand);
        return (Some(subcommand), command_pattern);
    }

    // Command has subcommands but none provided (e.g., just "git" or "docker")
    (None, command.to_string())
}

fn assess_risk(command: &str, args: &[String], info: &CommandInfo) -> (RiskLevel, String) {
    // Check for blocked patterns
    if is_blocked(command, args) {
        return (RiskLevel::Blocked, "This command is blocked for safety".to_string());
    }

    // Check for critical patterns
    if let Some(reason) = is_critical(command, args) {
        return (RiskLevel::Critical, reason);
    }

    // Privileged commands
    if info.is_privileged {
        return (RiskLevel::High, format!("Requires elevated privileges ({})", command));
    }

    // Network commands
    if info.is_network {
        return (RiskLevel::Medium, format!("Network operation ({})", command));
    }

    // Destructive commands
    if info.is_destructive {
        let has_recursive = args.iter().any(|a| a.contains('r') && a.starts_with('-'));
        let has_force = args.iter().any(|a| a.contains('f') && a.starts_with('-'));

        if has_recursive && has_force {
            return (RiskLevel::Medium, "Recursive forced delete".to_string());
        }
        if has_recursive {
            return (RiskLevel::Medium, "Recursive delete".to_string());
        }
        return (RiskLevel::Low, "File deletion".to_string());
    }

    // Safe commands
    if SAFE_COMMANDS.contains(&command) {
        return (RiskLevel::Safe, "Read-only operation".to_string());
    }

    // Default to low risk for unknown commands
    (RiskLevel::Low, "Unknown command".to_string())
}

fn is_blocked(command: &str, args: &[String]) -> bool {
    // rm -rf / or rm -rf /*
    if command == "rm" {
        let has_rf = args.iter().any(|a| a.starts_with('-') && a.contains('r') && a.contains('f'));
        let targets_root = args.iter().any(|a| a == "/" || a == "/*");
        if has_rf && targets_root {
            return true;
        }
    }
    false
}

fn is_critical(command: &str, args: &[String]) -> Option<String> {
    // rm -rf on home or other dangerous paths
    if command == "rm" {
        let has_rf = args.iter().any(|a| a.starts_with('-') && a.contains('r') && a.contains('f'));
        if has_rf {
            for arg in args {
                if arg == "~" || arg == "$HOME" || arg.starts_with("~/") && arg.len() <= 3 {
                    return Some("Recursive delete on home directory".to_string());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_command() {
        let parsed = parse_command("ls -la");
        assert_eq!(parsed.risk_level, RiskLevel::Safe);
    }

    #[test]
    fn test_rm_single_file() {
        let parsed = parse_command("rm temp.txt");
        assert_eq!(parsed.risk_level, RiskLevel::Low);
    }

    #[test]
    fn test_rm_rf() {
        let parsed = parse_command("rm -rf ./target");
        assert_eq!(parsed.risk_level, RiskLevel::Medium);
    }

    #[test]
    fn test_blocked_rm_rf_root() {
        let parsed = parse_command("rm -rf /");
        assert_eq!(parsed.risk_level, RiskLevel::Blocked);
    }

    // Subcommand detection tests
    #[test]
    fn test_git_subcommand_extraction() {
        let parsed = parse_command("git log -5");
        assert_eq!(parsed.info.command, "git");
        assert_eq!(parsed.info.subcommand, Some("log".to_string()));
        assert_eq!(parsed.info.command_pattern, "git log");
    }

    #[test]
    fn test_git_subcommand_with_flags_before() {
        let parsed = parse_command("git -C /path log --oneline");
        assert_eq!(parsed.info.command, "git");
        // First non-flag argument after command is treated as subcommand
        assert_eq!(parsed.info.subcommand, Some("/path".to_string()));
    }

    #[test]
    fn test_docker_subcommand() {
        let parsed = parse_command("docker run -it ubuntu");
        assert_eq!(parsed.info.command, "docker");
        assert_eq!(parsed.info.subcommand, Some("run".to_string()));
        assert_eq!(parsed.info.command_pattern, "docker run");
    }

    #[test]
    fn test_cargo_subcommand() {
        let parsed = parse_command("cargo build --release");
        assert_eq!(parsed.info.command, "cargo");
        assert_eq!(parsed.info.subcommand, Some("build".to_string()));
        assert_eq!(parsed.info.command_pattern, "cargo build");
    }

    #[test]
    fn test_npm_subcommand() {
        let parsed = parse_command("npm install lodash");
        assert_eq!(parsed.info.command, "npm");
        assert_eq!(parsed.info.subcommand, Some("install".to_string()));
        assert_eq!(parsed.info.command_pattern, "npm install");
    }

    #[test]
    fn test_kubectl_subcommand() {
        let parsed = parse_command("kubectl get pods -n default");
        assert_eq!(parsed.info.command, "kubectl");
        assert_eq!(parsed.info.subcommand, Some("get".to_string()));
        assert_eq!(parsed.info.command_pattern, "kubectl get");
    }

    #[test]
    fn test_command_without_subcommand_support() {
        let parsed = parse_command("rm -rf folder");
        assert_eq!(parsed.info.command, "rm");
        assert_eq!(parsed.info.subcommand, None);
        assert_eq!(parsed.info.command_pattern, "rm");
    }

    #[test]
    fn test_command_with_subcommand_support_but_no_subcommand() {
        let parsed = parse_command("git");
        assert_eq!(parsed.info.command, "git");
        assert_eq!(parsed.info.subcommand, None);
        assert_eq!(parsed.info.command_pattern, "git");
    }

    #[test]
    fn test_brew_subcommand() {
        let parsed = parse_command("brew install ripgrep");
        assert_eq!(parsed.info.command, "brew");
        assert_eq!(parsed.info.subcommand, Some("install".to_string()));
        assert_eq!(parsed.info.command_pattern, "brew install");
    }

    #[test]
    fn test_systemctl_subcommand() {
        let parsed = parse_command("systemctl status nginx");
        assert_eq!(parsed.info.command, "systemctl");
        assert_eq!(parsed.info.subcommand, Some("status".to_string()));
        assert_eq!(parsed.info.command_pattern, "systemctl status");
    }

    #[test]
    fn test_path_resolution_absolute() {
        let parsed = parse_command("rm /tmp/test.txt");
        assert_eq!(parsed.info.affected_paths, vec!["/tmp/test.txt"]);
    }

    #[test]
    fn test_path_resolution_glob_preserves_pattern() {
        let parsed = parse_command("rm /home/user/logs/*.txt");
        assert_eq!(parsed.info.affected_paths.len(), 1);
        assert!(parsed.info.affected_paths[0].contains("*.txt"));
        assert!(parsed.info.affected_paths[0].starts_with("/home/user/logs"));
    }

    #[test]
    fn test_normalize_path_resolves_parent() {
        // Test the normalize_path helper
        let path = PathBuf::from("/home/user/project/../other/file.txt");
        let normalized = normalize_path(&path);
        assert_eq!(normalized, PathBuf::from("/home/user/other/file.txt"));
    }

    #[test]
    fn test_normalize_path_resolves_current() {
        let path = PathBuf::from("/home/user/./project/./file.txt");
        let normalized = normalize_path(&path);
        assert_eq!(normalized, PathBuf::from("/home/user/project/file.txt"));
    }

    #[test]
    fn test_normalize_path_multiple_parents() {
        let path = PathBuf::from("/home/user/a/b/../../c/file.txt");
        let normalized = normalize_path(&path);
        assert_eq!(normalized, PathBuf::from("/home/user/c/file.txt"));
    }
}
