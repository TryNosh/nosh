mod repl;

use anyhow::Result;
use repl::Repl;

#[tokio::main]
async fn main() -> Result<()> {
    println!("nosh v{}", env!("CARGO_PKG_VERSION"));
    println!("Type 'exit' to quit.\n");

    let mut repl = Repl::new()?;
    repl.load_history();

    loop {
        match repl.readline()? {
            Some(line) if line == "exit" || line == "quit" => break,
            Some(line) => {
                println!("You typed: {}", line);
            }
            None => break,
        }
    }

    repl.save_history();
    println!("Goodbye!");
    Ok(())
}
