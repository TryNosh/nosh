//! nosh-context CLI - debugging tool for context detection.

use std::env;
use std::path::Path;

fn main() {
    let dir = env::args()
        .nth(1)
        .map(|s| std::path::PathBuf::from(s))
        .unwrap_or_else(|| env::current_dir().expect("Failed to get current directory"));

    let ctx = nosh_context::detect(Path::new(&dir));

    println!("{}", serde_json::to_string_pretty(&ctx).unwrap());
}
