//! Terminal and job control setup for nosh.
//!
//! This module handles the Unix terminal control required for job control
//! (Ctrl+Z, fg, bg, jobs) to work properly, as well as terminal title updates.

use std::io::{IsTerminal, Write};

use anyhow::Result;
use nix::sys::signal::{self, SigHandler, Signal};
use nix::unistd::{self, Pid};

/// Initialize terminal control for job control support.
///
/// This must be called early in main(), before any commands are executed.
/// It sets up nosh as a proper interactive shell that can manage jobs.
pub fn init() -> Result<()> {
    // Only do this if stdin is a terminal
    if !std::io::stdin().is_terminal() {
        return Ok(());
    }

    // Ignore job control signals so we can call tcsetpgrp without being stopped.
    // These signals are sent when a background process tries to access the terminal.
    // By ignoring them, we (and our children via inheritance) can safely manipulate
    // terminal foreground process groups.
    unsafe {
        signal::signal(Signal::SIGTTOU, SigHandler::SigIgn)?;
        signal::signal(Signal::SIGTTIN, SigHandler::SigIgn)?;
        // Don't ignore SIGTSTP - we want it to work for child processes
    }

    // Put ourselves in our own process group if we're not already a process group leader.
    // This is necessary for proper job control - the shell needs to be a process group
    // leader to manage child process groups.
    let our_pid = unistd::getpid();
    let our_pgid = unistd::getpgrp();

    if our_pid != our_pgid {
        // We're not the process group leader, so create our own process group
        if unistd::setpgid(Pid::from_raw(0), Pid::from_raw(0)).is_err() {
            // This can fail if we're a session leader or other edge cases.
            // Not fatal - job control just won't work.
            return Ok(());
        }
    }

    // Take control of the terminal's foreground process group.
    // This makes nosh the foreground process group, which is required
    // for job control to work properly.
    let _ = unistd::tcsetpgrp(std::io::stdin(), unistd::getpgrp());

    Ok(())
}

/// Reclaim the terminal foreground after a child process exits or is stopped.
///
/// This should be called after any command execution to ensure nosh
/// is back in the foreground and can accept input.
pub fn reclaim_foreground() {
    if !std::io::stdin().is_terminal() {
        return;
    }

    // Move ourselves back to the foreground
    let _ = unistd::tcsetpgrp(std::io::stdin(), unistd::getpgrp());
}

/// Set the terminal title to the current working directory.
/// The path is trimmed if too long, showing just the last few components.
pub fn set_title_to_cwd() {
    if !std::io::stdout().is_terminal() {
        return;
    }

    let title = match std::env::current_dir() {
        Ok(path) => {
            let path_str = path.display().to_string();
            // Replace home directory with ~
            let home = dirs::home_dir()
                .map(|h| h.display().to_string())
                .unwrap_or_default();
            let display = if !home.is_empty() && path_str.starts_with(&home) {
                format!("~{}", &path_str[home.len()..])
            } else {
                path_str
            };
            // Trim to last 30 chars if too long
            if display.len() > 30 {
                format!("…{}", &display[display.len() - 29..])
            } else {
                display
            }
        }
        Err(_) => "nosh".to_string(),
    };

    set_title(&title);
}

/// Set the terminal title to show a running command.
pub fn set_title_to_command(command: &str) {
    if !std::io::stdout().is_terminal() {
        return;
    }

    // Extract just the command name (first word), trimmed
    let cmd_name = command
        .split_whitespace()
        .next()
        .unwrap_or(command)
        .trim();

    // Trim if too long
    let title = if cmd_name.len() > 30 {
        format!("{}…", &cmd_name[..29])
    } else {
        cmd_name.to_string()
    };

    set_title(&title);
}

/// Set the terminal title using escape sequences.
fn set_title(title: &str) {
    // OSC 0 sets both window title and icon name
    // Format: ESC ] 0 ; <title> BEL
    let _ = write!(std::io::stdout(), "\x1b]0;{}\x07", title);
    let _ = std::io::stdout().flush();
}
