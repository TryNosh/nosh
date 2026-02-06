use crate::safety::{ParsedCommand, RiskLevel};
use crossterm::style::{Color, ResetColor, SetForegroundColor};
use crossterm::ExecutableCommand;
use std::io::{self, Write};

#[derive(Debug, Clone, PartialEq)]
pub enum PermissionChoice {
    AllowOnce,
    AllowCommand,  // Allow this command pattern
    AllowHere,     // Allow all in this directory
    Deny,
}

pub fn prompt_for_permission(parsed: &ParsedCommand) -> io::Result<PermissionChoice> {
    let mut stdout = io::stdout();

    // Print warning
    stdout.execute(SetForegroundColor(Color::Yellow))?;
    writeln!(stdout, "\nnosh wants to run: {}", parsed.raw)?;
    stdout.execute(ResetColor)?;

    writeln!(stdout, "Risk: {} - {}",
        match parsed.risk_level {
            RiskLevel::Safe => "safe",
            RiskLevel::Low => "low",
            RiskLevel::Medium => "medium",
            RiskLevel::High => "high",
            RiskLevel::Critical => "CRITICAL",
            RiskLevel::Blocked => "BLOCKED",
        },
        parsed.risk_reason
    )?;

    writeln!(stdout)?;
    writeln!(stdout, "  [enter] Allow once")?;
    writeln!(stdout, "  [a] Always allow \"{}\" commands", parsed.info.command)?;
    writeln!(stdout, "  [d] Always allow here (this directory)")?;
    writeln!(stdout, "  [n] Don't run")?;
    writeln!(stdout)?;
    write!(stdout, "> ")?;
    stdout.flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim().to_lowercase();

    Ok(match input.as_str() {
        "" => PermissionChoice::AllowOnce,
        "a" => PermissionChoice::AllowCommand,
        "d" => PermissionChoice::AllowHere,
        "n" | "no" => PermissionChoice::Deny,
        _ => PermissionChoice::Deny, // Default to deny on unknown input
    })
}

pub fn print_blocked(parsed: &ParsedCommand) -> io::Result<()> {
    let mut stdout = io::stdout();
    stdout.execute(SetForegroundColor(Color::Red))?;
    writeln!(stdout, "\n✗ Command blocked: {}", parsed.raw)?;
    stdout.execute(ResetColor)?;
    writeln!(stdout, "Reason: {}", parsed.risk_reason)?;
    Ok(())
}

pub fn print_critical_warning(parsed: &ParsedCommand) -> io::Result<bool> {
    let mut stdout = io::stdout();
    stdout.execute(SetForegroundColor(Color::Red))?;
    writeln!(stdout, "\n⚠ CRITICAL: {}", parsed.raw)?;
    stdout.execute(ResetColor)?;
    writeln!(stdout, "Reason: {}", parsed.risk_reason)?;
    writeln!(stdout)?;
    write!(stdout, "Type 'yes' to proceed: ")?;
    stdout.flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    Ok(input.trim().to_lowercase() == "yes")
}
