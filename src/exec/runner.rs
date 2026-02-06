use anyhow::Result;
use std::process::{Command, Stdio};

pub fn execute_command(command: &str) -> Result<()> {
    let status = Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;

    if !status.success() {
        if let Some(code) = status.code() {
            eprintln!("Command exited with code {}", code);
        }
    }

    Ok(())
}
