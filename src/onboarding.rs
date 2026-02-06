use anyhow::Result;
use crossterm::style::{Color, ResetColor, SetForegroundColor};
use crossterm::ExecutableCommand;
use std::io::{self, Write};
use crate::auth::Credentials;
use crate::config::Config;

pub enum OnboardingChoice {
    Ollama,
    Cloud,
    Quit,
}

pub fn run_onboarding() -> Result<OnboardingChoice> {
    let mut stdout = io::stdout();

    stdout.execute(SetForegroundColor(Color::Cyan))?;
    writeln!(stdout, "\nWelcome to nosh!")?;
    stdout.execute(ResetColor)?;

    writeln!(stdout)?;
    writeln!(stdout, "How would you like to power your shell?")?;
    writeln!(stdout)?;
    writeln!(stdout, "  [1] Ollama (free, runs locally)")?;
    writeln!(stdout, "      Requires Ollama installed with a model")?;
    writeln!(stdout)?;
    writeln!(stdout, "  [2] Nosh Cloud (subscription)")?;
    writeln!(stdout, "      No setup, works instantly")?;
    writeln!(stdout)?;
    writeln!(stdout, "  [q] Quit")?;
    writeln!(stdout)?;
    write!(stdout, "> ")?;
    stdout.flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim();

    match input {
        "1" => {
            setup_ollama()?;
            Ok(OnboardingChoice::Ollama)
        }
        "2" => {
            setup_cloud()?;
            Ok(OnboardingChoice::Cloud)
        }
        "q" | "Q" => Ok(OnboardingChoice::Quit),
        _ => {
            writeln!(stdout, "Invalid choice. Please try again.")?;
            run_onboarding()
        }
    }
}

fn setup_ollama() -> Result<()> {
    let mut stdout = io::stdout();

    writeln!(stdout)?;
    writeln!(stdout, "Setting up Ollama...")?;
    writeln!(stdout)?;
    writeln!(stdout, "Which model would you like to use?")?;
    writeln!(stdout, "(Press enter for default: llama3.2)")?;
    writeln!(stdout)?;
    write!(stdout, "Model: ")?;
    stdout.flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let model = input.trim();
    let model = if model.is_empty() { "llama3.2" } else { model };

    let mut config = Config::load().unwrap_or_default();
    config.ai.backend = "ollama".to_string();
    config.ai.model = model.to_string();
    config.save()?;

    writeln!(stdout)?;
    stdout.execute(SetForegroundColor(Color::Green))?;
    writeln!(stdout, "Ollama configured with model: {}", model)?;
    stdout.execute(ResetColor)?;
    writeln!(stdout)?;

    Ok(())
}

fn setup_cloud() -> Result<()> {
    let mut stdout = io::stdout();

    writeln!(stdout)?;
    writeln!(stdout, "Setting up Nosh Cloud...")?;
    writeln!(stdout)?;
    write!(stdout, "Enter your email: ")?;
    stdout.flush()?;

    let mut email = String::new();
    io::stdin().read_line(&mut email)?;
    let email = email.trim().to_string();

    if !email.contains('@') {
        writeln!(stdout, "Invalid email. Please try again.")?;
        return setup_cloud();
    }

    writeln!(stdout)?;
    writeln!(stdout, "Magic link sent! Check your inbox and click the link.")?;
    writeln!(stdout, "Waiting for authentication...")?;

    // TODO: Actually send magic link and poll for token
    // For now, ask user to paste token
    writeln!(stdout)?;
    write!(stdout, "Paste your token: ")?;
    stdout.flush()?;

    let mut token = String::new();
    io::stdin().read_line(&mut token)?;
    let token = token.trim().to_string();

    if token.is_empty() {
        writeln!(stdout, "No token provided. Please try again.")?;
        return setup_cloud();
    }

    let mut creds = Credentials::load().unwrap_or_default();
    creds.token = Some(token);
    creds.email = Some(email);
    creds.save()?;

    let mut config = Config::load().unwrap_or_default();
    config.ai.backend = "cloud".to_string();
    config.save()?;

    writeln!(stdout)?;
    stdout.execute(SetForegroundColor(Color::Green))?;
    writeln!(stdout, "Authenticated! You're ready to use nosh.")?;
    stdout.execute(ResetColor)?;
    writeln!(stdout)?;

    Ok(())
}

pub fn needs_onboarding(config: &Config, creds: &Credentials) -> bool {
    match config.ai.backend.as_str() {
        "cloud" => !creds.is_authenticated(),
        "ollama" => false, // Ollama doesn't need setup, just warn if not available
        _ => true, // Unknown backend, run onboarding
    }
}
