//! Bounded subprocesses for completion and prompt providers.
use std::io::{self, Read};
use std::os::unix::process::CommandExt;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

pub const OUTPUT_LIMIT: u64 = 1024 * 1024;

pub fn output(mut command: Command, timeout: Duration) -> io::Result<Output> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    let mut child = command.spawn()?;
    let group = child.id() as i32;
    struct Group(i32);
    impl Drop for Group {
        fn drop(&mut self) {
            let _ = nix::sys::signal::killpg(
                nix::unistd::Pid::from_raw(self.0),
                nix::sys::signal::Signal::SIGKILL,
            );
        }
    }
    let group = Group(group);
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let read = |reader: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            reader
                .take(OUTPUT_LIMIT)
                .read_to_end(&mut bytes)
                .map(|_| bytes)
        })
    };
    let out = read(Box::new(stdout));
    let err = read(Box::new(stderr));
    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break Ok(status);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            break Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "provider timed out",
            ));
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    drop(group);
    let stdout = out
        .join()
        .map_err(|_| io::Error::other("output reader failed"))??;
    let stderr = err
        .join()
        .map_err(|_| io::Error::other("output reader failed"))??;
    Ok(Output {
        status: status?,
        stdout,
        stderr,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timeout_terminates_children_holding_output_pipes() {
        let mut command = Command::new("sh");
        command.args(["-c", "sleep 60 & wait"]);
        let started = Instant::now();
        assert_eq!(
            output(command, Duration::from_millis(40))
                .unwrap_err()
                .kind(),
            io::ErrorKind::TimedOut
        );
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}
