//! SQLite history with stable row-ID snapshots and lazy command loading.
use rustyline::history::{History, SearchDirection, SearchResult};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;

use crate::history::History as SqliteHistory;

pub struct SqliteRustylineHistory {
    db: SqliteHistory,
    ids: Vec<i64>,
    cache: RefCell<HashMap<i64, String>>,
    ignore_space: bool,
    ignore_dups: bool,
}

fn history_error(error: impl std::fmt::Display) -> rustyline::error::ReadlineError {
    std::io::Error::other(error.to_string()).into()
}

impl SqliteRustylineHistory {
    pub fn open(path: &Path) -> Result<Self, String> {
        let db = SqliteHistory::open(path).map_err(|e| e.to_string())?;
        let ids = db.snapshot_ids().map_err(|e| e.to_string())?;
        Ok(Self {
            db,
            ids,
            cache: RefCell::new(HashMap::new()),
            ignore_space: false,
            ignore_dups: false,
        })
    }

    /// Include writes from other shells without changing indices during editing.
    pub fn refresh(&mut self, prefetch: usize) -> rustyline::Result<()> {
        self.ids = self.db.snapshot_ids().map_err(history_error)?;
        self.cache.borrow_mut().clear();
        for index in self.ids.len().saturating_sub(prefetch)..self.ids.len() {
            self.get(index, SearchDirection::Forward)?;
        }
        Ok(())
    }

    fn search_entries(
        &self,
        term: &str,
        start: usize,
        dir: SearchDirection,
        anchored: bool,
    ) -> rustyline::Result<Option<SearchResult<'_>>> {
        if self.is_empty() || start >= self.len() {
            return Ok(None);
        }
        let indices: Box<dyn Iterator<Item = usize>> = match dir {
            SearchDirection::Forward => Box::new(start..self.len()),
            SearchDirection::Reverse => Box::new((0..=start).rev()),
        };
        for index in indices {
            if let Some(mut result) = self.get(index, dir)? {
                let position = if anchored {
                    result.entry.starts_with(term).then_some(0)
                } else {
                    result.entry.find(term)
                };
                if let Some(position) = position {
                    result.pos = if anchored { term.len() } else { position };
                    return Ok(Some(result));
                }
            }
        }
        Ok(None)
    }
}

impl History for SqliteRustylineHistory {
    fn get(
        &self,
        index: usize,
        _dir: SearchDirection,
    ) -> rustyline::Result<Option<SearchResult<'_>>> {
        let Some(&id) = self.ids.get(index) else {
            return Ok(None);
        };
        let cached = self.cache.borrow().get(&id).cloned();
        let entry = match cached {
            Some(entry) => Some(entry),
            None => self.db.entry(id).map_err(history_error)?,
        };
        Ok(entry.map(|entry| {
            self.cache.borrow_mut().insert(id, entry.clone());
            SearchResult {
                entry: entry.into(),
                idx: index,
                pos: 0,
            }
        }))
    }

    fn add(&mut self, line: &str) -> rustyline::Result<bool> {
        if line.is_empty() || (self.ignore_space && line.starts_with(char::is_whitespace)) {
            return Ok(false);
        }
        if self.ignore_dups
            && !self.ids.is_empty()
            && self
                .get(self.ids.len() - 1, SearchDirection::Reverse)?
                .is_some_and(|last| last.entry == line)
        {
            return Ok(false);
        }
        self.db.add(line).map_err(history_error)?;
        self.refresh(0)?;
        Ok(true)
    }
    fn add_owned(&mut self, line: String) -> rustyline::Result<bool> {
        self.add(&line)
    }
    fn len(&self) -> usize {
        self.ids.len()
    }
    fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }
    fn set_max_len(&mut self, _len: usize) -> rustyline::Result<()> {
        Ok(())
    }
    fn ignore_dups(&mut self, yes: bool) -> rustyline::Result<()> {
        self.ignore_dups = yes;
        Ok(())
    }
    fn ignore_space(&mut self, yes: bool) {
        self.ignore_space = yes;
    }
    fn save(&mut self, _path: &Path) -> rustyline::Result<()> {
        Ok(())
    }
    fn load(&mut self, _path: &Path) -> rustyline::Result<()> {
        self.refresh(0)
    }
    fn append(&mut self, _path: &Path) -> rustyline::Result<()> {
        Ok(())
    }
    fn search(
        &self,
        term: &str,
        start: usize,
        dir: SearchDirection,
    ) -> rustyline::Result<Option<SearchResult<'_>>> {
        self.search_entries(term, start, dir, false)
    }
    fn starts_with(
        &self,
        term: &str,
        start: usize,
        dir: SearchDirection,
    ) -> rustyline::Result<Option<SearchResult<'_>>> {
        self.search_entries(term, start, dir, true)
    }
    fn clear(&mut self) -> rustyline::Result<()> {
        self.db.clear().map_err(history_error)?;
        self.ids.clear();
        self.cache.borrow_mut().clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stable_indices_search_and_shared_sessions() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.db");
        let db = SqliteHistory::open(&path).unwrap();
        for i in 0..250 {
            db.add(&format!("entry-{i:03}")).unwrap();
        }
        let mut history = SqliteRustylineHistory::open(&path).unwrap();
        assert_eq!(
            history
                .get(0, SearchDirection::Forward)
                .unwrap()
                .unwrap()
                .entry,
            "entry-000"
        );
        history.get(249, SearchDirection::Reverse).unwrap();
        assert_eq!(
            history
                .get(0, SearchDirection::Forward)
                .unwrap()
                .unwrap()
                .entry,
            "entry-000"
        );
        assert_eq!(
            history
                .search("entry", 0, SearchDirection::Reverse)
                .unwrap()
                .unwrap()
                .entry,
            "entry-000"
        );
        assert_eq!(
            history
                .search("entry", 248, SearchDirection::Reverse)
                .unwrap()
                .unwrap()
                .entry,
            "entry-248"
        );
        db.add("another session").unwrap();
        history.refresh(10).unwrap();
        assert_eq!(history.len(), 251);
        assert_eq!(
            history
                .get(250, SearchDirection::Reverse)
                .unwrap()
                .unwrap()
                .entry,
            "another session"
        );
        history.add("entry-000").unwrap();
        assert_eq!(history.len(), 251);
        assert_eq!(
            history
                .get(250, SearchDirection::Reverse)
                .unwrap()
                .unwrap()
                .entry,
            "entry-000"
        );
    }
}
