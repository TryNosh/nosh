//! Mtime-based caching for project context.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime};

use crate::context::ProjectContext;
use crate::scanner::detect;

/// Cache for project context to avoid redundant detection.
pub struct ContextCache {
    cached: Option<CachedContext>,
}

struct CachedContext {
    dir: PathBuf,
    context: ProjectContext,
    file_mtimes: HashMap<String, SystemTime>,
    detected_at: Instant,
}

/// Indicator files to monitor for changes.
const INDICATOR_FILES: &[&str] = &[
    "Cargo.toml",
    "Cargo.lock",
    "package.json",
    "package-lock.json",
    "bun.lockb",
    "bun.lock",
    "bunfig.toml",
    "go.mod",
    "go.sum",
    "pyproject.toml",
    "setup.py",
    "requirements.txt",
    "CMakeLists.txt",
    "meson.build",
    "Dockerfile",
    "docker-compose.yml",
    "docker-compose.yaml",
    "compose.yml",
    "compose.yaml",
    ".git/HEAD",
    ".git/index",
];

/// Maximum cache age in seconds before forcing refresh (for version info).
const MAX_CACHE_AGE_SECS: u64 = 5;

impl ContextCache {
    /// Create a new empty cache.
    pub fn new() -> Self {
        Self { cached: None }
    }

    /// Get project context, using cache if valid.
    pub fn get(&mut self, dir: &Path) -> ProjectContext {
        // Canonicalize path for consistent comparison
        let dir = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());

        // Check if cache is valid
        if let Some(cached) = &self.cached
            && cached.dir == dir
            && !self.cache_expired(&cached.detected_at)
            && !self.files_changed(&dir, &cached.file_mtimes)
        {
            return cached.context.clone();
        }

        // Cache miss - detect fresh
        let context = detect(&dir);
        let file_mtimes = self.collect_mtimes(&dir);

        self.cached = Some(CachedContext {
            dir: dir.clone(),
            context: context.clone(),
            file_mtimes,
            detected_at: Instant::now(),
        });

        context
    }

    /// Invalidate the cache.
    pub fn invalidate(&mut self) {
        self.cached = None;
    }

    /// Check if cache has expired.
    fn cache_expired(&self, detected_at: &Instant) -> bool {
        detected_at.elapsed().as_secs() > MAX_CACHE_AGE_SECS
    }

    /// Check if any indicator files have changed.
    fn files_changed(&self, dir: &Path, old_mtimes: &HashMap<String, SystemTime>) -> bool {
        for file in INDICATOR_FILES {
            let path = dir.join(file);
            let old_mtime = old_mtimes.get(*file);

            match (path.exists(), old_mtime) {
                // File exists now, didn't before
                (true, None) => return true,
                // File doesn't exist now, did before
                (false, Some(_)) => return true,
                // File exists - check mtime
                (true, Some(old)) => {
                    if let Ok(meta) = fs::metadata(&path)
                        && let Ok(new_mtime) = meta.modified()
                        && &new_mtime != old
                    {
                        return true;
                    }
                }
                // File doesn't exist and didn't before - no change
                (false, None) => {}
            }
        }
        false
    }

    /// Collect modification times for indicator files.
    fn collect_mtimes(&self, dir: &Path) -> HashMap<String, SystemTime> {
        let mut mtimes = HashMap::new();

        for file in INDICATOR_FILES {
            let path = dir.join(file);
            if let Ok(meta) = fs::metadata(&path)
                && let Ok(mtime) = meta.modified()
            {
                mtimes.insert(file.to_string(), mtime);
            }
        }

        mtimes
    }
}

impl Default for ContextCache {
    fn default() -> Self {
        Self::new()
    }
}
