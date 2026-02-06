//! SQLite-backed history for rustyline with lazy loading.
//!
//! Implements rustyline's History trait, loading entries on-demand from SQLite
//! as the user navigates through history with arrow keys.

use rustyline::history::{History, SearchDirection, SearchResult};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use crate::history::History as SqliteHistory;

/// Batch size for loading history entries.
const BATCH_SIZE: usize = 100;

/// SQLite-backed history with lazy loading.
///
/// Instead of loading all history upfront, this loads entries in batches
/// as the user navigates backwards through history.
pub struct SqliteRustylineHistory {
    /// The underlying SQLite history store
    db: Arc<SqliteHistory>,
    /// Total number of entries in the database
    total_count: RefCell<usize>,
    /// Cached entries: index -> command
    /// Index 0 is the oldest loaded entry, higher indices are more recent
    cache: RefCell<HashMap<usize, String>>,
    /// How many entries we've loaded from the database
    loaded_count: RefCell<usize>,
    /// Commands added during this session (newest at end)
    session_entries: RefCell<Vec<String>>,
}

impl SqliteRustylineHistory {
    /// Create a new SQLite-backed history.
    pub fn open(path: &Path) -> Result<Self, String> {
        let db = SqliteHistory::open(path)
            .map_err(|e| e.to_string())?;

        let total = db.count().unwrap_or(0) as usize;

        Ok(Self {
            db: Arc::new(db),
            total_count: RefCell::new(total),
            cache: RefCell::new(HashMap::new()),
            loaded_count: RefCell::new(0),
            session_entries: RefCell::new(Vec::new()),
        })
    }

    /// Get the underlying database for direct operations.
    pub fn db(&self) -> &SqliteHistory {
        &self.db
    }

    /// Ensure we have entries loaded up to the given index.
    fn ensure_loaded(&self, index: usize) {
        let session_len = self.session_entries.borrow().len();
        let total_db = *self.total_count.borrow();
        let total = total_db + session_len;

        if index >= total {
            return;
        }

        // If index is in session entries (at the end), we already have it
        if index >= total_db {
            return;
        }

        // Check if we need to load more from database
        let loaded = *self.loaded_count.borrow();
        if index >= loaded {
            // Load more entries
            let need_up_to = index + 1;
            let batch_count = ((need_up_to - loaded) / BATCH_SIZE) + 1;
            let load_count = loaded + (batch_count * BATCH_SIZE);
            let load_count = load_count.min(total_db);

            if let Ok(entries) = self.db.for_readline(load_count) {
                let mut cache = self.cache.borrow_mut();
                for (i, entry) in entries.into_iter().enumerate() {
                    cache.insert(i, entry);
                }
                *self.loaded_count.borrow_mut() = load_count;
            }
        }
    }
}

impl History for SqliteRustylineHistory {
    fn get(&self, index: usize, _dir: SearchDirection) -> Result<Option<SearchResult<'_>>, rustyline::error::ReadlineError> {
        self.ensure_loaded(index);

        let session_entries = self.session_entries.borrow();
        let total_db = *self.total_count.borrow();

        let entry = if index >= total_db {
            // It's a session entry
            let session_idx = index - total_db;
            session_entries.get(session_idx).cloned()
        } else {
            // It's a database entry
            let cache = self.cache.borrow();
            cache.get(&index).cloned()
        };

        Ok(entry.map(|e| SearchResult {
            entry: e.into(),
            idx: index,
            pos: 0,
        }))
    }

    fn add(&mut self, line: &str) -> Result<bool, rustyline::error::ReadlineError> {
        if line.is_empty() {
            return Ok(false);
        }

        // Add to SQLite immediately for persistence
        let _ = self.db.add(line);

        // Add to session entries for immediate access via arrow keys
        // Note: len() = total_count + session_entries.len(), so we don't
        // increment total_count here - session_entries already extends length
        self.session_entries.borrow_mut().push(line.to_string());

        Ok(true)
    }

    fn add_owned(&mut self, line: String) -> Result<bool, rustyline::error::ReadlineError> {
        self.add(&line)
    }

    fn len(&self) -> usize {
        *self.total_count.borrow() + self.session_entries.borrow().len()
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn set_max_len(&mut self, _len: usize) -> Result<(), rustyline::error::ReadlineError> {
        // SQLite doesn't need a max length - it can handle millions of entries
        Ok(())
    }

    fn ignore_dups(&mut self, _yes: bool) -> Result<(), rustyline::error::ReadlineError> {
        // Deduplication is handled at query time
        Ok(())
    }

    fn ignore_space(&mut self, _yes: bool) {
        // Not implemented - we store all commands
    }

    fn save(&mut self, _path: &Path) -> Result<(), rustyline::error::ReadlineError> {
        // Already saved to SQLite on each add()
        Ok(())
    }

    fn load(&mut self, _path: &Path) -> Result<(), rustyline::error::ReadlineError> {
        // SQLite is already loaded
        Ok(())
    }

    fn append(&mut self, _path: &Path) -> Result<(), rustyline::error::ReadlineError> {
        // Not applicable for SQLite
        Ok(())
    }

    fn search(
        &self,
        term: &str,
        start: usize,
        dir: SearchDirection,
    ) -> Result<Option<SearchResult<'_>>, rustyline::error::ReadlineError> {
        // Use SQLite's search capability for Ctrl+R
        if let Ok(results) = self.db.search(term, 100) {
            if !results.is_empty() {
                // Find the entry and return its position
                for (i, entry) in results.iter().enumerate() {
                    let idx = match dir {
                        SearchDirection::Forward => start + i,
                        SearchDirection::Reverse => {
                            if start >= i { start - i } else { 0 }
                        }
                    };
                    if entry.contains(term) {
                        return Ok(Some(SearchResult {
                            entry: entry.clone().into(),
                            idx,
                            pos: entry.find(term).unwrap_or(0),
                        }));
                    }
                }
            }
        }

        Ok(None)
    }

    fn starts_with(
        &self,
        term: &str,
        start: usize,
        dir: SearchDirection,
    ) -> Result<Option<SearchResult<'_>>, rustyline::error::ReadlineError> {
        // Use the search function to find entries starting with term
        let total = self.len();
        if total == 0 {
            return Ok(None);
        }

        let range: Box<dyn Iterator<Item = usize>> = match dir {
            SearchDirection::Forward => Box::new(start..total),
            SearchDirection::Reverse => Box::new((0..=start).rev()),
        };

        for idx in range {
            if let Ok(Some(result)) = self.get(idx, dir) {
                if result.entry.starts_with(term) {
                    return Ok(Some(SearchResult {
                        entry: result.entry,
                        idx,
                        pos: 0,
                    }));
                }
            }
        }

        Ok(None)
    }

    fn clear(&mut self) -> Result<(), rustyline::error::ReadlineError> {
        let _ = self.db.clear();
        self.cache.borrow_mut().clear();
        self.session_entries.borrow_mut().clear();
        *self.total_count.borrow_mut() = 0;
        *self.loaded_count.borrow_mut() = 0;
        Ok(())
    }
}
