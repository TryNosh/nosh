//! nosh-context - Efficient project context detection.
//!
//! This library provides fast, cached project context detection for shell prompts.
//! It detects:
//! - Git branch and status
//! - Package information (from Cargo.toml, package.json, etc.)
//! - Language/tool versions (Rust, Node.js, Go, Python)
//!
//! # Example
//!
//! ```no_run
//! use nosh_context::{ContextCache, ProjectContext};
//! use std::path::Path;
//!
//! let mut cache = ContextCache::new();
//! let ctx = cache.get(Path::new("."));
//! println!("Git branch: {:?}", ctx.git.as_ref().map(|g| &g.branch));
//! ```

mod cache;
mod context;
pub mod detectors;
mod scanner;

pub use cache::ContextCache;
pub use context::{GitInfo, PackageInfo, ProjectContext, ToolInfo};
pub use scanner::detect;
