use anyhow::Result;
use rustyline::error::ReadlineError;
use rustyline::history::History;
use rustyline::{Cmd, Config, Editor, EventHandler, KeyCode, KeyEvent, Modifiers};
use std::time::Instant;

use super::sqlite_history::SqliteRustylineHistory;
use crate::paths;
use crate::plugins::loader::PluginManager;
use crate::plugins::theme::Theme;

pub struct Repl {
    editor: Editor<(), SqliteRustylineHistory>,
    plugin_manager: PluginManager,
    theme: Theme,
    last_command_start: Option<Instant>,
}

impl Repl {
    pub fn new(theme_name: &str, _history_load_count: Option<usize>) -> Result<Self> {
        // Create SQLite-backed history with lazy loading
        let history = SqliteRustylineHistory::open(&paths::history_db())
            .map_err(|e| anyhow::anyhow!("Failed to open history: {}", e))?;

        // Configure rustyline with our SQLite history
        let config = Config::builder()
            .auto_add_history(false) // We handle this manually
            .build();
        let mut editor = Editor::with_history(config, history)?;

        // Bind Up/Down arrows to prefix-based history search
        // When there's text before the cursor, search for matching prefix
        editor.bind_sequence(
            KeyEvent(KeyCode::Up, Modifiers::NONE),
            EventHandler::Simple(Cmd::HistorySearchBackward),
        );
        editor.bind_sequence(
            KeyEvent(KeyCode::Down, Modifiers::NONE),
            EventHandler::Simple(Cmd::HistorySearchForward),
        );

        // Load plugins and theme
        let mut plugin_manager = PluginManager::new();
        let _ = plugin_manager.load_plugins();

        let theme = Theme::load(theme_name).unwrap_or_default();

        Ok(Self {
            editor,
            plugin_manager,
            theme,
            last_command_start: None,
        })
    }

    /// No-op: SQLite history loads lazily on demand.
    pub fn load_history(&mut self) {
        // History is loaded lazily as user navigates with arrow keys.
        // No upfront loading needed.
    }

    /// No-op: SQLite history is saved in real-time.
    pub fn save_history(&mut self) {
        // History is saved immediately on each command.
        // Nothing to do here.
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
                    // Add to history (SQLite handles persistence)
                    let _ = self.editor.history_mut().add(&line);
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
