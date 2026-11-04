//! Process management utilities

pub(crate) type ProcessId = i32;
pub(crate) use tokio::process::Child;

pub(crate) fn spawn(command: std::process::Command) -> std::io::Result<Child> {
    let mut command = tokio::process::Command::from(command);
    if crate::process_scope::active() {
        #[cfg(unix)]
        command.process_group(0);
        command.kill_on_drop(true);
    }
    let child = command.spawn()?;
    Ok(child)
}
