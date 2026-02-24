use std::rc::Rc;
use std::time::Instant;

use anyhow::Result;
use rustyline::error::ReadlineError;
use rustyline::history::History;
use rustyline::{Cmd, Config, Editor, EventHandler, KeyCode, KeyEvent, Modifiers};

use super::helper::NoshHelper;
use super::sqlite_history::SqliteRustylineHistory;
use crate::completions::CompletionManager;
use crate::paths;
use crate::plugins::loader::PluginManager;
use crate::plugins::theme::Theme;

/// Result of a readline operation
pub enum ReadlineResult {
    /// User entered a line (may be empty)
    Line(String),
    /// User pressed Ctrl+C (interrupt - show new prompt)
    Interrupted,
    /// User pressed Ctrl+D (EOF - exit shell)
    Eof,
}

pub struct Repl {
    editor: Editor<NoshHelper, SqliteRustylineHistory>,
    plugin_manager: PluginManager,
    theme: Theme,
    last_command_start: Option<Instant>,
    #[allow(dead_code)]
    completion_manager: Rc<CompletionManager>,
}

impl Repl {
    pub fn new(
        theme_name: &str,
        _history_load_count: Option<usize>,
        syntax_highlighting: bool,
    ) -> Result<Self> {
        // Create SQLite-backed history with lazy loading
        let history = SqliteRustylineHistory::open(&paths::history_db())
            .map_err(|e| anyhow::anyhow!("Failed to open history: {}", e))?;

        // Create completion manager (lazy-loading)
        let completion_manager = Rc::new(CompletionManager::new());
        let helper = NoshHelper::new(Rc::clone(&completion_manager), syntax_highlighting);

        // Configure rustyline with our SQLite history and helper
        let config = Config::builder()
            .auto_add_history(false) // We handle this manually
            .completion_type(rustyline::CompletionType::List)
            .build();
        let mut editor = Editor::with_history(config, history)?;
        editor.set_helper(Some(helper));

        // Bind Up/Down arrows to prefix-based history search
        // If line has text, only show history entries starting with that text
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
            completion_manager,
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

    /// Generate the prompt string asynchronously.
    /// Uses parallel plugin execution with soft/hard timeouts.
    pub async fn prompt(&mut self) -> String {
        // Get all plugin variables needed from theme
        let vars = self.theme.get_plugin_variables();

        // Fetch all variables in parallel with soft timeout
        let values = self.plugin_manager.get_variables(vars).await;

        // Format prompt with fetched values
        self.theme
            .format_prompt_with_values(&values, &mut self.plugin_manager)
    }

    pub async fn readline(&mut self) -> Result<ReadlineResult> {
        let prompt = self.prompt().await;
        match self.editor.readline(&prompt) {
            Ok(line) => {
                let line = line.trim().to_string();
                if !line.is_empty() {
                    // Add to history (SQLite handles persistence)
                    let _ = self.editor.history_mut().add(&line);
                }
                Ok(ReadlineResult::Line(line))
            }
            Err(ReadlineError::Interrupted) => Ok(ReadlineResult::Interrupted), // Ctrl+C
            Err(ReadlineError::Eof) => Ok(ReadlineResult::Eof),                 // Ctrl+D
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

    /// List all loaded plugins.
    pub fn list_plugins(&self) -> Vec<(&str, &str, Vec<&str>)> {
        self.plugin_manager.list_plugins()
    }

    /// Debug a specific plugin.
    pub async fn debug_plugin(
        &self,
        plugin_name: &str,
    ) -> Option<Vec<(String, String, Result<String, String>)>> {
        self.plugin_manager.debug_plugin(plugin_name).await
    }

    /// Get variables used by current theme.
    pub fn theme_variables(&self) -> Vec<String> {
        self.theme.get_plugin_variables()
    }
}
