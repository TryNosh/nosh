//! Convert zsh completion files to nosh TOML format.
//!
//! Parses zsh _arguments syntax and generates equivalent TOML completions.

use anyhow::{Context, Result};
use regex::Regex;
use std::collections::HashMap;

/// Parsed zsh completion option.
#[derive(Debug)]
struct ZshOption {
    name: String,
    description: String,
    takes_value: bool,
}

/// Parsed zsh completion definition.
#[derive(Debug)]
struct ZshCompletion {
    command: String,
    description: Option<String>,
    options: Vec<ZshOption>,
    subcommands: HashMap<String, String>,
}

/// Convert zsh completion file content to nosh TOML format.
///
/// # Example
///
/// ```ignore
/// let zsh = r#"
/// #compdef mycommand
/// _arguments \
///     '-h[Show help]' \
///     '-v[Verbose output]' \
///     '--config[Config file]:file:_files'
/// "#;
///
/// let toml = convert_zsh_completion(zsh).unwrap();
/// println!("{}", toml);
/// ```
pub fn convert_zsh_completion(zsh_content: &str) -> Result<String> {
    let completion = parse_zsh_completion(zsh_content)?;
    Ok(generate_toml(&completion))
}

/// Parse zsh completion content.
fn parse_zsh_completion(content: &str) -> Result<ZshCompletion> {
    let mut command = String::new();
    let mut description = None;
    let mut options = Vec::new();
    let mut subcommands = HashMap::new();

    // Find #compdef line to get command name
    let compdef_re = Regex::new(r"#compdef\s+(\S+)").unwrap();
    if let Some(cap) = compdef_re.captures(content) {
        command = cap[1].to_string();
    }

    // Find description comment
    let desc_re = Regex::new(r"#\s*Description:\s*(.+)").unwrap();
    if let Some(cap) = desc_re.captures(content) {
        description = Some(cap[1].trim().to_string());
    }

    // Parse _arguments calls
    let args_re = Regex::new(r"_arguments\s*\\?\s*((?:.*\\?\s*)+)").unwrap();
    for cap in args_re.captures_iter(content) {
        let args_block = &cap[1];
        parse_arguments_block(args_block, &mut options);
    }

    // Parse subcommands from _describe or case statements
    parse_subcommands(content, &mut subcommands);

    if command.is_empty() {
        anyhow::bail!("No #compdef directive found");
    }

    Ok(ZshCompletion {
        command,
        description,
        options,
        subcommands,
    })
}

/// Parse _arguments block for options.
fn parse_arguments_block(block: &str, options: &mut Vec<ZshOption>) {
    // Match various _arguments patterns:
    // '-h[Help]'
    // '--help[Show help]'
    // '-f[File]:file:_files'
    // '(-v --verbose)'{-v,--verbose}'[Verbose]'
    // '--config=[Config file]:config:_files'

    let simple_opt = Regex::new(r"'(-{1,2}[a-zA-Z0-9_-]+)(?:=)?\[([^\]]*)\](?::([^:]+):([^']+))?'")
        .unwrap();

    let combined_opt =
        Regex::new(r"'\([^)]+\)'\{([^}]+)\}'\[([^\]]*)\](?::([^:]+):([^']+))?'").unwrap();

    // Simple options
    for cap in simple_opt.captures_iter(block) {
        let name = cap[1].to_string();
        let description = cap[2].to_string();
        let takes_value = cap.get(3).is_some() || name.ends_with('=');

        options.push(ZshOption {
            name: name.trim_end_matches('=').to_string(),
            description,
            takes_value,
        });
    }

    // Combined options like '{-v,--verbose}'
    for cap in combined_opt.captures_iter(block) {
        let names: Vec<&str> = cap[1].split(',').collect();
        let description = cap[2].to_string();
        let takes_value = cap.get(3).is_some();

        for name in names {
            options.push(ZshOption {
                name: name.trim().to_string(),
                description: description.clone(),
                takes_value,
            });
        }
    }
}

/// Parse subcommands from _describe calls or case statements.
fn parse_subcommands(content: &str, subcommands: &mut HashMap<String, String>) {
    // Match _describe patterns:
    // _describe 'commands' commands
    // Where commands might be defined as:
    // commands=('cmd1:description1' 'cmd2:description2')

    let array_re = Regex::new(r"(\w+)=\(\s*((?:'[^']+'\s*)+)\)").unwrap();
    let entry_re = Regex::new(r"'([^:]+):([^']+)'").unwrap();

    for cap in array_re.captures_iter(content) {
        let entries = &cap[2];
        for entry_cap in entry_re.captures_iter(entries) {
            let name = entry_cap[1].trim().to_string();
            let desc = entry_cap[2].trim().to_string();
            subcommands.insert(name, desc);
        }
    }

    // Also try to match case statement patterns:
    // cmd) _description ;;
    let case_re = Regex::new(r"(\w+)\)\s*(?:#\s*(.+))?").unwrap();
    for cap in case_re.captures_iter(content) {
        if let Some(desc) = cap.get(2) {
            let name = cap[1].to_string();
            if !subcommands.contains_key(&name) {
                subcommands.insert(name, desc.as_str().trim().to_string());
            }
        }
    }
}

/// Generate TOML output from parsed completion.
fn generate_toml(completion: &ZshCompletion) -> String {
    let mut output = String::new();

    // Header
    output.push_str(&format!(
        "# Converted from zsh completion for {}\n\n",
        completion.command
    ));

    // Command completion section
    output.push_str(&format!("[completions.{}]\n", completion.command));

    if let Some(ref desc) = completion.description {
        output.push_str(&format!("description = {:?}\n", desc));
    }

    // Subcommands
    if !completion.subcommands.is_empty() {
        output.push_str(&format!(
            "\n[completions.{}.subcommands]\n",
            completion.command
        ));
        for (name, desc) in &completion.subcommands {
            output.push_str(&format!("{} = {:?}\n", name, desc));
        }
    }

    // Options
    if !completion.options.is_empty() {
        output.push_str(&format!("\n[completions.{}.options]\n", completion.command));
        for opt in &completion.options {
            if opt.takes_value {
                output.push_str(&format!(
                    "{:?} = {{ description = {:?}, takes_value = true }}\n",
                    opt.name, opt.description
                ));
            } else {
                output.push_str(&format!("{:?} = {:?}\n", opt.name, opt.description));
            }
        }
    }

    output
}

/// Read a zsh completion file and convert it.
pub fn convert_zsh_file(path: &std::path::Path) -> Result<String> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    convert_zsh_completion(&content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_compdef() {
        let content = "#compdef mycommand\n_arguments '-h[Help]'";
        let result = parse_zsh_completion(content).unwrap();
        assert_eq!(result.command, "mycommand");
    }

    #[test]
    fn test_parse_simple_option() {
        let content = "#compdef test\n_arguments '-h[Show help]' '-v[Verbose]'";
        let result = parse_zsh_completion(content).unwrap();
        assert_eq!(result.options.len(), 2);
        assert_eq!(result.options[0].name, "-h");
        assert_eq!(result.options[0].description, "Show help");
    }

    #[test]
    fn test_parse_option_with_value() {
        let content = "#compdef test\n_arguments '--config[Config file]:file:_files'";
        let result = parse_zsh_completion(content).unwrap();
        assert_eq!(result.options.len(), 1);
        assert!(result.options[0].takes_value);
    }

    #[test]
    fn test_generate_toml() {
        let completion = ZshCompletion {
            command: "mycmd".to_string(),
            description: Some("My command".to_string()),
            options: vec![ZshOption {
                name: "-h".to_string(),
                description: "Help".to_string(),
                takes_value: false,
            }],
            subcommands: HashMap::new(),
        };

        let toml = generate_toml(&completion);
        assert!(toml.contains("[completions.mycmd]"));
        assert!(toml.contains("description = \"My command\""));
        assert!(toml.contains("\"-h\" = \"Help\""));
    }
}
