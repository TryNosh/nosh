mod ai;
mod auth;
mod config;
mod exec;
mod onboarding;
mod repl;
mod safety;

use ai::{CloudClient, OllamaClient};
use anyhow::Result;
use auth::Credentials;
use config::Config;
use exec::execute_command;
use onboarding::{needs_onboarding, run_onboarding, OnboardingChoice};
use repl::Repl;
use safety::{parse_command, prompt_for_permission, PermissionChoice, PermissionStore, RiskLevel};

#[tokio::main]
async fn main() -> Result<()> {
    println!("nosh v{}", env!("CARGO_PKG_VERSION"));

    let mut creds = Credentials::load().unwrap_or_default();
    let mut permissions = PermissionStore::load().unwrap_or_default();

    // Run onboarding if needed (before loading config, since onboarding creates it)
    if needs_onboarding(&creds) {
        match run_onboarding()? {
            OnboardingChoice::Quit => return Ok(()),
            OnboardingChoice::Ollama => {}
            OnboardingChoice::Cloud => {
                creds = Credentials::load().unwrap_or_default();
            }
        }
    }

    // Load config (created by onboarding if first run)
    let config = Config::load().unwrap_or_default();

    // Check Ollama availability if using it
    if config.ai.backend == "ollama" {
        let ollama = OllamaClient::new(&config.ai.model, &config.ai.ollama_url);
        if !ollama.check_available().await {
            eprintln!("Warning: Ollama not available at {}", config.ai.ollama_url);
            eprintln!("Start Ollama or run `nosh --setup` to reconfigure.\n");
        }
    }

    println!("Type 'exit' to quit.\n");

    let mut repl = Repl::new()?;
    repl.load_history();

    loop {
        let cwd = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| ".".to_string());

        match repl.readline()? {
            Some(line) if line == "exit" || line == "quit" => break,
            Some(line) => {
                let command_result = if config.ai.backend == "cloud" {
                    if let Some(token) = &creds.token {
                        let client = CloudClient::new(token);
                        client.translate(&line, &cwd).await.map(|(cmd, _)| cmd)
                    } else {
                        Err(anyhow::anyhow!("Not authenticated"))
                    }
                } else {
                    let client = OllamaClient::new(&config.ai.model, &config.ai.ollama_url);
                    client.translate(&line, &cwd).await
                };

                match command_result {
                    Ok(command) => {
                        if config.behavior.show_command {
                            println!("⚡ {}", command);
                        }

                        let parsed = parse_command(&command);

                        let should_execute = match parsed.risk_level {
                            RiskLevel::Safe => true,
                            RiskLevel::Blocked => {
                                safety::prompt::print_blocked(&parsed)?;
                                false
                            }
                            RiskLevel::Critical => {
                                safety::prompt::print_critical_warning(&parsed)?
                            }
                            _ => {
                                if permissions.is_command_allowed(&parsed.info.command) {
                                    true
                                } else if permissions.is_directory_allowed(&cwd) {
                                    true
                                } else {
                                    match prompt_for_permission(&parsed)? {
                                        PermissionChoice::AllowOnce => true,
                                        PermissionChoice::AllowCommand => {
                                            permissions.allow_command(&parsed.info.command, true);
                                            true
                                        }
                                        PermissionChoice::AllowHere => {
                                            permissions.allow_directory(&cwd, true);
                                            true
                                        }
                                        PermissionChoice::Deny => false,
                                    }
                                }
                            }
                        };

                        if should_execute {
                            if let Err(e) = execute_command(&command) {
                                eprintln!("Execution error: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("AI error: {}", e);
                    }
                }
            }
            None => break,
        }
    }

    repl.save_history();
    println!("Goodbye!");
    Ok(())
}
