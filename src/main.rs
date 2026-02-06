mod ai;
mod exec;
mod repl;

use ai::OllamaClient;
use anyhow::Result;
use exec::execute_command;
use repl::Repl;

#[tokio::main]
async fn main() -> Result<()> {
    println!("nosh v{}", env!("CARGO_PKG_VERSION"));

    let ollama = OllamaClient::new("llama3.2");

    if !ollama.check_available().await {
        eprintln!("Warning: Ollama not available at localhost:11434");
        eprintln!("Start Ollama or configure a different backend.\n");
    }

    println!("Type 'exit' to quit.\n");

    let mut repl = Repl::new()?;
    repl.load_history();

    loop {
        match repl.readline()? {
            Some(line) if line == "exit" || line == "quit" => break,
            Some(line) => {
                let cwd = std::env::current_dir()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|_| ".".to_string());

                match ollama.translate(&line, &cwd).await {
                    Ok(command) => {
                        println!("⚡ {}", command);
                        if let Err(e) = execute_command(&command) {
                            eprintln!("Execution error: {}", e);
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
