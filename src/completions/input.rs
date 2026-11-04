//! Incomplete shell input for completion. Never evaluates expansions.
#[derive(Debug, Default)]
pub struct CompletionInput {
    /// Completed words in the current simple command (excluding redirections).
    pub words: Vec<String>,
    pub prefix: String,
    pub start: usize,
    pub segment_start: usize,
    pub redirect: bool,
    pub comment: bool,
}

impl CompletionInput {
    pub fn parse(line: &str, pos: usize) -> Self {
        let mut result = Self {
            start: pos,
            ..Self::default()
        };
        let mut chars = line[..pos].char_indices().peekable();
        let mut word: Option<(usize, String)> = None;
        let mut quote = None;
        let mut redirect = false;
        while let Some((index, ch)) = chars.next() {
            if let Some(q) = quote {
                if ch == q {
                    quote = None;
                } else if ch == '\\'
                    && q == '"'
                    && chars
                        .peek()
                        .is_some_and(|(_, next)| matches!(next, '$' | '`' | '"' | '\\' | '\n'))
                {
                    let (_, next) = chars.next().unwrap();
                    if next != '\n' {
                        word.as_mut().unwrap().1.push(next);
                    }
                } else {
                    word.as_mut().unwrap().1.push(ch);
                }
                continue;
            }
            match ch {
                '\'' | '"' => {
                    word.get_or_insert((index, String::new()));
                    quote = Some(ch);
                }
                '\\' => {
                    let entry = word.get_or_insert((index, String::new()));
                    if let Some((_, next)) = chars.next()
                        && next != '\n'
                    {
                        entry.1.push(next);
                    }
                }
                '#' if word.is_none() => {
                    result.comment = true;
                    break;
                }
                '|' | '&' | ';' | '\n' | '(' | ')' => {
                    word = None;
                    result.words.clear();
                    redirect = false;
                    result.segment_start = index + ch.len_utf8();
                }
                '>' | '<' => {
                    if let Some((_, text)) = word.take()
                        && !text.chars().all(|ch| ch.is_ascii_digit())
                        && !redirect
                    {
                        result.words.push(text);
                    }
                    redirect = true;
                    while chars
                        .peek()
                        .is_some_and(|(_, next)| *next == ch || *next == '&' || *next == '|')
                    {
                        chars.next();
                    }
                }
                ch if ch.is_whitespace() => {
                    if let Some((_, text)) = word.take() {
                        if redirect {
                            redirect = false;
                        } else {
                            result.words.push(text);
                        }
                    }
                }
                _ => word.get_or_insert((index, String::new())).1.push(ch),
            }
        }
        if let Some((start, text)) = word {
            result.start = start;
            result.prefix = text;
        }
        result.redirect = redirect;
        result
    }

    pub fn command(&self) -> Option<&str> {
        self.words
            .iter()
            .find(|word| !is_assignment(word))
            .map(String::as_str)
    }
}

fn is_assignment(word: &str) -> bool {
    word.split_once('=').is_some_and(|(name, _)| {
        !name.is_empty()
            && !name.starts_with(|ch: char| ch.is_ascii_digit())
            && name
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    })
}

/// End of a shell word, including its quoted or escaped suffix after the cursor.
pub fn word_end(line: &str, start: usize) -> usize {
    let mut quote = None;
    let mut escaped = false;
    for (offset, ch) in line[start..].char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            } else if ch == '\\' && q == '"' {
                escaped = true;
            }
        } else if ch == '\\' {
            escaped = true;
        } else if matches!(ch, '\'' | '"') {
            quote = Some(ch);
        } else if ch.is_whitespace() || matches!(ch, '|' | '&' | ';' | '<' | '>') {
            return start + offset;
        }
    }
    line.len()
}

/// Quote insertion text, leaving directory quotes open for further completion.
pub fn quote_replacement(text: &str, raw_prefix: &str) -> String {
    let quoted = raw_prefix
        .chars()
        .next()
        .filter(|ch| *ch == '\'' || *ch == '"');
    if let Some(quote) = quoted {
        let mut result = quote.to_string();
        for ch in text.chars() {
            if quote == '\'' && ch == '\'' {
                result.push_str("'\\''");
            } else {
                if quote == '"' && matches!(ch, '$' | '`' | '\\' | '"') {
                    result.push('\\');
                }
                result.push(ch);
            }
        }
        if !text.ends_with('/') {
            result.push(quote);
        }
        return result;
    }
    let mut result = String::new();
    for (index, ch) in text.char_indices() {
        let tilde = ch == '~' && index == 0 && raw_prefix.starts_with('~');
        if !tilde
            && (ch.is_whitespace()
                || matches!(
                    ch,
                    '\\' | '\''
                        | '"'
                        | '$'
                        | '`'
                        | ';'
                        | '&'
                        | '|'
                        | '<'
                        | '>'
                        | '('
                        | ')'
                        | '{'
                        | '}'
                        | '['
                        | ']'
                        | '*'
                        | '?'
                        | '~'
                        | '#'
                        | '!'
                ))
        {
            result.push('\\');
        }
        result.push(ch);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn incomplete_words_and_command_boundaries() {
        for (line, command, prefix) in [
            ("cat \"my rep", Some("cat"), "my rep"),
            ("cat my\\ rep", Some("cat"), "my rep"),
            ("echo ok|gi", None, "gi"),
            ("echo ok && git ch", Some("git"), "ch"),
            ("X=1 git ch", Some("git"), "ch"),
            ("cat 'a|b' fi", Some("cat"), "fi"),
        ] {
            let input = CompletionInput::parse(line, line.len());
            assert_eq!(input.command(), command, "{line}");
            assert_eq!(input.prefix, prefix, "{line}");
        }
        assert!(CompletionInput::parse("echo x > fi", 11).redirect);
    }
    #[test]
    fn inserted_names_are_literal_shell_arguments() {
        for text in [
            "my report.txt",
            "cash$HOME",
            "a'b",
            "semi;colon",
            "café🍜",
            "x$(touch nope)",
        ] {
            for prefix in ["", "'", "\""] {
                let escaped = quote_replacement(text, prefix);
                assert_eq!(shell_words::split(&escaped).unwrap(), vec![text]);
            }
        }
    }
}
