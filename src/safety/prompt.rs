use crate::safety::{ParsedCommand, RiskLevel};
use crossterm::style::{Color, ResetColor, SetForegroundColor};
use crossterm::ExecutableCommand;
use dialoguer::{theme::ColorfulTheme, Confirm, Select};
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

    let always_allow_cmd = format!("Always allow \"{}\" commands", parsed.info.command);
    let options = vec![
        "Allow once",
        always_allow_cmd.as_str(),
        "Always allow here (this directory)",
        "Don't run",
    ];

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("What would you like to do?")
        .items(&options)
        .default(0)
        .interact()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    Ok(match selection {
        0 => PermissionChoice::AllowOnce,
        1 => PermissionChoice::AllowCommand,
        2 => PermissionChoice::AllowHere,
        _ => PermissionChoice::Deny,
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

    Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Are you sure you want to run this?")
        .default(false)
        .interact()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
}
