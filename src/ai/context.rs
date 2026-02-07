//! Conversation context for AI translations.
//!
//! Stores recent exchanges to provide context for referential queries
//! like "find files" → "now delete them".

use std::collections::VecDeque;

/// A single exchange between user and AI.
#[derive(Debug, Clone)]
pub struct Exchange {
    /// The user's natural language input
    pub user_input: String,
    /// The AI-generated shell command
    pub ai_command: String,
    /// Optional summary of command output (first N chars)
    pub output_summary: Option<String>,
}

/// Tracks recent conversation exchanges for context.
#[derive(Debug)]
pub struct ConversationContext {
    exchanges: VecDeque<Exchange>,
    max_exchanges: usize,
}

impl Default for ConversationContext {
    fn default() -> Self {
        Self::new(10)
    }
}

impl ConversationContext {
    /// Create a new context with specified limits.
    pub fn new(max_exchanges: usize) -> Self {
        Self {
            exchanges: VecDeque::with_capacity(max_exchanges),
            max_exchanges,
        }
    }

    /// Record an exchange after AI translation.
    pub fn add_exchange(&mut self, user_input: &str, ai_command: &str) {
        let exchange = Exchange {
            user_input: user_input.to_string(),
            ai_command: ai_command.to_string(),
            output_summary: None,
        };

        if self.exchanges.len() >= self.max_exchanges {
            self.exchanges.pop_front();
        }
        self.exchanges.push_back(exchange);
    }

    /// Clear all context (e.g., on /clear command).
    pub fn clear(&mut self) {
        self.exchanges.clear();
    }

    /// Check if there's any context to include.
    pub fn is_empty(&self) -> bool {
        self.exchanges.is_empty()
    }

    /// Format context for inclusion in AI prompt.
    #[cfg(test)]
    pub fn format_for_prompt(&self) -> String {
        if self.exchanges.is_empty() {
            return String::new();
        }

        let mut lines = Vec::new();
        lines.push("Previous conversation:".to_string());

        for exchange in &self.exchanges {
            lines.push(format!("User: {}", exchange.user_input));
            lines.push(format!("Command: {}", exchange.ai_command));

            if let Some(ref output) = exchange.output_summary {
                lines.push(format!("Output: {}", output));
            }

            lines.push(String::new()); // blank line between exchanges
        }

        lines.join("\n")
    }

    /// Get the number of exchanges stored.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.exchanges.len()
    }

    /// Get iterator over exchanges for API serialization.
    pub fn exchanges(&self) -> impl Iterator<Item = &Exchange> {
        self.exchanges.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_exchange() {
        let mut ctx = ConversationContext::new(5);
        ctx.add_exchange("list files", "ls -la");
        assert_eq!(ctx.len(), 1);
    }

    #[test]
    fn test_max_exchanges_limit() {
        let mut ctx = ConversationContext::new(2);
        ctx.add_exchange("one", "cmd1");
        ctx.add_exchange("two", "cmd2");
        ctx.add_exchange("three", "cmd3");

        assert_eq!(ctx.len(), 2);
        // First one should be dropped
        let formatted = ctx.format_for_prompt();
        assert!(!formatted.contains("one"));
        assert!(formatted.contains("two"));
        assert!(formatted.contains("three"));
    }

    #[test]
    fn test_format_for_prompt() {
        let mut ctx = ConversationContext::new(5);
        ctx.add_exchange("find large files", "find . -size +100M");
        ctx.add_exchange("show rust ones", "find . -size +100M -name '*.rs'");

        let formatted = ctx.format_for_prompt();
        assert!(formatted.contains("Previous conversation:"));
        assert!(formatted.contains("User: find large files"));
        assert!(formatted.contains("Command: find . -size +100M"));
    }

    #[test]
    fn test_clear() {
        let mut ctx = ConversationContext::new(5);
        ctx.add_exchange("test", "cmd");
        ctx.clear();
        assert!(ctx.is_empty());
    }
}
