use crate::safety::{ParsedCommand, RiskLevel};
use crossterm::ExecutableCommand;
use crossterm::style::{Color, ResetColor, SetForegroundColor};
use dialoguer::{Confirm, Select, theme::ColorfulTheme};
use std::io::{self, Write};

#[derive(Debug, Clone, PartialEq)]
pub enum PermissionChoice {
    AllowOnce,
    AllowSubcommand,  // Allow specific subcommand pattern (e.g., "git log")
    AllowCommand,     // Allow base command (e.g., "git" - allows all subcommands)
    AllowCommandHere, // Allow this command/pattern in this directory only
    AllowHere,        // Allow all commands in this directory
    Deny,
}

pub fn prompt_for_permission(parsed: &ParsedCommand) -> io::Result<PermissionChoice> {
    let mut stdout = io::stdout();

    // Print warning
    stdout.execute(SetForegroundColor(Color::Yellow))?;
    writeln!(stdout, "\nnosh wants to run: {}", parsed.raw)?;
    stdout.execute(ResetColor)?;

    writeln!(
        stdout,
        "Risk: {} - {}",
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

    // Build options based on whether command has a subcommand
    let has_subcommand = parsed.info.subcommand.is_some();
    let command = &parsed.info.command;
    let command_pattern = &parsed.info.command_pattern;

    let mut options: Vec<String> = vec!["Allow once".to_string()];

    if has_subcommand {
        // Show option for specific subcommand (e.g., "git log")
        options.push(format!(
            "Always allow \"{}\" commands here",
            command_pattern
        ));
        options.push(format!(
            "Always allow \"{}\" commands everywhere",
            command_pattern
        ));
        // Show option for all subcommands (e.g., all "git" commands)
        options.push(format!("Always allow all \"{}\" commands", command));
    } else {
        // No subcommand - show directory-scoped option first, then global
        options.push(format!("Always allow \"{}\" here", command));
        options.push(format!("Always allow \"{}\" everywhere", command));
    }

    options.push("Always allow all commands here".to_string());
    options.push("Don't run".to_string());

    let option_refs: Vec<&str> = options.iter().map(|s| s.as_str()).collect();

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("What would you like to do?")
        .items(&option_refs)
        .default(0)
        .interact()
        .map_err(io::Error::other)?;

    if has_subcommand {
        // With subcommand: 0=once, 1=subcommand here, 2=subcommand everywhere, 3=all command, 4=all here, 5=deny
        Ok(match selection {
            0 => PermissionChoice::AllowOnce,
            1 => PermissionChoice::AllowCommandHere, // "git log" here only
            2 => PermissionChoice::AllowSubcommand,  // "git log" everywhere
            3 => PermissionChoice::AllowCommand,     // all "git" commands
            4 => PermissionChoice::AllowHere,        // all commands here
            _ => PermissionChoice::Deny,
        })
    } else {
        // Without subcommand: 0=once, 1=command here, 2=command everywhere, 3=all here, 4=deny
        Ok(match selection {
            0 => PermissionChoice::AllowOnce,
            1 => PermissionChoice::AllowCommandHere, // "rm" here only
            2 => PermissionChoice::AllowCommand,     // "rm" everywhere
            3 => PermissionChoice::AllowHere,        // all commands here
            _ => PermissionChoice::Deny,
        })
    }
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
        .map_err(io::Error::other)
}
