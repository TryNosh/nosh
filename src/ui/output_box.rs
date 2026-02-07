//! Output formatting for command output display.

use super::theme::colors;

/// Format command output with truncation for long output.
pub struct OutputBox {
    max_lines: usize,
}

impl Default for OutputBox {
    fn default() -> Self {
        Self { max_lines: 6 }
    }
}

impl OutputBox {
    /// Render output with indentation and truncation.
    /// Shows last N lines if output exceeds max_lines.
    pub fn render(&self, output: &str) -> String {
        if output.trim().is_empty() {
            return String::new();
        }

        let lines: Vec<&str> = output.lines().collect();

        // Take last N lines if too many
        let (display_lines, hidden_count) = if lines.len() > self.max_lines {
            let skip = lines.len() - self.max_lines;
            (
                lines.into_iter().skip(skip).collect::<Vec<_>>(),
                skip,
            )
        } else {
            (lines, 0)
        };

        let mut result = Vec::new();

        // Show hidden lines indicator
        if hidden_count > 0 {
            result.push(format!(
                "\n    {}... {} lines hidden{}",
                colors::DIM,
                hidden_count,
                colors::RESET
            ));
        } else {
            result.push(String::new()); // Empty line before output
        }

        // Content lines with indentation
        for line in display_lines {
            result.push(format!("    {}{}{}", colors::DIM, line, colors::RESET));
        }

        result.join("\n")
    }
}
