use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub enum RiskLevel {
    Safe,       // echo, pwd, ls (no writes)
    Low,        // single file write, git operations
    Medium,     // glob deletes, recursive operations
    High,       // sudo, system modifications
    Critical,   // rm -rf ~, rm -rf /, curl | sh
    Blocked,    // absolutely never allow
}

#[derive(Debug, Clone)]
pub struct CommandInfo {
    pub command: String,
    pub args: Vec<String>,
    pub is_destructive: bool,
    pub is_network: bool,
    pub is_privileged: bool,
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

pub fn parse_command(raw: &str) -> ParsedCommand {
    let words = shell_words::split(raw).unwrap_or_else(|_| vec![raw.to_string()]);

    let (command, args) = if words.is_empty() {
        (String::new(), vec![])
    } else {
        (words[0].clone(), words[1..].to_vec())
    };

    let is_destructive = DESTRUCTIVE_COMMANDS.contains(&command.as_str());
    let is_network = NETWORK_COMMANDS.contains(&command.as_str());
    let is_privileged = PRIVILEGED_COMMANDS.contains(&command.as_str());

    let affected_paths: Vec<String> = args
        .iter()
        .filter(|a| !a.starts_with('-'))
        .filter(|a| Path::new(a).exists() || a.contains('*') || a.contains('/'))
        .cloned()
        .collect();

    let info = CommandInfo {
        command: command.clone(),
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
}
