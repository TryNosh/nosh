use anyhow::Result;
use brush_builtins::{BuiltinSet, default_builtins};
use brush_core::ProcessGroupPolicy;
use brush_core::variables::ShellVariable;
use brush_core::{ExecutionParameters, ProfileLoadBehavior, RcLoadBehavior, Shell, SourceInfo};

use super::terminal;
use crate::paths;

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

        let rc = if init_script.exists() {
            // Use our init script which sources ~/.bashrc
            RcLoadBehavior::LoadCustom(init_script)
        } else {
            // No init script, skip rc entirely
            RcLoadBehavior::Skip
        };
        let mut shell = Shell::builder()
            .builtins(builtins)
            .interactive(true)
            .profile(ProfileLoadBehavior::Skip)
            .rc(rc)
            .build()
            .await?;

        // Set environment variables for colored output (exported so child processes see them)
        let mut clicolor = ShellVariable::new("1");
        clicolor.export();
        shell.env_mut().set_global("CLICOLOR", clicolor)?;

        let mut clicolor_force = ShellVariable::new("1");
        clicolor_force.export();
        shell
            .env_mut()
            .set_global("CLICOLOR_FORCE", clicolor_force)?;

        if std::env::var("TERM").is_err() {
            let mut term = ShellVariable::new("xterm-256color");
            term.export();
            shell.env_mut().set_global("TERM", term)?;
        }

        // Default params: SameProcessGroup (for AI commands - no job control)
        let mut params = ExecutionParameters::default();
        params.process_group_policy = ProcessGroupPolicy::SameProcessGroup;

        // Job control params: NewProcessGroup (for shell commands - enables Ctrl+Z, fg, bg)
        let mut job_control_params = ExecutionParameters::default();
        job_control_params.process_group_policy = ProcessGroupPolicy::NewProcessGroup;

        Ok(Self {
            shell,
            params,
            job_control_params,
        })
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

        let source_info = SourceInfo::from("nosh");
        let _result = self.shell.run_string(command, &source_info, params).await?;

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

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_session() -> ShellSession {
        // Avoid loading the user's init script during tests.
        let shell = Shell::builder()
            .builtins(default_builtins(BuiltinSet::BashMode))
            .interactive(true)
            .profile(ProfileLoadBehavior::Skip)
            .rc(RcLoadBehavior::Skip)
            .build()
            .await
            .unwrap();
        ShellSession {
            shell,
            params: ExecutionParameters::default(),
            job_control_params: ExecutionParameters::default(),
        }
    }

    #[tokio::test]
    async fn commands_preserve_state_and_support_pipelines() {
        let mut session = test_session().await;

        session
            .execute_no_job_control("export NOSH_TEST_VALUE='hello nosh'")
            .await
            .unwrap();
        session
            .execute_no_job_control(
                "result=$(printf '%s' \"$NOSH_TEST_VALUE\" | tr '[:lower:]' '[:upper:]'); \
                 test \"$result\" = 'HELLO NOSH'",
            )
            .await
            .unwrap();
        assert_eq!(session.shell.last_exit_status(), 0);

        session.execute_no_job_control("false").await.unwrap();
        assert_eq!(session.shell.last_exit_status(), 1);
        session
            .execute_no_job_control("test \"$NOSH_TEST_VALUE\" = 'hello nosh'")
            .await
            .unwrap();
        assert_eq!(session.shell.last_exit_status(), 0);
        session.check_jobs().unwrap();
    }

    #[tokio::test]
    async fn aliases_expand_variables_at_invocation() {
        let mut session = test_session().await;
        session
            .execute_no_job_control(
                "LS_OPTIONS='-N --color=none -T 0'; alias nosh_ls='printf \"<%s>\" $LS_OPTIONS'",
            )
            .await
            .unwrap();
        session
            .execute_no_job_control(
                "result=$(nosh_ls 'file with spaces'); \
                 test \"$result\" = '<-N><--color=none><-T><0><file with spaces>'",
            )
            .await
            .unwrap();
        assert_eq!(session.shell.last_exit_status(), 0);

        session
            .execute_no_job_control(
                "LS_OPTIONS='--color=auto'; result=$(nosh_ls file); \
                 test \"$result\" = '<--color=auto><file>'",
            )
            .await
            .unwrap();
        assert_eq!(session.shell.last_exit_status(), 0);
    }

    #[tokio::test]
    async fn suse_style_ls_alias_expands_options() {
        let mut session = test_session().await;
        session
            .execute_no_job_control("LS_OPTIONS=-d; alias nosh_ls='/bin/ls $LS_OPTIONS'")
            .await
            .unwrap();
        session
            .execute_no_job_control("result=$(nosh_ls .); test \"$result\" = .")
            .await
            .unwrap();
        assert_eq!(session.shell.last_exit_status(), 0);
    }

    #[tokio::test]
    async fn aliases_preserve_shell_word_semantics() {
        let cases = [
            (
                "unset variable",
                r#"unset NOSH_ALIAS_OPTIONS; alias show='printf "<%s>" $NOSH_ALIAS_OPTIONS'"#,
                r#"result=$(show file); test "$result" = '<file>'"#,
            ),
            (
                "quoted variable",
                r#"NOSH_ALIAS_OPTIONS='two words'; alias show='printf "<%s>" "$NOSH_ALIAS_OPTIONS"'"#,
                r#"result=$(show file); test "$result" = '<two words><file>'"#,
            ),
            (
                "literal dollar sign",
                r#"alias show='printf "<%s>" \$NOSH_ALIAS_OPTIONS'"#,
                r#"result=$(show); test "$result" = '<$NOSH_ALIAS_OPTIONS>'"#,
            ),
            (
                "variable contents are not evaluated again",
                r#"NOSH_ALIAS_OPTIONS='$HOME'; alias show='printf "<%s>" $NOSH_ALIAS_OPTIONS'"#,
                r#"result=$(show); test "$result" = '<$HOME>'"#,
            ),
            (
                "command substitution",
                r#"alias show='printf "<%s>" "$(printf "%s" "two words")"'"#,
                r#"result=$(show); test "$result" = '<two words>'"#,
            ),
            (
                "custom field separator",
                r#"IFS=:; NOSH_ALIAS_OPTIONS='one:two'; alias show='printf "<%s>" $NOSH_ALIAS_OPTIONS'"#,
                r#"result=$(show file); test "$result" = '<one><two><file>'"#,
            ),
            (
                "quoted command name bypasses aliases",
                r#"show() { printf original; }; alias show='printf alias'"#,
                r#"result=$('show'); test "$result" = original"#,
            ),
            (
                "expanded command name bypasses aliases",
                r#"show() { printf original; }; alias show='printf alias'; cmd=show"#,
                r#"result=$($cmd); test "$result" = original"#,
            ),
            (
                "disabled aliases",
                r#"show() { printf original; }; alias show='printf alias'; shopt -u expand_aliases"#,
                r#"result=$(show); test "$result" = original"#,
            ),
            (
                "empty alias",
                r#"alias nothing=''"#,
                r#"nothing; result=$(nothing printf original); test "$result" = original"#,
            ),
        ];

        for (name, setup, command) in cases {
            let mut session = test_session().await;
            session.execute_no_job_control(setup).await.unwrap();
            assert_eq!(session.shell.last_exit_status(), 0, "setup: {name}");
            session.execute_no_job_control(command).await.unwrap();
            assert_eq!(session.shell.last_exit_status(), 0, "{name}");
        }
    }
}
