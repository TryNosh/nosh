mod ai;
mod auth;
mod config;
mod exec;
mod onboarding;
mod repl;
mod safety;

use ai::{CloudClient, OllamaClient, PlanInfo};
use dialoguer::{theme::ColorfulTheme, Select};

fn format_tokens(tokens: i32) -> String {
    if tokens >= 1_000_000 {
        format!("{:.1}M", tokens as f64 / 1_000_000.0)
    } else if tokens >= 1_000 {
        format!("{:.1}k", tokens as f64 / 1_000.0)
    } else {
        tokens.to_string()
    }
}

fn format_date(iso: &str) -> String {
    // Parse ISO date and format nicely
    // Input: "2026-03-06T12:00:00.000Z"
    // Output: "Mar 6, 2026"
    if let Some(date_part) = iso.split('T').next() {
        let parts: Vec<&str> = date_part.split('-').collect();
        if parts.len() == 3 {
            let month = match parts[1] {
                "01" => "Jan", "02" => "Feb", "03" => "Mar", "04" => "Apr",
                "05" => "May", "06" => "Jun", "07" => "Jul", "08" => "Aug",
                "09" => "Sep", "10" => "Oct", "11" => "Nov", "12" => "Dec",
                _ => parts[1],
            };
            let day = parts[2].trim_start_matches('0');
            return format!("{} {}, {}", month, day, parts[0]);
        }
    }
    iso.to_string()
}
use anyhow::Result;
use auth::Credentials;
use config::Config;
use exec::ShellSession;
use indicatif::{ProgressBar, ProgressStyle};
use onboarding::{needs_onboarding, run_onboarding, OnboardingChoice};
use repl::Repl;
use safety::{parse_command, prompt_for_permission, PermissionChoice, PermissionStore, RiskLevel};

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // Handle --help
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("nosh v{}", env!("CARGO_PKG_VERSION"));
        println!("Natural language shell powered by AI\n");
        println!("Usage: nosh [OPTIONS]\n");
        println!("Options:");
        println!("  --setup    Run setup wizard to configure AI backend");
        println!("  --help     Show this help message");
        println!("\nIn the shell:");
        println!("  command    Run command directly");
        println!("  ?query     Translate natural language to command via AI");
        println!("  exit       Quit nosh");
        return Ok(());
    }

    // Handle --setup flag
    let force_setup = args.iter().any(|a| a == "--setup");

    println!("nosh v{}", env!("CARGO_PKG_VERSION"));

    let mut creds = Credentials::load().unwrap_or_default();
    let mut permissions = PermissionStore::load().unwrap_or_default();

    // Run onboarding if needed or if --setup flag is passed
    if force_setup || needs_onboarding(&creds) {
        match run_onboarding().await? {
            OnboardingChoice::Quit => return Ok(()),
            OnboardingChoice::Ollama => {}
            OnboardingChoice::Cloud => {
                creds = Credentials::load().unwrap_or_default();
            }
        }
    }

    // Load config (created by onboarding if first run)
    let mut config = Config::load().unwrap_or_default();

    // Check Ollama availability if using it
    if config.ai.backend == "ollama" {
        let ollama = OllamaClient::new(&config.ai.model, &config.ai.ollama_url);
        if !ollama.check_available().await {
            eprintln!("Warning: Ollama not available at {}", config.ai.ollama_url);
            eprintln!("Start Ollama or run `nosh --setup` to reconfigure.\n");
        }
    }

    println!("Type /help for commands. Prefix with ? for AI.\n");

    let mut repl = Repl::new()?;
    repl.load_history();

    // Create persistent shell session (brush-based bash interpreter)
    let mut shell = ShellSession::new().await?;

    loop {
        let cwd = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| ".".to_string());

        match repl.readline()? {
            Some(line) if line == "exit" || line == "quit" => break,
            Some(line) if line == "/setup" => {
                match run_onboarding().await {
                    Ok(OnboardingChoice::Quit) => {}
                    Ok(OnboardingChoice::Ollama) | Ok(OnboardingChoice::Cloud) => {
                        creds = Credentials::load().unwrap_or_default();
                        config = Config::load().unwrap_or_default();
                        println!("\nSettings updated!");
                    }
                    Err(e) => {
                        eprintln!("Setup error: {}", e);
                    }
                }
                continue;
            }
            Some(line) if line == "/help" => {
                println!("\nBuilt-in commands:");
                println!("  /setup    Run setup wizard to switch AI backend");
                println!("  /usage    Show usage, balance, and manage subscription");
                println!("  /buy      Buy tokens or subscribe to a plan");
                println!("  /help     Show this help");
                println!("  exit      Quit nosh");
                println!("\nUsage:");
                println!("  command   Run command directly");
                println!("  ?query    Translate natural language via AI\n");
                continue;
            }
            Some(line) if line == "/usage" || line == "/tokens" || line == "/plan" => {
                if config.ai.backend != "cloud" {
                    println!("\nBackend: Ollama (local)");
                    println!("Model: {}", config.ai.model);
                    println!("URL: {}", config.ai.ollama_url);
                    println!("\nLocal mode has unlimited usage.\n");
                    continue;
                }

                let token = match &creds.token {
                    Some(t) => t,
                    None => {
                        println!("Not authenticated. Run /setup to sign in.");
                        continue;
                    }
                };

                let client = CloudClient::new(token);

                // Fetch both usage and plan info
                let usage = client.get_usage().await;
                let plan_info = client.get_plan().await.ok();

                match usage {
                    Ok(usage) => {
                        println!("\n┌─ Nosh Cloud ───────────────────────┐");
                        println!("│");

                        // Show plan info
                        if let Some(ref plan) = plan_info {
                            if let Some(plan_name) = &plan.plan {
                                let display_name = match plan_name.as_str() {
                                    "starter" => "Starter ($2.99/mo)",
                                    "pro" => "Pro ($4.99/mo)",
                                    _ => plan_name,
                                };
                                print!("│  Plan:         {}", display_name);
                                if plan.cancel_at_period_end {
                                    println!(" (canceling)");
                                } else {
                                    println!();
                                }
                            } else {
                                println!("│  Plan:         Free tier");
                            }
                        }

                        // Show token balances
                        if usage.monthly_allowance > 0 {
                            println!("│  Subscription: {} / {}",
                                format_tokens(usage.subscription_balance),
                                format_tokens(usage.monthly_allowance));
                            if let Some(resets_at) = &usage.resets_at {
                                println!("│  Renews:       {}", format_date(resets_at));
                            }
                        }
                        println!("│  Pack tokens:  {} (never expire)", format_tokens(usage.pack_balance));
                        println!("│");
                        println!("│  Total:        {}", format_tokens(usage.total_balance));
                        println!("│  Used:         {}", format_tokens(usage.tokens_used));
                        println!("│");
                        println!("└────────────────────────────────────┘\n");

                        // Show options if user has a subscription
                        let has_active_subscription = plan_info
                            .as_ref()
                            .map(|p| p.plan.is_some() && !p.cancel_at_period_end)
                            .unwrap_or(false);

                        if has_active_subscription {
                            let options = vec![
                                "Done",
                                "Manage billing (invoices, payment method)",
                                "Cancel subscription",
                            ];

                            let selection = Select::with_theme(&ColorfulTheme::default())
                                .items(&options)
                                .default(0)
                                .interact_opt();

                            match selection {
                                Ok(Some(1)) => {
                                    match client.get_portal_url().await {
                                        Ok(url) => {
                                            println!("Opening Stripe billing portal...");
                                            if let Err(e) = open::that(&url) {
                                                println!("Could not open browser: {}", e);
                                                println!("Open this URL manually: {}", url);
                                            }
                                        }
                                        Err(e) => eprintln!("Error: {}", e),
                                    }
                                }
                                Ok(Some(2)) => {
                                    println!("\nAre you sure you want to cancel?");
                                    println!("You'll keep access until the end of your billing period.\n");

                                    let confirm = Select::with_theme(&ColorfulTheme::default())
                                        .items(&["No, keep my subscription", "Yes, cancel"])
                                        .default(0)
                                        .interact_opt();

                                    if let Ok(Some(1)) = confirm {
                                        match client.cancel_subscription().await {
                                            Ok(_) => println!("\nSubscription canceled. You'll have access until the end of the billing period."),
                                            Err(e) => eprintln!("Error: {}", e),
                                        }
                                    }
                                }
                                _ => {}
                            }
                        } else if usage.total_balance < 10000 {
                            println!("Low on tokens! Run /buy to get more.\n");
                        }
                    }
                    Err(e) => eprintln!("Error: {}", e),
                }
                continue;
            }
            Some(line) if line == "/buy" => {
                if config.ai.backend != "cloud" {
                    println!("Token purchases are only available for cloud backend.");
                    println!("Run /setup to switch to Nosh Cloud.");
                    continue;
                }

                let token = match &creds.token {
                    Some(t) => t,
                    None => {
                        println!("Not authenticated. Run /setup to sign in.");
                        continue;
                    }
                };

                let client = CloudClient::new(token);

                // Get current plan to show appropriate options
                let plan_info = client.get_plan().await.ok();
                let has_subscription = plan_info.as_ref().map(|p| p.plan.is_some()).unwrap_or(false);

                let options = if has_subscription {
                    vec![
                        "Buy token pack ($2.99 - 50k tokens, never expire)",
                        "Upgrade to Pro ($4.99/mo - 250k tokens)",
                        "Cancel",
                    ]
                } else {
                    vec![
                        "Buy token pack ($2.99 - 50k tokens, never expire)",
                        "Subscribe to Starter ($2.99/mo - 100k tokens)",
                        "Subscribe to Pro ($4.99/mo - 250k tokens)",
                        "Cancel",
                    ]
                };

                let selection = Select::with_theme(&ColorfulTheme::default())
                    .with_prompt("What would you like to purchase?")
                    .items(&options)
                    .default(0)
                    .interact_opt();

                let selection = match selection {
                    Ok(Some(s)) => s,
                    _ => continue,
                };

                let url = if has_subscription {
                    match selection {
                        0 => client.buy_tokens().await,
                        1 => client.subscribe("pro").await,
                        _ => continue,
                    }
                } else {
                    match selection {
                        0 => client.buy_tokens().await,
                        1 => client.subscribe("starter").await,
                        2 => client.subscribe("pro").await,
                        _ => continue,
                    }
                };

                match url {
                    Ok(url) => {
                        println!("Opening checkout in browser...");
                        if let Err(e) = open::that(&url) {
                            println!("Could not open browser: {}", e);
                            println!("Open this URL manually: {}", url);
                        }
                    }
                    Err(e) => eprintln!("Error: {}", e),
                }
                continue;
            }
            Some(line) if line.starts_with('/') => {
                // Unknown built-in command
                eprintln!("Unknown command: {}", line);
                eprintln!("Type /help for available commands.");
                continue;
            }
            Some(line) if line.starts_with('?') => {
                // AI request - translate and run through safety layer
                let input = line[1..].trim();
                if input.is_empty() {
                    continue;
                }

                // Show spinner while waiting for AI
                let spinner = ProgressBar::new_spinner();
                spinner.set_style(
                    ProgressStyle::default_spinner()
                        .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
                        .template("{spinner:.cyan} {msg}")
                        .unwrap()
                );
                spinner.set_message("Thinking...");
                spinner.enable_steady_tick(std::time::Duration::from_millis(80));

                // AI translation
                let result = if config.ai.backend == "cloud" {
                    if let Some(token) = &creds.token {
                        let client = CloudClient::new(token);
                        client.translate(input, &cwd).await.map(|(cmd, _)| cmd)
                    } else {
                        Err(anyhow::anyhow!("Not authenticated"))
                    }
                } else {
                    let client = OllamaClient::new(&config.ai.model, &config.ai.ollama_url);
                    client.translate(input, &cwd).await
                };

                spinner.finish_and_clear();

                let command = match result {
                    Ok(cmd) => {
                        println!("⚡ {}", cmd);
                        cmd
                    }
                    Err(e) => {
                        eprintln!("AI error: {}", e);
                        continue;
                    }
                };

                // Safety layer for AI-generated commands
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
                    if let Err(e) = shell.execute(&command).await {
                        eprintln!("Execution error: {}", e);
                    }
                }
            }
            Some(command) => {
                // Direct command - execute without safety checks
                if let Err(e) = shell.execute(&command).await {
                    eprintln!("Execution error: {}", e);
                }
            }
            None => break,
        }
    }

    repl.save_history();
    println!("Goodbye!");
    Ok(())
}
