use std::io::Write;
use std::process::{Command, Stdio};

fn shell() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_nosh"));
    command.arg("--norc");
    command
}

#[test]
fn command_mode_preserves_arguments_and_exit_status() {
    let output = shell()
        .args([
            "-c",
            "printf '%s:%s' \"$0\" \"$1\"; exit 7",
            "label",
            "--help",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(7));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "label:--help");
    assert_eq!(
        shell()
            .args(["-c", "command_that_does_not_exist_for_nosh_tests"])
            .status()
            .unwrap()
            .code(),
        Some(127)
    );
}

#[test]
fn scripts_and_piped_input_do_not_launch_onboarding() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("script with spaces.sh");
    std::fs::write(&path, "printf '%s' \"$1\"\nexit 9\n").unwrap();
    let output = shell()
        .arg(&path)
        .arg("argument with spaces")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(9));
    assert_eq!(output.stdout, b"argument with spaces");
    let mut child = shell()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"if true; then\n printf piped\nfi\nexit 3\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(3));
    assert_eq!(output.stdout, b"piped");
}

#[test]
fn expansion_redirection_and_pipeline_status() {
    let temp = tempfile::tempdir().unwrap();
    let output = shell().current_dir(temp.path()).args(["-c", "printf '%s' 'hello world' > 'a file'; value=$(cat 'a file' | tr a-z A-Z); test \"$value\" = 'HELLO WORLD'"]).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        std::fs::read(temp.path().join("a file")).unwrap(),
        b"hello world"
    );
    assert_eq!(
        shell()
            .args(["-c", "set -o pipefail; false | true"])
            .status()
            .unwrap()
            .code(),
        Some(1)
    );
}

#[test]
fn startup_script_with_spaces_runs_once() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config with spaces");
    std::fs::create_dir(&config).unwrap();
    let marker = dir.path().join("startup-count");
    std::fs::write(
        config.join("init.sh"),
        format!(
            "COUNT=$(( ${{COUNT:-0}} + 1 )); printf x >> {}\n",
            shell_words::quote(&marker.to_string_lossy())
        ),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_nosh"))
        .env("NOSH_CONFIG_DIR", &config)
        .args(["-ic", "printf '%s' \"$COUNT\""])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"1");
    assert_eq!(std::fs::read(marker).unwrap(), b"x");
}
