use anyhow::Result;
use brush_builtins::{BuiltinSet, default_builtins};
use brush_core::ProcessGroupPolicy;
use brush_core::variables::ShellVariable;
use brush_core::{
    ExecutionControlFlow, ExecutionParameters, ProfileLoadBehavior, RcLoadBehavior, Shell,
    SourceInfo,
};
use std::io::Read;
use std::time::Duration;

use super::terminal;
use crate::paths;

pub struct ShellSession {
    shell: Shell,
    pub exit_requested: Option<u8>,
    /// Default params (SameProcessGroup, for AI commands)
    params: ExecutionParameters,
    /// Job control params (NewProcessGroup, for shell commands)
    job_control_params: ExecutionParameters,
}

impl ShellSession {
    pub async fn configured(
        interactive: bool,
        login: bool,
        load_rc: bool,
        name: String,
        args: Vec<String>,
    ) -> Result<Self> {
        let init_script = paths::init_file();
        // Migrate only the exact stock script; customized init files are preserved.
        if load_rc
            && std::fs::read_to_string(&init_script).ok().as_deref()
                == Some(include_str!("../plugins/data/legacy-init.sh"))
        {
            std::fs::write(&init_script, include_str!("../plugins/data/init.sh"))?;
        }
        let rc = if load_rc && init_script.exists() {
            RcLoadBehavior::LoadCustom(init_script)
        } else {
            RcLoadBehavior::Skip
        };
        let mut shell = Shell::builder()
            .builtins(default_builtins(BuiltinSet::BashMode))
            .interactive(interactive)
            .login(login)
            .shell_name(name)
            .shell_args(args)
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
            exit_requested: None,
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

        // Update terminal title to show running command
        terminal::set_title_to_command(trimmed);

        let params = if job_control {
            self.job_control_params.clone()
        } else {
            self.params.clone()
        };
        let result = self.run(command, &params).await;
        if job_control {
            terminal::reclaim_foreground();
        }
        result.map(|_| ())
    }

    async fn run(&mut self, command: &str, params: &ExecutionParameters) -> Result<u8> {
        let result = self
            .shell
            .run_string(command, &SourceInfo::from("nosh"), params)
            .await;
        let _ = std::env::set_current_dir(self.shell.working_dir());
        let result = result?;
        let status = u8::from(result.exit_code);
        if matches!(result.next_control_flow, ExecutionControlFlow::ExitShell) {
            self.exit_requested = Some(status);
        }
        Ok(status)
    }

    pub fn environment(&self) -> std::collections::HashMap<String, String> {
        self.shell
            .env()
            .iter_exported()
            .filter(|(_, variable)| variable.value().is_set())
            .map(|(name, variable)| {
                (
                    name.clone(),
                    variable.value().to_cow_str(&self.shell).into_owned(),
                )
            })
            .collect()
    }

    pub fn status(&self) -> u8 {
        self.shell.last_exit_status()
    }

    pub async fn command_string(&mut self, command: &str) -> Result<u8> {
        Ok(self
            .shell
            .run_dash_c_command(command)
            .await?
            .exit_code
            .into())
    }

    pub async fn script(&mut self, path: &std::path::Path, args: Vec<String>) -> Result<u8> {
        Ok(self
            .shell
            .run_script(path, args.into_iter())
            .await?
            .exit_code
            .into())
    }

    /// Capture AI execution in the same shell, with bounded output and cancellation.
    pub async fn capture(&mut self, command: &str, timeout: Duration) -> Result<(String, i32)> {
        let mut snapshot = self.shell.operation_snapshot();
        let scope = brush_core::process_scope::ProcessScope::new();
        let (reader, writer) = std::io::pipe()?;
        let mut params = self.params.clone();
        params.set_fd(0, brush_core::openfiles::null()?);
        params.set_fd(1, writer.try_clone()?.into());
        params.set_fd(2, writer.into());
        let output = tokio::task::spawn_blocking(move || {
            let mut bytes = Vec::new();
            reader
                .take(super::process::OUTPUT_LIMIT)
                .read_to_end(&mut bytes)
                .map(|_| String::from_utf8_lossy(&bytes).into_owned())
        });
        let source_info = SourceInfo::from("AI command");
        let result: Result<brush_core::ExecutionResult> = tokio::select! {
            result = tokio::time::timeout(timeout, scope.run(snapshot.run_string(command, &source_info, &params))) => match result {
                Ok(result) => result.map_err(Into::into),
                Err(_) => Err(anyhow::anyhow!("Command timed out")),
            },
            _ = tokio::signal::ctrl_c() => Err(anyhow::anyhow!("Cancelled")),
        };
        drop(scope);
        drop(params);
        let output = output.await??;
        let result = result?;
        let status = u8::from(result.exit_code);
        if matches!(result.next_control_flow, ExecutionControlFlow::ExitShell) {
            self.exit_requested = Some(status);
        }
        self.shell.commit_operation(snapshot);
        let _ = std::env::set_current_dir(self.shell.working_dir());
        Ok((output, i32::from(status)))
    }

    /// Work on a clone so an interrupted completion cannot corrupt shell frames.
    pub async fn complete_native(
        &mut self,
        line: &str,
        pos: usize,
    ) -> Result<brush_core::completion::Completions> {
        let mut shell = self.shell.operation_snapshot();
        for fd in [0, 1, 2] {
            shell
                .open_files_mut()
                .set_fd(fd, brush_core::openfiles::null()?);
        }
        let scope = brush_core::process_scope::ProcessScope::new();
        let operation = async {
            if shell.funcs().get("_nosh_completion_loader").is_none() {
                shell
                    .run_string(
                        include_str!("../completions/bash-loader.sh"),
                        &SourceInfo::from("completion loader"),
                        &shell.default_exec_params(),
                    )
                    .await?;
            }
            shell.complete(line, pos).await
        };
        let result =
            tokio::time::timeout(Duration::from_millis(250), scope.run(operation)).await??;
        *self.shell.completion_config_mut() = shell.completion_config().clone();
        *self.shell.funcs_mut() = shell.funcs().clone();
        *self.shell.env_mut() = shell.env().clone();
        Ok(result)
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
            exit_requested: None,
            params: ExecutionParameters::default(),
            job_control_params: ExecutionParameters::default(),
        }
    }

    #[tokio::test]
    async fn native_completion_uses_live_functions_and_variables() {
        let mut session = test_session().await;
        session.execute_no_job_control(r#"VALUES='alpha beta'; _audit_complete() { COMPREPLY=( $(compgen -W "$VALUES" -- "$2") ); }; complete -F _audit_complete auditcmd"#).await.unwrap();
        assert_eq!(
            session
                .complete_native("auditcmd b", 10)
                .await
                .unwrap()
                .candidates,
            ["beta"]
        );
        session
            .execute_no_job_control("VALUES='bravo charlie'")
            .await
            .unwrap();
        assert_eq!(
            session
                .complete_native("auditcmd b", 10)
                .await
                .unwrap()
                .candidates,
            ["bravo"]
        );
    }

    #[tokio::test]
    async fn bash_definition_is_loaded_on_first_use() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("completions")).unwrap();
        std::fs::write(
            dir.path().join("completions/nosh_audit_tool"),
            "complete -W 'alpha beta' nosh_audit_tool\n",
        )
        .unwrap();
        let mut session = test_session().await;
        session
            .execute_no_job_control(&format!(
                "BASH_COMPLETION_USER_DIR={}",
                shell_words::quote(&dir.path().to_string_lossy())
            ))
            .await
            .unwrap();
        let line = "nosh_audit_tool b";
        assert_eq!(
            session
                .complete_native(line, line.len())
                .await
                .unwrap()
                .candidates,
            ["beta"]
        );
        std::fs::remove_file(dir.path().join("completions/nosh_audit_tool")).unwrap();
        assert_eq!(
            session
                .complete_native(line, line.len())
                .await
                .unwrap()
                .candidates,
            ["beta"]
        );
    }

    #[tokio::test]
    async fn bounded_operations_cancel_subshells_and_leave_shell_usable() {
        let mut session = test_session().await;
        session
            .execute_no_job_control(
                "VALUE=kept; work() { local VALUE=temporary; sleep 60 & wait; }",
            )
            .await
            .unwrap();
        assert!(
            session
                .capture("work", Duration::from_millis(40))
                .await
                .is_err()
        );
        assert_eq!(
            session
                .capture("printf '%s' \"$VALUE\"", Duration::from_secs(1))
                .await
                .unwrap(),
            ("kept".into(), 0)
        );
        session
            .execute_no_job_control("_slow() { while :; do :; done; }; complete -F _slow slowcmd")
            .await
            .unwrap();
        let start = std::time::Instant::now();
        assert!(session.complete_native("slowcmd ", 8).await.is_err());
        assert!(start.elapsed() < Duration::from_secs(2));
        assert_eq!(
            session
                .capture("printf ok", Duration::from_secs(1))
                .await
                .unwrap()
                .0,
            "ok"
        );
    }

    #[tokio::test]
    async fn captured_commands_commit_variables_aliases_and_functions() {
        let mut session = test_session().await;
        session
            .capture(
                "VALUE=shared; alias say='printf'; greet() { say '%s' \"$VALUE\"; }",
                Duration::from_secs(1),
            )
            .await
            .unwrap();
        assert_eq!(
            session
                .capture("greet", Duration::from_secs(1))
                .await
                .unwrap()
                .0,
            "shared"
        );
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
