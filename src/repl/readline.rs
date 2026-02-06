use anyhow::Result;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::path::PathBuf;
use std::time::Instant;

use crate::paths;
use crate::plugins::loader::PluginManager;
use crate::plugins::theme::Theme;

pub struct Repl {
    editor: DefaultEditor,
    history_path: PathBuf,
    plugin_manager: PluginManager,
    theme: Theme,
    last_command_start: Option<Instant>,
}

impl Repl {
    pub fn new(theme_name: &str) -> Result<Self> {
        let editor = DefaultEditor::new()?;
        let history_path = paths::history_file();

        // Load plugins and theme
        let mut plugin_manager = PluginManager::new();
        let _ = plugin_manager.load_plugins();

        let theme = Theme::load(theme_name).unwrap_or_default();

        Ok(Self {
            editor,
            history_path,
            plugin_manager,
            theme,
            last_command_start: None,
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

    /// Mark the start of a command execution.
    pub fn start_command(&mut self) {
        self.last_command_start = Some(Instant::now());
    }

    /// Mark the end of a command execution and record duration.
    pub fn end_command(&mut self) {
        if let Some(start) = self.last_command_start.take() {
            let duration = start.elapsed();
            self.plugin_manager.set_last_command_duration(duration);
        }
    }

    pub fn prompt(&mut self) -> String {
        // Invalidate plugin cache for fresh values
        self.plugin_manager.invalidate_cache();

        // Format prompt using theme
        self.theme.format_prompt(&mut self.plugin_manager)
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

    /// Reload theme and plugins from disk.
    pub fn reload(&mut self, theme_name: &str) {
        // Reload plugins
        self.plugin_manager = PluginManager::new();
        let _ = self.plugin_manager.load_plugins();

        // Reload theme
        self.theme = Theme::load(theme_name).unwrap_or_default();
    }
}
