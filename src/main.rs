mod ai;
mod config;
mod exec;
mod repl;
mod safety;

use ai::OllamaClient;
use anyhow::Result;
use config::Config;
use exec::execute_command;
use repl::Repl;
use safety::{parse_command, prompt_for_permission, PermissionChoice, PermissionStore, RiskLevel};

#[tokio::main]
async fn main() -> Result<()> {
    println!("nosh v{}", env!("CARGO_PKG_VERSION"));

    let config = Config::load().unwrap_or_default();
    let ollama = OllamaClient::new(&config.ai.model);
    let mut permissions = PermissionStore::load().unwrap_or_default();

    if config.ai.backend == "ollama" && !ollama.check_available().await {
        eprintln!("Warning: Ollama not available at localhost:11434");
        eprintln!("Start Ollama or configure a different backend.\n");
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
                match ollama.translate(&line, &cwd).await {
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
