//! Git repository detection.

use std::fs;
use std::path::Path;
use std::process::Command;

use crate::context::GitInfo;

/// Detect git repository information.
pub fn detect(dir: &Path) -> Option<GitInfo> {
    // Try to get branch from git command first (most reliable)
    let branch = get_branch_from_command(dir).or_else(|| get_branch_from_head(dir))?;

    // Get status information
    let (dirty, staged, untracked) = get_status(dir);

    Some(GitInfo {
        branch,
        dirty,
        staged,
        untracked,
    })
}

/// Get current branch using git command.
fn get_branch_from_command(dir: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(dir)
        .output()
        .ok()?;

    if output.status.success() {
        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !branch.is_empty() {
            return Some(branch);
        }
    }

    // Fallback for detached HEAD - try to get commit hash
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(dir)
        .output()
        .ok()?;

    if output.status.success() {
        let hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !hash.is_empty() {
            return Some(format!(":{}", hash));
        }
    }

    None
}

/// Get current branch by reading .git/HEAD directly.
fn get_branch_from_head(dir: &Path) -> Option<String> {
    // Find .git directory (could be in parent)
    let git_dir = find_git_dir(dir)?;
    let head_path = git_dir.join("HEAD");

    let content = fs::read_to_string(head_path).ok()?;
    let content = content.trim();

    // Parse "ref: refs/heads/branch-name"
    if let Some(ref_path) = content.strip_prefix("ref: refs/heads/") {
        return Some(ref_path.to_string());
    }

    // Detached HEAD - return short hash
    if content.len() >= 7 {
        return Some(format!(":{}", &content[..7]));
    }

    None
}

/// Find the .git directory (handles worktrees and submodules).
fn find_git_dir(dir: &Path) -> Option<std::path::PathBuf> {
    let mut current = dir.to_path_buf();
    loop {
        let git_path = current.join(".git");
        if git_path.is_dir() {
            return Some(git_path);
        }
        // Handle git worktrees: .git is a file containing "gitdir: /path/to/git"
        if git_path.is_file()
            && let Ok(content) = fs::read_to_string(&git_path)
            && let Some(gitdir) = content.trim().strip_prefix("gitdir: ")
        {
            return Some(std::path::PathBuf::from(gitdir));
        }
        if !current.pop() {
            break;
        }
    }
    None
}

/// Get repository status (dirty, staged, untracked).
fn get_status(dir: &Path) -> (bool, bool, bool) {
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(dir)
        .output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return (false, false, false),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);

    let mut dirty = false;
    let mut staged = false;
    let mut untracked = false;

    for line in stdout.lines() {
        if line.len() < 2 {
            continue;
        }

        let index_status = line.chars().next().unwrap_or(' ');
        let worktree_status = line.chars().nth(1).unwrap_or(' ');

        // Untracked files
        if index_status == '?' {
            untracked = true;
            continue;
        }

        // Staged changes (index has changes)
        if index_status != ' ' && index_status != '?' {
            staged = true;
        }

        // Worktree changes (unstaged modifications)
        if worktree_status != ' ' {
            dirty = true;
        }
    }

    (dirty, staged, untracked)
}
