//! Shell invocation modes, separate from interactive Nosh slash commands.
use anyhow::{Result, bail};
use std::io::IsTerminal;
use std::path::PathBuf;

#[derive(Debug, PartialEq)]
pub enum Mode {
    Interactive,
    Command(String),
    Script(PathBuf),
    Stdin,
}
#[derive(Debug)]
pub struct Options {
    pub mode: Mode,
    pub interactive: bool,
    pub login: bool,
    pub no_rc: bool,
    pub name: String,
    pub args: Vec<String>,
}
impl Options {
    pub fn parse(args: &[String]) -> Result<Self> {
        Self::parse_with_terminal(args, std::io::stdin().is_terminal())
    }
    fn parse_with_terminal(args: &[String], terminal: bool) -> Result<Self> {
        let executable = args.first().cloned().unwrap_or_else(|| "nosh".into());
        let mut result = Self {
            mode: Mode::Interactive,
            interactive: false,
            login: executable
                .rsplit('/')
                .next()
                .unwrap_or_default()
                .starts_with('-'),
            no_rc: false,
            name: executable,
            args: vec![],
        };
        let mut index = 1;
        while let Some(arg) = args.get(index) {
            match arg.as_str() {
                "-l" | "--login" => result.login = true,
                "-i" | "--setup" => result.interactive = true,
                "--norc" | "--no-rc" => result.no_rc = true,
                "-c" | "-lc" | "-cl" | "-ic" | "-ci" => {
                    result.login |= arg.contains('l');
                    result.interactive |= arg.contains('i');
                    let Some(command) = args.get(index + 1) else {
                        bail!("{arg} requires a command string");
                    };
                    result.mode = Mode::Command(command.clone());
                    if let Some(name) = args.get(index + 2) {
                        result.name = name.clone();
                    }
                    result.args = args.get(index + 3..).unwrap_or_default().to_vec();
                    return Ok(result);
                }
                "-s" | "-" => {
                    result.mode = Mode::Stdin;
                    result.args = args[index + 1..].to_vec();
                    return Ok(result);
                }
                "--" => {
                    index += 1;
                    if let Some(path) = args.get(index) {
                        result.mode = Mode::Script(path.into());
                        result.name = path.clone();
                        result.args = args[index + 1..].to_vec();
                    } else {
                        result.interactive |= terminal;
                        if !result.interactive {
                            result.mode = Mode::Stdin;
                        }
                    }
                    return Ok(result);
                }
                flag if flag.starts_with('-') => bail!("Unknown shell option: {flag}"),
                path => {
                    result.mode = Mode::Script(path.into());
                    result.name = path.into();
                    result.args = args[index + 1..].to_vec();
                    return Ok(result);
                }
            }
            index += 1;
        }
        result.interactive |= terminal;
        if !result.interactive {
            result.mode = Mode::Stdin;
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invocation_modes_and_positional_arguments() {
        let parse = |args: &[&str], tty| {
            Options::parse_with_terminal(
                &args.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                tty,
            )
            .unwrap()
        };
        let options = parse(&["nosh", "-c", "echo", "label", "--help"], false);
        assert_eq!(options.mode, Mode::Command("echo".into()));
        assert_eq!(options.name, "label");
        assert_eq!(options.args, ["--help"]);
        assert_eq!(parse(&["nosh"], false).mode, Mode::Stdin);
        assert_eq!(parse(&["nosh"], true).mode, Mode::Interactive);
        assert_eq!(parse(&["nosh", "script.sh", "x"], false).args, ["x"]);
        assert!(parse(&["nosh", "-lc", "echo"], false).login);
    }
}
