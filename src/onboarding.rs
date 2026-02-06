use anyhow::{anyhow, Result};
use crossterm::style::{Color, ResetColor, SetForegroundColor};
use crossterm::ExecutableCommand;
use dialoguer::{theme::ColorfulTheme, Input, Select};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use std::time::Duration;
use crate::auth::Credentials;
use crate::config::Config;

pub enum OnboardingChoice {
    Ollama,
    Cloud,
    Quit,
}

#[derive(Serialize)]
struct DeviceAuthRequest {
    email: String,
}

#[derive(Deserialize)]
struct DeviceAuthResponse {
    device_code: String,
    #[allow(dead_code)]
    verification_url: String,
    #[allow(dead_code)]
    expires_in: u32,
}

#[derive(Serialize)]
struct DeviceTokenRequest {
    device_code: String,
}

#[derive(Deserialize)]
struct DeviceTokenResponse {
    token: String,
}

#[derive(Deserialize)]
struct ErrorResponse {
    error: String,
}

pub async fn run_onboarding() -> Result<OnboardingChoice> {
    let mut stdout = io::stdout();

    stdout.execute(SetForegroundColor(Color::Cyan))?;
    writeln!(stdout, "\nWelcome to nosh!")?;
    stdout.execute(ResetColor)?;
    writeln!(stdout)?;

    let options = &[
        "Ollama (free, runs locally)",
        "Nosh Cloud (subscription)",
        "Quit",
    ];

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("How would you like to power your shell?")
        .items(options)
        .default(0)
        .interact()?;

    match selection {
        0 => {
            setup_ollama().await?;
            Ok(OnboardingChoice::Ollama)
        }
        1 => {
            setup_cloud().await?;
            Ok(OnboardingChoice::Cloud)
        }
        _ => Ok(OnboardingChoice::Quit),
    }
}

async fn setup_ollama() -> Result<()> {
    let mut stdout = io::stdout();

    writeln!(stdout)?;
    writeln!(stdout, "Setting up Ollama...")?;
    writeln!(stdout)?;

    let model: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Which model would you like to use?")
        .default("llama3.2".to_string())
        .interact_text()?;

    let mut config = Config::load().unwrap_or_default();
    config.ai.backend = "ollama".to_string();
    config.ai.model = model.clone();
    config.save()?;

    writeln!(stdout)?;
    stdout.execute(SetForegroundColor(Color::Green))?;
    writeln!(stdout, "Ollama configured with model: {}", model)?;
    stdout.execute(ResetColor)?;
    writeln!(stdout)?;

    Ok(())
}

fn get_cloud_url() -> String {
    std::env::var("NOSH_CLOUD_URL").unwrap_or_else(|_| "https://nosh.sh/api".to_string())
}

async fn setup_cloud() -> Result<()> {
    let mut stdout = io::stdout();
    let client = Client::new();
    let base_url = get_cloud_url();

    writeln!(stdout)?;
    writeln!(stdout, "Setting up Nosh Cloud...")?;
    writeln!(stdout)?;

    let email: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Enter your email")
        .validate_with(|input: &String| -> Result<(), &str> {
            if input.contains('@') {
                Ok(())
            } else {
                Err("Please enter a valid email address")
            }
        })
        .interact_text()?;

    writeln!(stdout)?;
    writeln!(stdout, "Sending magic link...")?;

    // Start device auth flow
    let response = client
        .post(format!("{}/auth/device", base_url))
        .json(&DeviceAuthRequest { email: email.clone() })
        .send()
        .await;

    let device_code = match response {
        Ok(resp) if resp.status().is_success() => {
            let auth: DeviceAuthResponse = resp.json().await?;
            auth.device_code
        }
        Ok(resp) => {
            let error: ErrorResponse = resp.json().await.unwrap_or(ErrorResponse {
                error: "Unknown error".to_string(),
            });
            return Err(anyhow!("Failed to start auth: {}", error.error));
        }
        Err(e) => {
            // Server not available - fall back to manual token entry
            writeln!(stdout)?;
            stdout.execute(SetForegroundColor(Color::Yellow))?;
            writeln!(stdout, "Could not connect to Nosh Cloud: {}", e)?;
            stdout.execute(ResetColor)?;
            writeln!(stdout, "Enter your token manually (get one from https://nosh.sh):")?;
            write!(stdout, "Token: ")?;
            stdout.flush()?;

            let mut token = String::new();
            io::stdin().read_line(&mut token)?;
            let token = token.trim().to_string();

            if token.is_empty() {
                return Err(anyhow!("No token provided"));
            }

            save_cloud_credentials(&email, &token)?;
            return Ok(());
        }
    };

    writeln!(stdout)?;
    stdout.execute(SetForegroundColor(Color::Green))?;
    writeln!(stdout, "Magic link sent! Check your inbox and click the link.")?;
    stdout.execute(ResetColor)?;
    writeln!(stdout, "Waiting for you to click the link...")?;
    writeln!(stdout)?;

    // Poll for token
    let mut attempts = 0;
    let max_attempts = 90; // 90 * 2 seconds = 3 minutes

    loop {
        attempts += 1;
        if attempts > max_attempts {
            return Err(anyhow!("Authentication timed out. Please try again."));
        }

        tokio::time::sleep(Duration::from_secs(2)).await;

        let response = client
            .post(format!("{}/auth/device/token", base_url))
            .json(&DeviceTokenRequest {
                device_code: device_code.clone(),
            })
            .send()
            .await?;

        if response.status().is_success() {
            let token_resp: DeviceTokenResponse = response.json().await?;
            save_cloud_credentials(&email, &token_resp.token)?;

            writeln!(stdout)?;
            stdout.execute(SetForegroundColor(Color::Green))?;
            writeln!(stdout, "Authenticated! You're ready to use nosh.")?;
            stdout.execute(ResetColor)?;
            writeln!(stdout)?;
            return Ok(());
        }

        // 428 means authorization_pending - keep polling
        if response.status().as_u16() != 428 {
            let error: ErrorResponse = response.json().await.unwrap_or(ErrorResponse {
                error: "Unknown error".to_string(),
            });
            return Err(anyhow!("Authentication failed: {}", error.error));
        }

        // Show a simple progress indicator
        if attempts % 5 == 0 {
            write!(stdout, ".")?;
            stdout.flush()?;
        }
    }
}

fn save_cloud_credentials(email: &str, token: &str) -> Result<()> {
    let mut creds = Credentials::load().unwrap_or_default();
    creds.token = Some(token.to_string());
    creds.email = Some(email.to_string());
    creds.save()?;

    let mut config = Config::load().unwrap_or_default();
    config.ai.backend = "cloud".to_string();
    config.save()?;

    Ok(())
}

pub fn needs_onboarding(creds: &Credentials) -> bool {
    // Run onboarding if config doesn't exist (first run)
    if !Config::exists() {
        return true;
    }

    // Otherwise check backend-specific requirements
    let config = Config::load().unwrap_or_default();
    match config.ai.backend.as_str() {
        "cloud" => !creds.is_authenticated(),
        "ollama" => false, // Ollama config exists, just warn if not available
        _ => true, // Unknown backend, run onboarding
    }
}
