mod ai;
mod auth;
mod completions;
mod config;
mod exec;
mod history;
mod onboarding;
mod paths;
mod plugins;
mod repl;
mod safety;
mod ui;

use ai::{
    AgenticConfig, AgenticSession, AgenticStep, CloudClient, CommandPermission,
    ConversationContext,
};
use ui::{format_step, format_output, format_translated_command, format_header, format_result, format_error};
use plugins::builtins::{install_builtins, ConfigFile, update_config, config_needs_update};
use dialoguer::{theme::ColorfulTheme, Select, MultiSelect};

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

async fn show_buy_menu(client: &CloudClient) {
    // Get current plan to show appropriate options
    let plan_info = client.get_plan().await.ok();
    let current_plan = plan_info.as_ref().and_then(|p| p.plan.as_deref());
    let is_canceling = plan_info.as_ref().map(|p| p.cancel_at_period_end).unwrap_or(false);

    // Build options based on current plan
    let mut options: Vec<String> = Vec::new();
    let mut actions: Vec<&str> = Vec::new();

    // Subscribers can buy token packs
    if current_plan.is_some() {
        options.push("Buy token pack ($2.99 - 125k tokens)".to_string());
        actions.push("tokens");
    }

    // Show all plan options with current plan marked
    let plans = [
        ("lite", "Lite", "$2.99/mo", "250k tokens"),
        ("pro", "Pro", "$9.99/mo", "1M tokens"),
        ("power", "Power", "$19.99/mo", "3M tokens"),
    ];

    for (id, name, price, tokens) in plans {
        let is_current = current_plan == Some(id);
        let label = if is_current && is_canceling {
            format!("{} ({} - {}) (current, canceling)", name, price, tokens)
        } else if is_current {
            format!("{} ({} - {}) (current)", name, price, tokens)
        } else {
            format!("{} ({} - {})", name, price, tokens)
        };
        options.push(label);
        actions.push(id);
    }

    options.push("Back".to_string());
    actions.push("cancel");

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select a plan")
        .items(&options)
        .default(0)
        .interact_opt();

    let selection = match selection {
        Ok(Some(s)) => s,
        _ => return,
    };

    let action = actions.get(selection).copied().unwrap_or("cancel");

    // Handle selecting current plan
    if Some(action) == current_plan {
        if is_canceling {
            // Resubscribe to current plan (reactivate)
            match client.reactivate_subscription().await {
                Ok(_) => {
                    println!("\nSubscription reactivated!");
                    return;
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    return;
                }
            }
        } else {
            println!("\nYou're already on this plan.");
            return;
        }
    }

    let url = match action {
        "tokens" => client.buy_tokens().await,
        "lite" => client.subscribe("lite").await,
        "pro" => client.subscribe("pro").await,
        "power" => client.subscribe("power").await,
        _ => return,
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
}
use anyhow::Result;
use auth::Credentials;
use config::Config;
use exec::ShellSession;
use indicatif::{ProgressBar, ProgressStyle};
use onboarding::{needs_onboarding, run_onboarding, OnboardingChoice};
use repl::{Repl, ReadlineResult};
use safety::{parse_command, prompt_for_permission, PermissionChoice, PermissionStore, RiskLevel};

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // Handle --help
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("nosh v{}", env!("CARGO_PKG_VERSION"));
        println!("A modern shell for developers\n");
        println!("Usage: nosh [COMMAND] [OPTIONS]\n");
        println!("Commands:");
        println!("  convert-zsh FILE   Convert zsh completion file to nosh TOML format");
        println!("\nOptions:");
        println!("  --setup            Run setup wizard to sign in");
        println!("  --help             Show this help message");
        println!("\nIn the shell:");
        println!("  command    Run command directly");
        println!("  ?query     Translate natural language to command via AI");
        println!("  ??query    Agentic mode - AI investigates before answering");
        println!("  exit       Quit nosh");
        println!("\nLegal:");
        println!("  Terms of Use:    https://nosh.sh/docs/terms");
        println!("  Privacy Policy:  https://nosh.sh/docs/privacy");
        println!("\nBy using nosh, you agree to the Terms of Use.");
        return Ok(());
    }

    // Handle convert-zsh subcommand
    if args.get(1).map(|s| s.as_str()) == Some("convert-zsh") {
        if let Some(path) = args.get(2) {
            let path = std::path::Path::new(path);
            match completions::convert_zsh_file(path) {
                Ok(toml) => {
                    println!("{}", toml);
                    return Ok(());
                }
                Err(e) => {
                    eprintln!("Error converting zsh completion: {}", e);
                    std::process::exit(1);
                }
            }
        } else {
            eprintln!("Error: convert-zsh requires a file path");
            eprintln!("Usage: nosh convert-zsh /path/to/zsh/completion");
            std::process::exit(1);
        }
    }

    // Handle --setup flag
    let force_setup = args.iter().any(|a| a == "--setup");

    // Initialize terminal control for job control support (Ctrl+Z, fg, bg, jobs)
    if let Err(e) = exec::terminal::init() {
        eprintln!("Warning: Could not initialize job control: {}", e);
    }

    let mut creds = Credentials::load().unwrap_or_default();
    let mut permissions = PermissionStore::load().unwrap_or_default();

    // Run onboarding if needed or if --setup flag is passed
    if force_setup || needs_onboarding(&creds) {
        // Install built-in plugins and themes on first run
        let _ = install_builtins();

        match run_onboarding().await? {
            OnboardingChoice::Cloud => {
                creds = Credentials::load().unwrap_or_default();
            }
            OnboardingChoice::Skip => {
                // User skipped AI setup - continue with shell only
            }
        }
    }

    // Load config (created by onboarding if first run)
    let config = Config::load().unwrap_or_default();

    // Show welcome message if configured
    if !config.welcome_message.is_empty() {
        println!("{}\n", config.welcome_message);
    }

    // Initialize REPL with theme from config
    let mut repl = Repl::new(&config.prompt.theme, Some(config.history.load_count))?;
    repl.load_history();

    // Create persistent shell session (brush-based bash interpreter)
    let mut shell = ShellSession::new().await?;

    // Create conversation context for AI
    let mut ai_context = ConversationContext::new(config.ai.context_size);

    loop {
        let cwd = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| ".".to_string());

        // Update terminal title to show current directory
        exec::terminal::set_title_to_cwd();

        match repl.readline()? {
            ReadlineResult::Eof => break,
            ReadlineResult::Interrupted => {
                // Ctrl+C at prompt - just show a new prompt
                println!();
                continue;
            }
            ReadlineResult::Line(line) if line == "exit" || line == "quit" => break,
            ReadlineResult::Line(line) if line == "/setup" => {
                match run_onboarding().await {
                    Ok(OnboardingChoice::Cloud) => {
                        creds = Credentials::load().unwrap_or_default();
                        println!("\nSettings updated!");
                    }
                    Ok(OnboardingChoice::Skip) => {
                        // User cancelled setup
                    }
                    Err(e) => {
                        eprintln!("Setup error: {}", e);
                    }
                }
                continue;
            }
            ReadlineResult::Line(line) if line == "/help" => {
                println!("\nBuilt-in commands:");
                println!("  /setup              Run setup wizard to sign in");
                println!("  /usage              Show usage, balance, and manage subscription");
                println!("  /buy                Buy tokens or subscribe to a plan");
                println!("  /nosh               Manage nosh config files");
                println!("  /convert-zsh FILE   Convert zsh completion to nosh TOML");
                println!("  /clear              Clear AI conversation context");
                println!("  /help               Show this help");
                println!("  exit                Quit nosh");
                println!("\nUsage:");
                println!("  command   Run command directly");
                println!("  ?query    Translate natural language via AI");
                println!("  ??query   Agentic mode - AI investigates before answering");
                println!("\nLegal:");
                println!("  Terms of Use:    https://nosh.sh/docs/terms");
                println!("  Privacy Policy:  https://nosh.sh/docs/privacy\n");
                continue;
            }
            ReadlineResult::Line(line) if line == "/clear" => {
                ai_context.clear();
                println!("AI context cleared.");
                continue;
            }
            ReadlineResult::Line(line) if line.starts_with("/convert-zsh ") => {
                let path = line.strip_prefix("/convert-zsh ").unwrap().trim();
                if path.is_empty() {
                    eprintln!("Usage: /convert-zsh /path/to/zsh/completion");
                    continue;
                }
                let path = std::path::Path::new(path);
                match completions::convert_zsh_file(path) {
                    Ok(toml) => println!("{}", toml),
                    Err(e) => eprintln!("Error: {}", e),
                }
                continue;
            }
            ReadlineResult::Line(line) if line == "/convert-zsh" => {
                eprintln!("Usage: /convert-zsh /path/to/zsh/completion");
                continue;
            }
            ReadlineResult::Line(line) if line == "/usage" => {
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
                                    "lite" => "Lite ($2.99/mo)",
                                    "pro" => "Pro ($9.99/mo)",
                                    "power" => "Power ($19.99/mo)",
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

                        // Show options based on subscription state
                        let has_subscription = plan_info.as_ref().map(|p| p.plan.is_some()).unwrap_or(false);
                        let is_canceling = plan_info.as_ref().map(|p| p.cancel_at_period_end).unwrap_or(false);

                        if has_subscription {
                            let options = if is_canceling {
                                vec![
                                    "Done",
                                    "Manage billing (invoices, payment method)",
                                    "Reactivate subscription",
                                ]
                            } else {
                                vec![
                                    "Done",
                                    "Manage billing (invoices, payment method)",
                                    "Upgrade plan (via /buy)",
                                    "Cancel subscription",
                                ]
                            };

                            let selection = Select::with_theme(&ColorfulTheme::default())
                                .items(&options)
                                .default(0)
                                .interact_opt();

                            match selection {
                                Ok(Some(1)) => {
                                    // Manage billing - open Stripe portal
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
                                Ok(Some(2)) if is_canceling => {
                                    // Reactivate subscription
                                    match client.reactivate_subscription().await {
                                        Ok(_) => println!("\nSubscription reactivated!"),
                                        Err(e) => eprintln!("Error: {}", e),
                                    }
                                }
                                Ok(Some(2)) => {
                                    // Upgrade plan - show buy menu
                                    show_buy_menu(&client).await;
                                }
                                Ok(Some(3)) if !is_canceling => {
                                    // Cancel subscription
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
            ReadlineResult::Line(line) if line == "/buy" => {
                let token = match &creds.token {
                    Some(t) => t,
                    None => {
                        println!("Not authenticated. Run /setup to sign in.");
                        continue;
                    }
                };

                let client = CloudClient::new(token);
                show_buy_menu(&client).await;
                continue;
            }
            ReadlineResult::Line(line) if line == "/nosh" => {
                let options = vec![
                    "Open config directory",
                    "Edit config file",
                    "Update config files to latest",
                    "Install/update completions",
                    "Back",
                ];

                let selection = Select::with_theme(&ColorfulTheme::default())
                    .with_prompt("Nosh Configuration")
                    .items(&options)
                    .default(0)
                    .interact_opt();

                match selection {
                    Ok(Some(0)) => {
                        // Open config directory
                        let config_dir = paths::nosh_config_dir();
                        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "open".to_string());
                        println!("Opening {}...", config_dir.display());
                        if let Err(e) = std::process::Command::new(&editor)
                            .arg(&config_dir)
                            .spawn()
                        {
                            eprintln!("Could not open directory: {}", e);
                            println!("Config directory: {}", config_dir.display());
                        }
                    }
                    Ok(Some(1)) => {
                        // Edit specific config file
                        let files = vec![
                            ("Theme (default.toml)", paths::themes_dir().join("default.toml")),
                            ("Config (config.toml)", paths::config_file()),
                            ("Init script (init.sh)", paths::init_file()),
                            ("Permissions", paths::permissions_file()),
                        ];

                        let file_names: Vec<&str> = files.iter().map(|(n, _)| *n).collect();

                        let file_selection = Select::with_theme(&ColorfulTheme::default())
                            .with_prompt("Select file to edit")
                            .items(&file_names)
                            .default(0)
                            .interact_opt();

                        if let Ok(Some(idx)) = file_selection {
                            let (_, path) = &files[idx];
                            let editor = std::env::var("EDITOR").unwrap_or_else(|_| "nano".to_string());
                            println!("Opening {} with {}...", path.display(), editor);
                            if let Err(e) = std::process::Command::new(&editor)
                                .arg(path)
                                .status()
                            {
                                eprintln!("Could not open editor: {}", e);
                            }
                        }
                    }
                    Ok(Some(2)) => {
                        // Update config files
                        let files_to_update: Vec<_> = ConfigFile::all()
                            .iter()
                            .filter(|f| config_needs_update(**f))
                            .copied()
                            .collect();

                        if files_to_update.is_empty() {
                            println!("\nAll config files are up to date!");
                            continue;
                        }

                        let labels: Vec<String> = files_to_update
                            .iter()
                            .map(|f| format!("{} ({})", f.display_name(), f.path().display()))
                            .collect();

                        println!("\nThe following files differ from built-in defaults:\n");

                        let selections = MultiSelect::with_theme(&ColorfulTheme::default())
                            .with_prompt("Select files to update (Space to toggle, Enter to confirm)")
                            .items(&labels)
                            .defaults(&vec![true; labels.len()])
                            .interact_opt();

                        if let Ok(Some(indices)) = selections {
                            if indices.is_empty() {
                                println!("No files selected.");
                            } else {
                                for idx in &indices {
                                    let file = files_to_update[*idx];
                                    match update_config(file) {
                                        Ok(_) => println!("  Updated: {}", file.display_name()),
                                        Err(e) => eprintln!("  Error updating {}: {}", file.display_name(), e),
                                    }
                                }
                                // Reload theme and plugins
                                repl.reload("default");
                                println!("\nConfig reloaded!");
                            }
                        }
                    }
                    Ok(Some(3)) => {
                        // Install/update completions
                        let completion_files = [
                            ConfigFile::GitCompletion,
                            ConfigFile::CargoCompletion,
                            ConfigFile::NpmCompletion,
                            ConfigFile::DockerCompletion,
                        ];

                        let labels: Vec<String> = completion_files
                            .iter()
                            .map(|f| {
                                let status = if f.path().exists() {
                                    if config_needs_update(*f) { "(update available)" } else { "(installed)" }
                                } else {
                                    "(not installed)"
                                };
                                format!("{} {}", f.display_name(), status)
                            })
                            .collect();

                        let selections = MultiSelect::with_theme(&ColorfulTheme::default())
                            .with_prompt("Select completions to install/update")
                            .items(&labels)
                            .defaults(&vec![true; labels.len()])
                            .interact_opt();

                        if let Ok(Some(indices)) = selections {
                            if indices.is_empty() {
                                println!("No completions selected.");
                            } else {
                                for idx in &indices {
                                    let file = completion_files[*idx];
                                    match update_config(file) {
                                        Ok(_) => println!("  Installed: {}", file.display_name()),
                                        Err(e) => eprintln!("  Error installing {}: {}", file.display_name(), e),
                                    }
                                }
                                println!("\nCompletions installed to {}", paths::completions_dir().display());
                            }
                        }
                    }
                    _ => {}
                }
                continue;
            }
            ReadlineResult::Line(line) if line.starts_with('/') => {
                // Unknown built-in command
                eprintln!("Unknown command: {}", line);
                eprintln!("Type /help for available commands.");
                continue;
            }
            ReadlineResult::Line(line) if line.starts_with("??") => {
                // Agentic mode - AI investigates before answering
                let input = line[2..].trim();
                if input.is_empty() {
                    continue;
                }

                if !config.ai.agentic_enabled {
                    eprintln!("Agentic mode is disabled. Enable it in config.toml:");
                    eprintln!("  [ai]");
                    eprintln!("  agentic_enabled = true");
                    continue;
                }

                let token = match &creds.token {
                    Some(t) => t.clone(),
                    None => {
                        eprintln!("Not authenticated. Run /setup to sign in.");
                        continue;
                    }
                };

                let client = CloudClient::new(&token);
                let agentic_config = AgenticConfig {
                    max_iterations: config.ai.max_iterations,
                    timeout_seconds: config.ai.timeout,
                };
                let mut session = AgenticSession::new(agentic_config);
                let mut executions: Vec<(String, String, i32)> = Vec::new();

                println!("{}", format_header("Investigating", input));

                // Agentic loop
                loop {
                    // Check limits
                    if let Err(msg) = session.check_limits() {
                        eprintln!("{}", format_error(&msg));
                        break;
                    }

                    // Get next step from AI
                    println!(); // Separate from previous step
                    let ai_spinner = ui::spinner::create();

                    let step = match client
                        .agentic_step(input, &cwd, Some(&ai_context), &executions)
                        .await
                    {
                        Ok(s) => {
                            ai_spinner.finish_and_clear();
                            s
                        }
                        Err(e) => {
                            ai_spinner.finish_and_clear();
                            eprintln!("AI error: {}", e);
                            break;
                        }
                    };

                    session.increment();

                    match step {
                        AgenticStep::RunCommand { command, reasoning } => {
                            // Check permissions
                            let permission =
                                session.check_permission(&command, &cwd, &permissions);

                            let should_run = match permission {
                                CommandPermission::Allowed => true,
                                CommandPermission::Blocked => {
                                    eprintln!(
                                        "\x1b[31m[Blocked]\x1b[0m AI requested blocked command: {}",
                                        command
                                    );
                                    false
                                }
                                CommandPermission::NeedsApproval => {
                                    // Show the command and ask for permission
                                    let parsed = parse_command(&command);
                                    println!(
                                        "\n\x1b[33m[Approval needed]\x1b[0m AI wants to run: {}",
                                        command
                                    );
                                    match prompt_for_permission(&parsed)? {
                                        PermissionChoice::AllowOnce => true,
                                        PermissionChoice::AllowCommand => {
                                            permissions.allow_command(&parsed.info.command, true);
                                            true
                                        }
                                        PermissionChoice::AllowSubcommand => {
                                            permissions
                                                .allow_command(&parsed.info.command_pattern, true);
                                            true
                                        }
                                        PermissionChoice::AllowCommandHere => {
                                            permissions.allow_command_in_directory(
                                                &parsed.info.command_pattern,
                                                &cwd,
                                                true,
                                            );
                                            true
                                        }
                                        PermissionChoice::AllowHere => {
                                            permissions.allow_directory(&cwd, true);
                                            true
                                        }
                                        PermissionChoice::Deny => {
                                            println!("Command denied. Stopping agentic mode.");
                                            false
                                        }
                                    }
                                }
                            };

                            if !should_run {
                                // Send empty result to AI so it can try something else
                                executions.push((
                                    command,
                                    "[Permission denied]".to_string(),
                                    1,
                                ));
                                continue;
                            }

                            // Execute the command and capture output
                            println!("{}", format_step(session.iterations(), &command, reasoning.as_deref()));

                            // Show spinner while command runs
                            let spinner = ProgressBar::new_spinner();
                            spinner.set_style(
                                ProgressStyle::default_spinner()
                                    .template("{spinner:.cyan} {msg}")
                                    .unwrap(),
                            );
                            spinner.set_message("Running...");
                            spinner.enable_steady_tick(std::time::Duration::from_millis(100));

                            // Capture output by running through shell (async so spinner can tick)
                            let output = match tokio::process::Command::new("sh")
                                .arg("-c")
                                .arg(&command)
                                .current_dir(&cwd)
                                .output()
                                .await
                            {
                                Ok(out) => {
                                    spinner.finish_and_clear();
                                    let stdout = String::from_utf8_lossy(&out.stdout);
                                    let stderr = String::from_utf8_lossy(&out.stderr);
                                    let combined = if stderr.is_empty() {
                                        stdout.to_string()
                                    } else {
                                        format!("{}\n{}", stdout, stderr)
                                    };

                                    // Print output in dimmed box
                                    let formatted = format_output(&combined);
                                    if !formatted.is_empty() {
                                        println!("{}", formatted);
                                    }

                                    (combined, out.status.code().unwrap_or(1))
                                }
                                Err(e) => {
                                    spinner.finish_and_clear();
                                    (format!("Error: {}", e), 1)
                                }
                            };

                            session.record_execution(&command, &output.0);
                            executions.push((command, output.0, output.1));
                        }
                        AgenticStep::FinalResponse { message } => {
                            println!("{}", format_result(&message));
                            // Record in context
                            ai_context.add_exchange(input, &format!("[agentic] {}", message));
                            break;
                        }
                        AgenticStep::Error { message } => {
                            eprintln!("{}", format_error(&message));
                            break;
                        }
                    }
                }
                continue;
            }
            ReadlineResult::Line(line) if line.starts_with('?') => {
                // AI request - translate and run through safety layer
                let input = line[1..].trim();
                if input.is_empty() {
                    continue;
                }

                // Show spinner while waiting for AI
                let spinner = ui::spinner::create();

                // AI translation with conversation context
                let result = if let Some(token) = &creds.token {
                    let client = CloudClient::new(token);
                    client.translate(input, &cwd, Some(&ai_context)).await.map(|(cmd, _)| cmd)
                } else {
                    Err(anyhow::anyhow!("Not authenticated. Run /setup to sign in."))
                };

                spinner.finish_and_clear();

                let command = match result {
                    Ok(cmd) => {
                        println!("{}", format_translated_command(&cmd));
                        // Record exchange in context (before execution, in case it fails)
                        ai_context.add_exchange(input, &cmd);
                        cmd
                    }
                    Err(e) => {
                        eprintln!("{}", format_error(&e.to_string()));
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
                        // Check permissions in order: global command, command+directory (checking actual paths), all-directory
                        if permissions.is_command_allowed(&parsed.info.command, &parsed.info.command_pattern) {
                            true
                        } else if permissions.are_affected_paths_allowed(
                            &parsed.info.command,
                            &parsed.info.command_pattern,
                            &parsed.info.affected_paths,
                            &cwd,
                        ) {
                            true
                        } else if permissions.is_directory_allowed(&cwd) {
                            true
                        } else {
                            match prompt_for_permission(&parsed)? {
                                PermissionChoice::AllowOnce => true,
                                PermissionChoice::AllowCommandHere => {
                                    // Allow this command/pattern in this directory only
                                    permissions.allow_command_in_directory(
                                        &parsed.info.command_pattern,
                                        &cwd,
                                        true,
                                    );
                                    true
                                }
                                PermissionChoice::AllowSubcommand => {
                                    // Allow specific subcommand pattern globally (e.g., "git log")
                                    permissions.allow_command(&parsed.info.command_pattern, true);
                                    true
                                }
                                PermissionChoice::AllowCommand => {
                                    // Allow base command globally (all subcommands)
                                    permissions.allow_command(&parsed.info.command, true);
                                    true
                                }
                                PermissionChoice::AllowHere => {
                                    // Allow all commands in this directory
                                    permissions.allow_directory(&cwd, true);
                                    true
                                }
                                PermissionChoice::Deny => false,
                            }
                        }
                    }
                };

                if should_execute {
                    repl.start_command();
                    // AI commands run without job control (Ctrl+Z won't suspend)
                    if let Err(e) = shell.execute_no_job_control(&command).await {
                        eprintln!("Execution error: {}", e);
                    }
                    repl.end_command();
                }
            }
            ReadlineResult::Line(command) => {
                // Direct command - execute with job control (Ctrl+Z suspends)
                repl.start_command();
                if let Err(e) = shell.execute(&command).await {
                    eprintln!("Execution error: {}", e);
                }
                repl.end_command();

                // Check for completed background jobs
                let _ = shell.check_jobs();
            }
        }
    }

    repl.save_history();
    println!("Goodbye!");
    Ok(())
}
