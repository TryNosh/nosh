use anyhow::Result;
use brush_builtins::{BuiltinSet, default_builtins};
use brush_core::ProcessGroupPolicy;
use brush_core::variables::ShellVariable;
use brush_core::{Shell, ExecutionParameters};

use crate::paths;

pub struct ShellSession {
    shell: Shell,
    params: ExecutionParameters,
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

        // Use SameProcessGroup to avoid terminal control issues
        let mut params = ExecutionParameters::default();
        params.process_group_policy = ProcessGroupPolicy::SameProcessGroup;

        Ok(Self { shell, params })
    }

    /// Execute a command string
    pub async fn execute(&mut self, command: &str) -> Result<()> {
        let trimmed = command.trim();

        // Handle exit/quit
        if trimmed == "exit" || trimmed == "quit" {
            std::process::exit(0);
        }

        let _result = self.shell.run_string(command, &self.params).await?;

        // Sync nosh's cwd with brush's cwd
        let shell_cwd = self.shell.working_dir();
        let _ = std::env::set_current_dir(shell_cwd);

        Ok(())
    }

}
