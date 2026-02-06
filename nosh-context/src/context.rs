//! Project context types.
//!
//! Defines the core data structures for project context information.

use serde::{Deserialize, Serialize};

/// Complete project context information.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectContext {
    /// Current directory path.
    pub dir: String,
    /// Git repository information.
    pub git: Option<GitInfo>,
    /// Package/project information.
    pub package: Option<PackageInfo>,
    /// Rust toolchain information.
    pub rust: Option<ToolInfo>,
    /// Node.js toolchain information.
    pub node: Option<ToolInfo>,
    /// Bun runtime information.
    pub bun: Option<ToolInfo>,
    /// Go toolchain information.
    pub go: Option<ToolInfo>,
    /// Python toolchain information.
    pub python: Option<ToolInfo>,
    /// C++ toolchain information.
    pub cpp: Option<ToolInfo>,
    /// Docker toolchain information.
    pub docker: Option<ToolInfo>,
}

/// Git repository status information.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GitInfo {
    /// Current branch name.
    pub branch: String,
    /// Whether there are uncommitted changes.
    pub dirty: bool,
    /// Whether there are staged changes.
    pub staged: bool,
    /// Whether there are untracked files.
    pub untracked: bool,
}

impl GitInfo {
    /// Format git status as a short indicator string (e.g., "[!?]").
    pub fn status_indicator(&self) -> String {
        let mut s = String::new();
        if self.staged {
            s.push('!');
        }
        if self.untracked {
            s.push('?');
        }
        if self.dirty && s.is_empty() {
            s.push('*');
        }
        if s.is_empty() {
            String::new()
        } else {
            format!("[{}]", s)
        }
    }
}

/// Package/project metadata.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PackageInfo {
    /// Package name.
    pub name: String,
    /// Package version.
    pub version: String,
}

/// Tool/language runtime information.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolInfo {
    /// Version string.
    pub version: String,
}
