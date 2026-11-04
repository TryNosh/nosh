//! NoshHelper for rustyline - implements Completer, Hinter, Highlighter, and Validator.

use std::borrow::Cow;
use std::collections::HashSet;
use std::rc::Rc;

use rustyline::completion::Completer;
use rustyline::highlight::{CmdKind, Highlighter};
use rustyline::hint::Hinter;
use rustyline::validate::{ValidationContext, ValidationResult, Validator};
use rustyline::{Context, Helper};

use super::words;
use crate::completions::{Completion, CompletionInput, CompletionManager, quote_replacement};
use crate::exec::ShellSession;
use tokio::sync::Mutex;

/// Shell builtins recognized as valid commands.
const SHELL_BUILTINS: &[&str] = &[
    "cd",
    "echo",
    "exit",
    "export",
    "source",
    "alias",
    "unalias",
    "set",
    "unset",
    "eval",
    "exec",
    "read",
    "test",
    "true",
    "false",
    "pwd",
    "type",
    "hash",
    "umask",
    "wait",
    "trap",
    "return",
    "shift",
    "break",
    "continue",
    "builtin",
    "command",
    "declare",
    "local",
    "readonly",
    "typeset",
    "let",
    "bg",
    "fg",
    "jobs",
    "kill",
    "suspend",
    "logout",
    "history",
    "help",
    "pushd",
    "popd",
    "dirs",
    "shopt",
    "printf",
    "getopts",
    "ulimit",
    "times",
    "bind",
    "complete",
    "compgen",
    "caller",
    "enable",
    "mapfile",
    "readarray",
];

/// Rustyline helper providing completions, hints, and highlighting.
pub struct NoshHelper {
    completion_manager: Rc<CompletionManager>,
    syntax_highlighting: bool,
    command_cache: HashSet<String>,
    shell: Option<Rc<Mutex<ShellSession>>>,
}

impl NoshHelper {
    pub fn set_shell(&mut self, shell: Rc<Mutex<ShellSession>>) {
        self.shell = Some(shell);
    }
    pub fn new(completion_manager: Rc<CompletionManager>, syntax_highlighting: bool) -> Self {
        let command_cache = if syntax_highlighting {
            build_command_cache()
        } else {
            HashSet::new()
        };
        Self {
            completion_manager,
            syntax_highlighting,
            command_cache,
            shell: None,
        }
    }
}

/// Completion candidate for rustyline.
#[derive(Debug)]
pub struct NoshCandidate {
    /// Text to insert
    text: String,
    /// Display text (may differ from text)
    display: String,
}

impl NoshCandidate {
    pub fn new(completion: Completion) -> Self {
        let display = if let Some(desc) = completion.description {
            format!("{:<20} -- {}", completion.text, desc)
        } else {
            completion.display
        };

        Self {
            text: completion.text,
            display,
        }
    }
}

impl rustyline::completion::Candidate for NoshCandidate {
    fn display(&self) -> &str {
        &self.display
    }

    fn replacement(&self) -> &str {
        &self.text
    }
}

/// Available slash commands with descriptions.
const SLASH_COMMANDS: &[(&str, &str)] = &[
    ("/login", "Sign in to Nosh Cloud"),
    ("/setup", "Run first-time setup wizard"),
    ("/usage", "Show usage, balance, manage subscription"),
    ("/buy", "Buy tokens or subscribe to a plan"),
    ("/config", "Open or edit config files"),
    ("/create", "Create or link a nosh package"),
    ("/install", "Install package from GitHub"),
    ("/upgrade", "Upgrade nosh to latest version"),
    ("/sync", "Sync config, builtins, and packages"),
    ("/packages", "List and manage installed packages"),
    ("/convert-zsh", "Convert zsh completion to TOML"),
    ("/clear", "Clear AI conversation context"),
    ("/reload", "Reload config and theme"),
    ("/debug", "Debug plugins and theme"),
    ("/help", "Show help"),
];

impl Completer for NoshHelper {
    type Candidate = NoshCandidate;

    fn update(
        &self,
        line: &mut rustyline::line_buffer::LineBuffer,
        start: usize,
        elected: &str,
        changes: &mut rustyline::Changeset,
    ) {
        let end = crate::completions::word_end(line.as_str(), start);
        line.replace(start..end, elected, changes);
    }

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Self::Candidate>)> {
        // Check if we're in AI mode (line starts with ? or ??)
        if line.starts_with('?') {
            return self.complete_ai_mode(line, pos);
        }

        // Check if we're completing a slash command
        if line[..pos].starts_with('/')
            && !line[..pos].contains(char::is_whitespace)
            && SLASH_COMMANDS
                .iter()
                .any(|(name, _)| name.starts_with(&line[..pos]))
        {
            return self.complete_slash_command(line, pos);
        }
        let input = CompletionInput::parse(line, pos);
        if input.comment {
            return Ok((pos, vec![]));
        }
        if let Some(shell) = &self.shell
            && let Ok(session) = shell.try_lock()
        {
            self.completion_manager
                .set_environment(session.environment());
        }
        let has_toml = input
            .command()
            .is_some_and(|command| self.completion_manager.has_definition(command));
        let (start, completions, quote) = if !has_toml && let Some(shell) = &self.shell {
            // Rustyline's callback is synchronous; the live shell is asynchronous.
            // block_in_place leaves Tokio workers free for bounded shell operations.
            let native = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    shell
                        .lock()
                        .await
                        .complete_native(&line[input.segment_start..], pos - input.segment_start)
                        .await
                })
            });
            match native {
                Ok(result) => (
                    input.segment_start + result.insertion_index,
                    result.candidates.into_iter().map(Completion::new).collect(),
                    !result.options.no_autoquote_filenames,
                ),
                Err(_) => (input.start, vec![], true),
            }
        } else {
            (
                input.start,
                self.completion_manager.complete(line, pos),
                true,
            )
        };
        let raw = &line[start..pos];
        let variable_completion = raw.starts_with('$') && !raw.contains('/');
        let candidates = completions
            .into_iter()
            .map(|mut completion| {
                let path = if let Some(rest) = completion.text.strip_prefix("~/") {
                    dirs::home_dir().unwrap_or_default().join(rest)
                } else {
                    std::path::PathBuf::from(&completion.text)
                };
                if path.is_dir() && !completion.text.ends_with('/') {
                    completion.text.push('/');
                }
                let mut candidate = NoshCandidate::new(completion);
                if quote && !variable_completion {
                    candidate.text = quote_replacement(&candidate.text, raw);
                    if candidate.text.ends_with('/')
                        && pos < line.len()
                        && raw.starts_with(['\'', '"'])
                    {
                        candidate.text.push(raw.chars().next().unwrap());
                    }
                }
                candidate
            })
            .collect();
        Ok((start, candidates))
    }
}

impl NoshHelper {
    /// Complete slash commands.
    fn complete_slash_command(
        &self,
        line: &str,
        pos: usize,
    ) -> rustyline::Result<(usize, Vec<NoshCandidate>)> {
        let prefix = &line[..pos];

        let candidates: Vec<NoshCandidate> = SLASH_COMMANDS
            .iter()
            .filter(|(cmd, _)| cmd.starts_with(prefix))
            .map(|(cmd, desc)| NoshCandidate {
                text: cmd.to_string(),
                display: format!("{:<15} -- {}", cmd, desc),
            })
            .collect();

        Ok((0, candidates))
    }

    /// Complete in AI mode using English word list.
    fn complete_ai_mode(
        &self,
        line: &str,
        pos: usize,
    ) -> rustyline::Result<(usize, Vec<NoshCandidate>)> {
        // Find the start of the current word (space-separated for natural language)
        let start = find_word_start_simple(line, pos);
        let prefix = &line[start..pos];

        // Get word completions
        let candidates: Vec<NoshCandidate> = words::complete_words(prefix)
            .into_iter()
            .map(|w| NoshCandidate {
                text: w.to_string(),
                display: w.to_string(),
            })
            .collect();

        Ok((start, candidates))
    }
}

/// Find word start for natural language (simple space-based).
fn find_word_start_simple(line: &str, pos: usize) -> usize {
    let bytes = &line.as_bytes()[..pos];
    let mut start = pos;

    while start > 0 && bytes[start - 1] != b' ' && bytes[start - 1] != b'\t' {
        start -= 1;
    }

    start
}

impl Hinter for NoshHelper {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, ctx: &Context<'_>) -> Option<Self::Hint> {
        // Only show hint if cursor is at end of line
        if pos != line.len() {
            return None;
        }

        // Don't show hints for very short input
        if line.len() < 2 {
            return None;
        }

        // Slash command hints
        if line.starts_with('/') {
            return SLASH_COMMANDS
                .iter()
                .find(|(cmd, _)| cmd.starts_with(line) && cmd.len() > line.len())
                .map(|(cmd, _)| cmd[line.len()..].to_string());
        }

        // AI mode hints
        if line.starts_with('?') {
            let word_start = find_word_start_simple(line, pos);
            let current_word = &line[word_start..pos];

            if current_word.len() < 2 {
                return None;
            }

            return words::complete_words(current_word)
                .first()
                .filter(|w| w.len() > current_word.len())
                .map(|w| w[current_word.len()..].to_string());
        }

        use rustyline::history::SearchDirection;
        let history = ctx.history();
        let last = history.len().checked_sub(1)?;
        history
            .starts_with(line, last, SearchDirection::Reverse)
            .ok()
            .flatten()
            .filter(|result| result.entry.len() > line.len())
            .map(|result| result.entry[line.len()..].to_string())
    }
}

impl Highlighter for NoshHelper {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        // Style ?? and ? with elegant formatting (preserve length for cursor)
        if let Some(rest) = line.strip_prefix("??") {
            // Bold purple "??" with glow effect
            Cow::Owned(format!(
                "\x1b[1m\x1b[38;5;135m??\x1b[0m\x1b[38;5;250m{}\x1b[0m",
                rest
            ))
        } else if let Some(rest) = line.strip_prefix('?') {
            // Bold cyan "?" with glow effect
            Cow::Owned(format!(
                "\x1b[1m\x1b[38;5;45m?\x1b[0m\x1b[38;5;250m{}\x1b[0m",
                rest
            ))
        } else if self.syntax_highlighting {
            Cow::Owned(self.highlight_shell(line))
        } else {
            Cow::Borrowed(line)
        }
    }

    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self,
        prompt: &'p str,
        _default: bool,
    ) -> Cow<'b, str> {
        // Don't modify prompt - it's already styled by theme
        Cow::Borrowed(prompt)
    }

    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        // Dim gray for hints
        Cow::Owned(format!("\x1b[90m{}\x1b[0m", hint))
    }

    fn highlight_char(&self, _line: &str, _pos: usize, _kind: CmdKind) -> bool {
        // Highlight the whole line, not just changed chars
        true
    }

    fn highlight_candidate<'c>(
        &self,
        candidate: &'c str,
        _completion: rustyline::CompletionType,
    ) -> Cow<'c, str> {
        // Format candidate display
        if let Some(sep_pos) = candidate.find(" -- ") {
            let (name, desc) = candidate.split_at(sep_pos);
            Cow::Owned(format!("\x1b[1m{}\x1b[0m\x1b[90m{}\x1b[0m", name, desc))
        } else {
            Cow::Borrowed(candidate)
        }
    }
}

impl Validator for NoshHelper {
    fn validate(&self, ctx: &mut ValidationContext) -> rustyline::Result<ValidationResult> {
        let line = ctx.input();

        // Skip quote validation for AI queries - they're natural language, not shell commands
        if line.starts_with('?')
            || SLASH_COMMANDS
                .iter()
                .any(|(command, _)| line.split_whitespace().next() == Some(*command))
        {
            return Ok(ValidationResult::Valid(None));
        }

        Ok(validate_shell_input(line))
    }
}

fn validate_shell_input(line: &str) -> ValidationResult {
    let continued = brush_parser::tokenize_str(line).ok().is_some_and(|tokens| {
        tokens.last().is_some_and(|token| {
            token
                .to_str()
                .chars()
                .rev()
                .take_while(|ch| *ch == '\\')
                .count()
                % 2
                == 1
        })
    });
    let mut parser = brush_parser::Parser::new(
        std::io::BufReader::new(line.as_bytes()),
        &Default::default(),
    );
    match parser.parse_program() {
        Ok(_) if !continued => ValidationResult::Valid(None),
        Ok(_) | Err(brush_parser::ParseError::ParsingAtEndOfInput) => ValidationResult::Incomplete,
        Err(brush_parser::ParseError::Tokenizing { .. }) => ValidationResult::Incomplete,
        Err(error) => ValidationResult::Invalid(Some(error.to_string())),
    }
}

impl Helper for NoshHelper {}

/// Build a cache of valid commands by scanning PATH directories.
fn build_command_cache() -> HashSet<String> {
    let mut commands: HashSet<String> = SHELL_BUILTINS.iter().map(|s| s.to_string()).collect();

    if let Ok(path_var) = std::env::var("PATH") {
        for dir in path_var.split(':') {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.filter_map(|e| e.ok()) {
                    if let Some(name) = entry.file_name().to_str() {
                        commands.insert(name.to_string());
                    }
                }
            }
        }
    }

    commands
}

impl NoshHelper {
    /// Syntax-highlight a shell input line.
    fn highlight_shell(&self, line: &str) -> String {
        let mut result = String::with_capacity(line.len() * 2);
        let bytes = line.as_bytes();
        let len = bytes.len();
        let mut i = 0;
        // Track whether we've seen the command (first word) in the current simple command
        let mut expect_command = true;

        while i < len {
            let c = bytes[i];

            // Comments
            if c == b'#' && (i == 0 || bytes[i - 1] == b' ' || bytes[i - 1] == b'\t') {
                result.push_str("\x1b[90m");
                result.push_str(&line[i..]);
                result.push_str("\x1b[0m");
                return result;
            }

            // Strings
            if c == b'\'' || c == b'"' {
                let quote = c;
                let start = i;
                i += 1;
                while i < len && bytes[i] != quote {
                    if quote == b'"' && bytes[i] == b'\\' && i + 1 < len {
                        i += 1;
                    }
                    i += 1;
                }
                if i < len {
                    i += 1; // closing quote
                }
                result.push_str("\x1b[32m");
                result.push_str(&line[start..i]);
                result.push_str("\x1b[0m");
                expect_command = false;
                continue;
            }

            // Variables
            if c == b'$' && i + 1 < len {
                let start = i;
                i += 1;
                if i < len && bytes[i] == b'{' {
                    // ${VAR}
                    i += 1;
                    while i < len && bytes[i] != b'}' {
                        i += 1;
                    }
                    if i < len {
                        i += 1;
                    }
                } else {
                    while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                        i += 1;
                    }
                }
                result.push_str("\x1b[33m");
                result.push_str(&line[start..i]);
                result.push_str("\x1b[0m");
                expect_command = false;
                continue;
            }

            // Operators
            if c == b'|' || c == b'>' || c == b'<' || c == b';' || c == b'&' {
                let start = i;
                // Handle multi-char operators
                if i + 1 < len
                    && ((c == b'|' && bytes[i + 1] == b'|')
                        || (c == b'&' && bytes[i + 1] == b'&')
                        || (c == b'>' && bytes[i + 1] == b'>'))
                {
                    i += 2;
                } else {
                    i += 1;
                }
                result.push_str("\x1b[36m");
                result.push_str(&line[start..i]);
                result.push_str("\x1b[0m");
                // After a pipe or ;/&& the next word is a command
                expect_command = true;
                continue;
            }

            // Whitespace
            if c == b' ' || c == b'\t' {
                result.push(c as char);
                i += 1;
                continue;
            }

            // Words (commands, flags, arguments)
            // A literal '#' within a word and a trailing '$' must advance too.
            // They are neither comments nor parameter expansions here.
            if c == b'#' || c == b'$' {
                result.push(c as char);
                i += 1;
                continue;
            }
            let start = i;
            while i < len
                && bytes[i] != b' '
                && bytes[i] != b'\t'
                && bytes[i] != b'|'
                && bytes[i] != b'>'
                && bytes[i] != b'<'
                && bytes[i] != b';'
                && bytes[i] != b'&'
                && bytes[i] != b'\''
                && bytes[i] != b'"'
                && bytes[i] != b'$'
                && bytes[i] != b'#'
            {
                i += 1;
            }
            let word = &line[start..i];

            if expect_command {
                // Command position: bold if known, red if unknown
                if word.contains('/') || self.command_cache.contains(word) {
                    // Known command or path — bold
                    result.push_str("\x1b[1m");
                    result.push_str(word);
                    result.push_str("\x1b[0m");
                } else {
                    // Unknown command — red
                    result.push_str("\x1b[31m");
                    result.push_str(word);
                    result.push_str("\x1b[0m");
                }
                expect_command = false;
            } else if word.starts_with('-') {
                // Flag — blue
                result.push_str("\x1b[34m");
                result.push_str(word);
                result.push_str("\x1b[0m");
            } else {
                result.push_str(word);
            }
        }

        result
    }
}

/// Find the start of the current word being completed.
#[cfg(test)]
fn find_word_start(line: &str, pos: usize) -> usize {
    CompletionInput::parse(line, pos).start
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustyline::completion::Candidate;

    #[tokio::test(flavor = "multi_thread")]
    async fn toml_precedes_bash_and_empty_matches_do_not_fall_through() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("auditcmd.toml"),
            "[completions.auditcmd.subcommands]\ntoml = 'TOML choice'\n",
        )
        .unwrap();
        let manager = Rc::new(CompletionManager::for_paths(vec![dir.path().into()]));
        let mut helper = NoshHelper::new(manager, false);
        let shell = Rc::new(Mutex::new(
            ShellSession::configured(true, false, false, "nosh".into(), vec![])
                .await
                .unwrap(),
        ));
        shell
            .lock()
            .await
            .execute_no_job_control("complete -W 'bash' auditcmd; complete -W 'beta bravo' bashcmd")
            .await
            .unwrap();
        helper.set_shell(shell);
        let history = rustyline::history::DefaultHistory::new();
        let ctx = Context::new(&history);
        assert_eq!(
            helper.complete("auditcmd ", 9, &ctx).unwrap().1[0].replacement(),
            "toml"
        );
        assert!(
            helper
                .complete("auditcmd b", 10, &ctx)
                .unwrap()
                .1
                .is_empty()
        );
        let line = "echo ok | bashcmd b";
        let (start, choices) = helper.complete(line, line.len(), &ctx).unwrap();
        assert_eq!(&line[start..], "b");
        assert_eq!(
            choices.iter().map(|c| c.replacement()).collect::<Vec<_>>(),
            ["beta", "bravo"]
        );
        assert!(
            helper
                .complete("echo $PA", 8, &ctx)
                .unwrap()
                .1
                .iter()
                .any(|c| c.replacement() == "$PATH")
        );
    }

    #[test]
    fn file_completion_handles_spaces_absolute_paths_and_directories() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["xyz.foo", "xyz.foo.bar", "my report.txt"] {
            std::fs::write(dir.path().join(name), "").unwrap();
        }
        std::fs::create_dir(dir.path().join("docs")).unwrap();
        std::fs::write(dir.path().join("docs/inside.txt"), "").unwrap();
        std::os::unix::fs::symlink(dir.path().join("docs"), dir.path().join("link")).unwrap();
        let helper = NoshHelper::new(Rc::new(CompletionManager::for_paths(vec![])), false);
        let history = rustyline::history::DefaultHistory::new();
        let ctx = Context::new(&history);
        for prefix in [
            format!("cat {}/my", dir.path().display()),
            format!("cat \"{}/my", dir.path().display()),
        ] {
            let (start, choices) = helper.complete(&prefix, prefix.len(), &ctx).unwrap();
            let completed = format!("{}{}", &prefix[..start], choices[0].replacement());
            assert_eq!(shell_words::split(&completed).unwrap().len(), 2);
        }
        let line = format!("{}/xyz", dir.path().display());
        assert_eq!(helper.complete(&line, line.len(), &ctx).unwrap().1.len(), 2);
        let line = format!("cd {}/", dir.path().display());
        let (_, choices) = helper.complete(&line, line.len(), &ctx).unwrap();
        assert_eq!(choices.len(), 2);
        assert!(choices.iter().all(|c| c.replacement().ends_with('/')));
        let line = format!("cat {}/docs", dir.path().display());
        assert_eq!(
            helper.complete(&line, line.len(), &ctx).unwrap().1[0].replacement(),
            format!("{}/docs/", dir.path().display())
        );
    }

    #[test]
    fn grammar_validation_handles_compound_and_literal_input() {
        for line in [
            "sleep 60",
            "echo $",
            "echo a#b",
            "echo '\\\\'",
            "if true; then\n echo ok\nfi",
            "echo foo\\\\",
            "echo hi # comment \\",
        ] {
            assert!(
                matches!(validate_shell_input(line), ValidationResult::Valid(_)),
                "{line}"
            );
        }
        for line in [
            "if true; then",
            "for x in a; do",
            "echo \"hello",
            "echo ok |",
            "cat <<EOF\nhello",
            "echo foo\\",
        ] {
            assert!(
                matches!(validate_shell_input(line), ValidationResult::Incomplete),
                "{line}"
            );
        }
    }

    #[test]
    fn literal_hash_and_trailing_dollar_do_not_hang() {
        for line in ["echo $", "echo a#b", "printf '%s' '$'", "echo café#🍜"] {
            let (sender, receiver) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let helper = NoshHelper::new(Rc::new(CompletionManager::new()), true);
                sender
                    .send(helper.highlight(line, line.len()).into_owned())
                    .ok();
            });
            assert!(
                receiver
                    .recv_timeout(std::time::Duration::from_secs(2))
                    .is_ok(),
                "highlighting hung for {line:?}"
            );
        }
    }

    #[test]
    fn test_find_word_start() {
        assert_eq!(find_word_start("git commit", 10), 4);
        assert_eq!(find_word_start("git ", 4), 4);
        assert_eq!(find_word_start("git", 3), 0);
        assert_eq!(find_word_start("echo \"hello world\"", 18), 5);
    }

    #[test]
    fn test_nosh_candidate() {
        let c = Completion::new("test").with_description("Test completion");
        let candidate = NoshCandidate::new(c);
        assert_eq!(candidate.replacement(), "test");
        assert!(candidate.display().contains("Test completion"));
    }
}
