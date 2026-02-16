//! Environment initialization for nosh.
//!
//! Sources the user's init.sh to set up PATH and other environment variables.
//! This is necessary when nosh is used as a login shell.

use std::process::Command;
use crate::paths;

/// Initialize the environment by sourcing init.sh.
///
/// This extracts environment variables from init.sh,
/// ensuring tools like rustc, cargo, docker, etc. are found.
pub fn init() {
    let init_script = paths::init_file();

    if !init_script.exists() {
        return;
    }

    // Source init.sh in bash and capture the resulting environment
    let script = format!(
        "source {} 2>/dev/null && env",
        init_script.display()
    );

    let output = Command::new("bash")
        .args(["-c", &script])
        .output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return,
    };

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse and apply environment variables
    for line in stdout.lines() {
        if let Some((key, value)) = line.split_once('=') {
            // Skip some variables that shouldn't be inherited
            match key {
                "SHLVL" | "_" | "PWD" | "OLDPWD" => continue,
                _ => {}
            }
            // SAFETY: We're single-threaded at this point (called at startup)
            unsafe { std::env::set_var(key, value) };
        }
    }
}
