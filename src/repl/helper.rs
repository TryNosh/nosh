//! NoshHelper for rustyline - implements Completer, Hinter, Highlighter, and Validator.

use std::borrow::Cow;
use std::sync::Arc;

use rustyline::completion::Completer;
use rustyline::highlight::{CmdKind, Highlighter};
use rustyline::hint::Hinter;
use rustyline::validate::{ValidationContext, ValidationResult, Validator};
use rustyline::{Context, Helper};

use super::words;
use crate::completions::{Completion, CompletionManager};

/// Rustyline helper providing completions, hints, and highlighting.
pub struct NoshHelper {
    completion_manager: Arc<CompletionManager>,
}

impl NoshHelper {
    pub fn new(completion_manager: Arc<CompletionManager>) -> Self {
        Self { completion_manager }
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
    ("/setup", "Run setup wizard to sign in"),
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
        if line.starts_with('/') {
            return self.complete_slash_command(line, pos);
        }

        let completions = self.completion_manager.complete(line, pos);
        let start = find_word_start(line, pos);

        let candidates = completions
            .into_iter()
            .map(NoshCandidate::new)
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
    let bytes = line[..pos].as_bytes();
    let mut start = pos;

    while start > 0 && bytes[start - 1] != b' ' && bytes[start - 1] != b'\t' {
        start -= 1;
    }

    start
}

impl Hinter for NoshHelper {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> Option<Self::Hint> {
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

        // Get completions for shell commands
        let completions = self.completion_manager.complete(line, pos);

        // Find completion that starts with current word
        let word_start = find_word_start(line, pos);
        let current_word = &line[word_start..pos];

        completions
            .into_iter()
            .find(|c| c.text.starts_with(current_word) && c.text.len() > current_word.len())
            .map(|c| c.text[current_word.len()..].to_string())
    }
}

impl Highlighter for NoshHelper {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        // Style ?? and ? with elegant formatting (preserve length for cursor)
        if line.starts_with("??") {
            let rest = &line[2..];
            // Bold purple "??" with glow effect
            Cow::Owned(format!(
                "\x1b[1m\x1b[38;5;135m??\x1b[0m\x1b[38;5;250m{}\x1b[0m",
                rest
            ))
        } else if line.starts_with('?') {
            let rest = &line[1..];
            // Bold cyan "?" with glow effect
            Cow::Owned(format!(
                "\x1b[1m\x1b[38;5;45m?\x1b[0m\x1b[38;5;250m{}\x1b[0m",
                rest
            ))
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
        if line.starts_with('?') || line.starts_with('/') {
            return Ok(ValidationResult::Valid(None));
        }

        // Check for unclosed quotes
        let mut in_single = false;
        let mut in_double = false;
        let mut escaped = false;

        for c in line.chars() {
            if escaped {
                escaped = false;
                continue;
            }

            match c {
                '\\' => escaped = true,
                '\'' if !in_double => in_single = !in_single,
                '"' if !in_single => in_double = !in_double,
                _ => {}
            }
        }

        if in_single || in_double {
            return Ok(ValidationResult::Incomplete);
        }

        // Check for line continuation
        if line.ends_with('\\') {
            return Ok(ValidationResult::Incomplete);
        }

        Ok(ValidationResult::Valid(None))
    }
}

impl Helper for NoshHelper {}

/// Find the start of the current word being completed.
fn find_word_start(line: &str, pos: usize) -> usize {
    let line = &line[..pos];
    let bytes = line.as_bytes();

    let mut start = pos;
    let mut in_quote = false;
    let mut quote_char = b'"';

    // Walk back from cursor to find word start
    while start > 0 {
        let c = bytes[start - 1];

        if in_quote {
            if c == quote_char {
                in_quote = false;
            }
            start -= 1;
            continue;
        }

        match c {
            b'"' | b'\'' => {
                in_quote = true;
                quote_char = c;
                start -= 1;
            }
            b' ' | b'\t' => break,
            _ => start -= 1,
        }
    }

    start
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustyline::completion::Candidate;

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
