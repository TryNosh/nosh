//! Agentic mode for AI-driven command execution.
//!
//! Allows the AI to iteratively run commands, inspect output,
//! and gather information before providing a final response.

use anyhow::Result;
use std::time::{Duration, Instant};

use crate::safety::{parse_command, PermissionStore, RiskLevel};

/// Result of a single agentic step.
#[derive(Debug, Clone)]
pub enum AgenticStep {
    /// AI wants to run a command
    RunCommand {
        command: String,
        reasoning: Option<String>,
    },
    /// AI has finished and provides final response
    FinalResponse { message: String },
    /// AI encountered an error
    Error { message: String },
}

/// Result of permission check for an agentic command.
#[derive(Debug, Clone, PartialEq)]
pub enum CommandPermission {
    /// Command is allowed to run
    Allowed,
    /// Command needs user approval
    NeedsApproval,
    /// Command is blocked (critical risk)
    Blocked,
}

/// Configuration for an agentic session.
#[derive(Debug, Clone)]
pub struct AgenticConfig {
    pub max_iterations: usize,
    pub timeout_seconds: u64,
}

impl Default for AgenticConfig {
    fn default() -> Self {
        Self {
            max_iterations: 10,
            timeout_seconds: 60,
        }
    }
}

/// Tracks state during an agentic session.
pub struct AgenticSession {
    config: AgenticConfig,
    iterations: usize,
    start_time: Instant,
    /// Commands executed and their outputs
    history: Vec<(String, String)>,
}

impl AgenticSession {
    /// Create a new agentic session.
    pub fn new(config: AgenticConfig) -> Self {
        Self {
            config,
            iterations: 0,
            start_time: Instant::now(),
            history: Vec::new(),
        }
    }

    /// Check if we've exceeded limits.
    pub fn check_limits(&self) -> Result<(), String> {
        if self.iterations >= self.config.max_iterations {
            return Err(format!(
                "Reached maximum iterations ({})",
                self.config.max_iterations
            ));
        }

        let elapsed = self.start_time.elapsed();
        if elapsed > Duration::from_secs(self.config.timeout_seconds) {
            return Err(format!(
                "Timeout after {} seconds",
                self.config.timeout_seconds
            ));
        }

        Ok(())
    }

    /// Increment iteration count.
    pub fn increment(&mut self) {
        self.iterations += 1;
    }

    /// Get current iteration count.
    pub fn iterations(&self) -> usize {
        self.iterations
    }

    /// Record a command execution.
    pub fn record_execution(&mut self, command: &str, output: &str) {
        self.history.push((command.to_string(), output.to_string()));
    }

    /// Get execution history for context.
    pub fn history(&self) -> &[(String, String)] {
        &self.history
    }

    /// Check if a command is allowed to run.
    pub fn check_permission(
        &self,
        command: &str,
        cwd: &str,
        permissions: &PermissionStore,
    ) -> CommandPermission {
        let parsed = parse_command(command);

        match parsed.risk_level {
            RiskLevel::Safe => CommandPermission::Allowed,
            RiskLevel::Blocked | RiskLevel::Critical => CommandPermission::Blocked,
            _ => {
                // Check if command is already allowed
                if permissions.is_command_allowed(&parsed.info.command, &parsed.info.command_pattern)
                {
                    CommandPermission::Allowed
                } else if permissions.are_affected_paths_allowed(
                    &parsed.info.command,
                    &parsed.info.command_pattern,
                    &parsed.info.affected_paths,
                    cwd,
                ) {
                    CommandPermission::Allowed
                } else if permissions.is_directory_allowed(cwd) {
                    CommandPermission::Allowed
                } else {
                    CommandPermission::NeedsApproval
                }
            }
        }
    }
}

/// Format agentic output for display.
pub fn format_step_output(command: &str, output: &str, iteration: usize) -> String {
    let truncated = if output.len() > 1000 {
        format!("{}...\n(output truncated)", &output[..1000])
    } else {
        output.to_string()
    };

    format!(
        "\x1b[36m[Step {}]\x1b[0m Running: {}\n{}",
        iteration, command, truncated
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_limits() {
        let config = AgenticConfig {
            max_iterations: 3,
            timeout_seconds: 60,
        };
        let mut session = AgenticSession::new(config);

        assert!(session.check_limits().is_ok());
        session.increment();
        session.increment();
        session.increment();
        assert!(session.check_limits().is_err());
    }

    #[test]
    fn test_record_execution() {
        let mut session = AgenticSession::new(AgenticConfig::default());
        session.record_execution("ls -la", "file1.txt\nfile2.txt");

        assert_eq!(session.history().len(), 1);
        assert_eq!(session.history()[0].0, "ls -la");
    }
}
