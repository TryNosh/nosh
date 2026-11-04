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
    for (flags, loads_init) in [
        (vec!["-ic"], true),
        (vec!["-l", "-ic"], true),
        (vec!["-lc"], true),
        (vec!["-c"], false),
        (vec!["--norc", "-ic"], false),
        (vec!["--norc", "-l", "-ic"], false),
        (vec!["--norc", "-lc"], false),
    ] {
        let _ = std::fs::remove_file(&marker);
        let output = Command::new(env!("CARGO_BIN_EXE_nosh"))
            .env("NOSH_CONFIG_DIR", &config)
            .env_remove("COUNT")
            .args(&flags)
            .arg("test \"$0\" = startup-test && test \"$1\" = argument || exit 99; printf '%s' \"${COUNT:-skipped}\"")
            .args(["startup-test", "argument"])
            .output()
            .unwrap();
        assert!(output.status.success(), "{flags:?}: {output:?}");
        assert_eq!(
            output.stdout,
            if loads_init {
                b"1".as_slice()
            } else {
                b"skipped"
            },
            "{flags:?}"
        );
        assert_eq!(marker.exists(), loads_init, "{flags:?}");
        if loads_init {
            assert_eq!(std::fs::read(&marker).unwrap(), b"x");
        }
    }
}

#[test]
fn stock_startup_preserves_system_variables_and_profile_precedence() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home with spaces");
    let config = dir.path().join("config");
    std::fs::create_dir(&home).unwrap();
    std::fs::create_dir(&config).unwrap();
    // Substitute system paths only in this fixture, never touch host config.
    let mut init = include_str!("../src/plugins/data/init.sh").to_owned();
    for (name, content) in [
        ("profile", "TRACE=profile\n"),
        (
            "bash.bashrc",
            "case $- in *i*) TRACE=system; export LS_OPTIONS='--color=tty';; esac\n",
        ),
        (
            "bashrc",
            "TRACE=fallback; export LS_OPTIONS='--color=auto'\n",
        ),
    ] {
        let path = dir.path().join(name);
        std::fs::write(&path, content).unwrap();
        init = init.replace(
            &format!("/etc/{name}"),
            &shell_words::quote(&path.to_string_lossy()),
        );
    }
    std::fs::write(config.join("init.sh"), init).unwrap();
    for (name, content) in [
        (".bashrc", "TRACE=${TRACE}:user\n"),
        (".bash_profile", "TRACE=${TRACE}:bash_profile\n"),
        (".bash_login", "TRACE=${TRACE}:bash_login\n"),
        (".profile", "TRACE=${TRACE}:user_profile\n"),
    ] {
        std::fs::write(home.join(name), content).unwrap();
    }
    let run = |flags: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_nosh"))
            .env("HOME", &home)
            .env("NOSH_CONFIG_DIR", &config)
            .env_remove("LS_OPTIONS")
            .env_remove("TRACE")
            .args(flags)
            .arg("printf '%s|' \"$TRACE\"; sh -c 'printf %s \"$LS_OPTIONS\"'")
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        String::from_utf8(output.stdout).unwrap()
    };
    assert_eq!(run(&["-ic"]), "system:user|--color=tty");
    std::fs::remove_file(dir.path().join("bash.bashrc")).unwrap();
    assert_eq!(run(&["-ic"]), "fallback:user|--color=auto");
    for flags in [vec!["-l", "-ic"], vec!["-lc"]] {
        assert_eq!(run(&flags), "profile:bash_profile|");
    }
    std::fs::remove_file(home.join(".bash_profile")).unwrap();
    assert_eq!(run(&["-lc"]), "profile:bash_login|");
    std::fs::remove_file(home.join(".bash_login")).unwrap();
    assert_eq!(run(&["-lc"]), "profile:user_profile|");
}

#[test]
fn stock_init_migration_preserves_customizations_and_respects_norc() {
    for original in [
        include_str!("../src/plugins/data/legacy-init.sh"),
        include_str!("../src/plugins/data/legacy-init-v1.6.0.sh"),
        "CUSTOM_INIT=preserved\n",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let init = dir.path().join("init.sh");
        std::fs::write(&init, original).unwrap();
        let run = |flags: &[&str]| {
            let output = Command::new(env!("CARGO_BIN_EXE_nosh"))
                .env("HOME", dir.path())
                .env("NOSH_CONFIG_DIR", dir.path())
                .args(flags)
                .arg(":")
                .output()
                .unwrap();
            assert!(output.status.success(), "{output:?}");
        };
        run(&["--norc", "-ic"]);
        assert_eq!(std::fs::read_to_string(&init).unwrap(), original);
        run(&["-ic"]);
        let expected = if original.starts_with("CUSTOM_INIT") {
            original
        } else {
            include_str!("../src/plugins/data/init.sh")
        };
        assert_eq!(std::fs::read_to_string(init).unwrap(), expected);
    }
}

#[test]
fn startup_exit_prevents_command_execution() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("init.sh"), "exit 17\n").unwrap();
    for flags in [vec!["-ic"], vec!["-l", "-ic"], vec!["-lc"]] {
        let output = Command::new(env!("CARGO_BIN_EXE_nosh"))
            .env("NOSH_CONFIG_DIR", dir.path())
            .args(flags)
            .arg("printf should-not-run")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(17));
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn missing_init_uses_stock_startup_without_creating_config() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(".bashrc"), "FALLBACK_INIT=loaded\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_nosh"))
        .env("HOME", dir.path())
        .env("NOSH_CONFIG_DIR", dir.path())
        .env_remove("FALLBACK_INIT")
        .args(["-ic", "printf '%s' \"$FALLBACK_INIT\""])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, b"loaded");
    assert!(!dir.path().join("init.sh").exists());
}
