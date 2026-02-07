//! Terminal UI components for nosh.

mod output_box;
pub mod theme;

pub use output_box::OutputBox;

use termimad::MadSkin;
use theme::colors;

/// Format a step header with iteration number and command
pub fn format_step(iteration: usize, command: &str, reasoning: Option<&str>) -> String {
    let mut result = format!(
        "\n  {}{}─{} {}",
        colors::CYAN,
        iteration,
        colors::RESET,
        command
    );

    if let Some(reason) = reasoning {
        result.push_str(&format!(
            "\n    {}{}{}",
            colors::DIM,
            reason,
            colors::RESET
        ));
    }

    result
}

/// Format command output in a dimmed box
pub fn format_output(output: &str) -> String {
    OutputBox::default().render(output)
}

/// Format a translated command for simple query mode
pub fn format_translated_command(command: &str) -> String {
    format!(
        "{}⚡{} {}",
        colors::CYAN,
        colors::RESET,
        command
    )
}

/// Format a simple header with separator
pub fn format_header(title: &str, subtitle: &str) -> String {
    format!(
        "\n{}{}:{} {}\n{}─────────────────────────────────{}",
        colors::CYAN,
        title,
        colors::RESET,
        subtitle,
        colors::DIM,
        colors::RESET
    )
}

/// Format a result message with markdown rendering
pub fn format_result(message: &str) -> String {
    use termimad::crossterm::style::{Color, Attribute};

    let mut skin = MadSkin::default();

    // Highlight specific elements
    skin.bold.set_fg(Color::Green);
    skin.bold.add_attr(Attribute::Bold);
    skin.italic.set_fg(Color::Cyan);
    skin.inline_code.set_fg(Color::Yellow);
    skin.code_block.set_fg(Color::Yellow);
    skin.headers[0].set_fg(Color::Green);
    skin.headers[0].add_attr(Attribute::Bold);

    let rendered = skin.term_text(message);
    format!("\n{}", rendered)
}

/// Format an error message
pub fn format_error(message: &str) -> String {
    format!(
        "{}error:{} {}",
        colors::RED,
        colors::RESET,
        message
    )
}
