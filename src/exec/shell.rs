use anyhow::Result;
use brush_builtins::{BuiltinSet, default_builtins};
use brush_core::ProcessGroupPolicy;
use brush_core::variables::ShellVariable;
use brush_core::{Shell, ExecutionParameters};

use crate::paths;
use super::terminal;

pub struct ShellSession {
    shell: Shell,
    /// Default params (SameProcessGroup, for AI commands)
    params: ExecutionParameters,
    /// Job control params (NewProcessGroup, for shell commands)
    job_control_params: ExecutionParameters,
}

impl ShellSession {
    pub async fn new() -> Result<Self> {
        // Get the standard bash builtins (cd, export, etc.)
        let builtins = default_builtins(BuiltinSet::BashMode);

        // Build shell with builtins
        // Use our custom init.sh instead of default rc files
        let init_script = paths::init_file();

        let mut shell = if init_script.exists() {
            // Use our init script which sources ~/.bashrc
            Shell::builder()
                .builtins(builtins)
                .interactive(true)
                .no_profile(true)
                .rc_file(init_script)
                .build()
                .await?
        } else {
            // No init script, skip rc entirely
            Shell::builder()
                .builtins(builtins)
                .interactive(true)
                .no_profile(true)
                .no_rc(true)
                .build()
                .await?
        };

        // Set environment variables for colored output (exported so child processes see them)
        let mut clicolor = ShellVariable::new("1");
        clicolor.export();
        shell.env.set_global("CLICOLOR", clicolor)?;

        let mut clicolor_force = ShellVariable::new("1");
        clicolor_force.export();
        shell.env.set_global("CLICOLOR_FORCE", clicolor_force)?;

        if std::env::var("TERM").is_err() {
            let mut term = ShellVariable::new("xterm-256color");
            term.export();
            shell.env.set_global("TERM", term)?;
        }

        // Default params: SameProcessGroup (for AI commands - no job control)
        let mut params = ExecutionParameters::default();
        params.process_group_policy = ProcessGroupPolicy::SameProcessGroup;

        // Job control params: NewProcessGroup (for shell commands - enables Ctrl+Z, fg, bg)
        let mut job_control_params = ExecutionParameters::default();
        job_control_params.process_group_policy = ProcessGroupPolicy::NewProcessGroup;

        Ok(Self { shell, params, job_control_params })
    }

    /// Execute a command string with job control (for direct shell commands).
    /// Supports Ctrl+Z to suspend, and fg/bg/jobs builtins.
    pub async fn execute(&mut self, command: &str) -> Result<()> {
        self.execute_internal(command, true).await
    }

    /// Execute a command without job control (for AI-translated commands).
    /// Ctrl+Z will not suspend these commands.
    pub async fn execute_no_job_control(&mut self, command: &str) -> Result<()> {
        self.execute_internal(command, false).await
    }

    /// Internal execution with configurable job control
    async fn execute_internal(&mut self, command: &str, job_control: bool) -> Result<()> {
        let trimmed = command.trim();

        // Handle exit/quit
        if trimmed == "exit" || trimmed == "quit" {
            std::process::exit(0);
        }

        // Update terminal title to show running command
        terminal::set_title_to_command(trimmed);

        let params = if job_control {
            &self.job_control_params
        } else {
            &self.params
        };

        let _result = self.shell.run_string(command, params).await?;

        // After command completes (or is stopped), reclaim terminal foreground
        if job_control {
            terminal::reclaim_foreground();
        }

        // Sync nosh's cwd with brush's cwd
        let shell_cwd = self.shell.working_dir();
        let _ = std::env::set_current_dir(shell_cwd);

        Ok(())
    }

    /// Check and report completed background jobs.
    /// Call this after each command to notify user of finished jobs.
    pub fn check_jobs(&mut self) -> Result<()> {
        self.shell.check_for_completed_jobs()?;
        Ok(())
    }
}
