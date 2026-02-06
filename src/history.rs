//! SQLite-based command history with multi-session support.
//!
//! Each command is stored with a timestamp, allowing multiple nosh sessions
//! to share history in real-time without overwriting each other's entries.

use anyhow::Result;
use rusqlite::{Connection, params};
use std::path::Path;

/// SQLite-backed command history.
pub struct History {
    conn: Connection,
    /// Session ID for tracking which session added which commands
    session_id: i64,
}

impl History {
    /// Open or create the history database.
    pub fn open(path: &Path) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(path)?;

        // Enable WAL mode for better concurrent access
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;

        // Create tables if they don't exist
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                command TEXT NOT NULL,
                timestamp INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
                cwd TEXT,
                session_id INTEGER
            );
            CREATE INDEX IF NOT EXISTS idx_history_timestamp ON history(timestamp DESC);
            CREATE INDEX IF NOT EXISTS idx_history_command ON history(command);

            CREATE TABLE IF NOT EXISTS sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                started_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
                pid INTEGER
            );"
        )?;

        // Register this session
        let pid = std::process::id() as i64;
        conn.execute(
            "INSERT INTO sessions (pid) VALUES (?1)",
            params![pid],
        )?;
        let session_id = conn.last_insert_rowid();

        Ok(Self { conn, session_id })
    }

    /// Add a command to history.
    pub fn add(&self, command: &str) -> Result<()> {
        let cwd = std::env::current_dir()
            .ok()
            .and_then(|p| p.to_str().map(String::from));

        self.conn.execute(
            "INSERT INTO history (command, cwd, session_id) VALUES (?1, ?2, ?3)",
            params![command, cwd, self.session_id],
        )?;

        Ok(())
    }

    /// Get the N most recent commands, newest first.
    pub fn recent(&self, limit: usize) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT command FROM history
             ORDER BY timestamp DESC, id DESC
             LIMIT ?1"
        )?;

        let commands = stmt
            .query_map(params![limit as i64], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;

        Ok(commands)
    }

    /// Get commands for rustyline history (oldest first for proper navigation).
    pub fn for_readline(&self, limit: usize) -> Result<Vec<String>> {
        let mut commands = self.recent(limit)?;
        commands.reverse(); // Oldest first for readline
        Ok(commands)
    }

    /// Search history for commands containing the pattern.
    pub fn search(&self, pattern: &str, limit: usize) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT command FROM history
             WHERE command LIKE ?1
             ORDER BY timestamp DESC, id DESC
             LIMIT ?2"
        )?;

        let search_pattern = format!("%{}%", pattern);
        let commands = stmt
            .query_map(params![search_pattern, limit as i64], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;

        Ok(commands)
    }

    /// Get commands run in a specific directory.
    #[allow(dead_code)]
    pub fn in_directory(&self, dir: &str, limit: usize) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT command FROM history
             WHERE cwd = ?1 OR cwd LIKE ?2
             ORDER BY timestamp DESC, id DESC
             LIMIT ?3"
        )?;

        let dir_pattern = format!("{}/%", dir);
        let commands = stmt
            .query_map(params![dir, dir_pattern, limit as i64], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;

        Ok(commands)
    }

    /// Get total number of unique commands in history.
    pub fn count(&self) -> Result<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(DISTINCT command) FROM history",
            [],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Clear all history.
    pub fn clear(&self) -> Result<()> {
        self.conn.execute("DELETE FROM history", [])?;
        Ok(())
    }

    /// Remove duplicate consecutive commands (keeps the most recent).
    #[allow(dead_code)]
    pub fn deduplicate(&self) -> Result<usize> {
        let deleted = self.conn.execute(
            "DELETE FROM history WHERE id NOT IN (
                SELECT MAX(id) FROM history GROUP BY command
            )",
            [],
        )?;
        Ok(deleted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_db() -> PathBuf {
        let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let mut path = std::env::temp_dir();
        path.push(format!("nosh_test_{}_{}.db", std::process::id(), id));
        // Remove any existing file from previous test run
        std::fs::remove_file(&path).ok();
        path
    }

    #[test]
    fn test_add_and_recent() {
        let path = temp_db();
        let history = History::open(&path).unwrap();

        history.add("ls").unwrap();
        history.add("pwd").unwrap();
        history.add("git status").unwrap();

        let recent = history.recent(10).unwrap();
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0], "git status"); // Most recent first
        assert_eq!(recent[2], "ls");

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_search() {
        let path = temp_db();
        let history = History::open(&path).unwrap();

        history.add("git status").unwrap();
        history.add("git log").unwrap();
        history.add("ls -la").unwrap();
        history.add("git push").unwrap();

        let git_commands = history.search("git", 10).unwrap();
        assert_eq!(git_commands.len(), 3);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_for_readline() {
        let path = temp_db();
        let history = History::open(&path).unwrap();

        history.add("first").unwrap();
        history.add("second").unwrap();
        history.add("third").unwrap();

        let for_rl = history.for_readline(10).unwrap();
        assert_eq!(for_rl[0], "first"); // Oldest first for readline
        assert_eq!(for_rl[2], "third");

        std::fs::remove_file(&path).ok();
    }
}
