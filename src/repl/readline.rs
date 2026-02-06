use anyhow::Result;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::path::PathBuf;

pub struct Repl {
    editor: DefaultEditor,
    history_path: PathBuf,
}

impl Repl {
    pub fn new() -> Result<Self> {
        let editor = DefaultEditor::new()?;
        let history_path = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("nosh")
            .join("history");

        Ok(Self {
            editor,
            history_path,
        })
    }

    pub fn load_history(&mut self) {
        let _ = self.editor.load_history(&self.history_path);
    }

    pub fn save_history(&mut self) {
        if let Some(parent) = self.history_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = self.editor.save_history(&self.history_path);
    }

    pub fn prompt(&self) -> String {
        let cwd = std::env::current_dir()
            .ok()
            .and_then(|p| p.file_name().map(|s| s.to_string_lossy().to_string()))
            .unwrap_or_else(|| "~".to_string());
        format!("{} $ ", cwd)
    }

    pub fn readline(&mut self) -> Result<Option<String>> {
        let prompt = self.prompt();
        match self.editor.readline(&prompt) {
            Ok(line) => {
                let line = line.trim().to_string();
                if !line.is_empty() {
                    let _ = self.editor.add_history_entry(&line);
                }
                Ok(Some(line))
            }
            Err(ReadlineError::Interrupted) => Ok(None), // Ctrl+C
            Err(ReadlineError::Eof) => Ok(None),         // Ctrl+D
            Err(e) => Err(e.into()),
        }
    }
}
