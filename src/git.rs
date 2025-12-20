use std::path::Path;
use anyhow::{Result, bail};
use crate::app::BranchEntry;

/// List all branches (local and remote) for a git repository
pub async fn list_branches(repo_path: &Path) -> Result<Vec<BranchEntry>> {
    // Get local branches
    let local_output = tokio::process::Command::new("git")
        .args(["branch", "--format=%(refname:short)"])
        .current_dir(repo_path)
        .output()
        .await?;

    // Get current branch
    let current_output = tokio::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(repo_path)
        .output()
        .await?;

    let current_branch = String::from_utf8_lossy(&current_output.stdout)
        .trim()
        .to_string();

    let mut branches = Vec::new();

    // Parse local branches
    for line in String::from_utf8_lossy(&local_output.stdout).lines() {
        let name = line.trim().to_string();
        if !name.is_empty() {
            branches.push(BranchEntry {
                name: name.clone(),
                is_current: name == current_branch,
                is_remote: false,
            });
        }
    }

    // Get remote branches
    let remote_output = tokio::process::Command::new("git")
        .args(["branch", "-r", "--format=%(refname:short)"])
        .current_dir(repo_path)
        .output()
        .await?;

    for line in String::from_utf8_lossy(&remote_output.stdout).lines() {
        let name = line.trim().to_string();
        // Skip HEAD refs and extract branch name after origin/
        if !name.is_empty() && !name.contains("HEAD") {
            // Strip remote prefix for display (e.g., "origin/main" -> "main")
            let short_name = name.split('/').skip(1).collect::<Vec<_>>().join("/");
            // Only add if not already in local branches
            if !branches.iter().any(|b| b.name == short_name) {
                branches.push(BranchEntry {
                    name: short_name,
                    is_current: false,
                    is_remote: true,
                });
            }
        }
    }

    Ok(branches)
}

/// Check if a branch exists locally
pub async fn branch_exists(repo_path: &Path, branch_name: &str) -> Result<bool> {
    let output = tokio::process::Command::new("git")
        .args(["rev-parse", "--verify", &format!("refs/heads/{}", branch_name)])
        .current_dir(repo_path)
        .output()
        .await?;

    Ok(output.status.success())
}

/// Check if a branch exists as a remote tracking branch
pub async fn remote_branch_exists(repo_path: &Path, branch_name: &str) -> Result<bool> {
    // Check common remotes
    for remote in &["origin", "upstream"] {
        let output = tokio::process::Command::new("git")
            .args(["rev-parse", "--verify", &format!("refs/remotes/{}/{}", remote, branch_name)])
            .current_dir(repo_path)
            .output()
            .await?;

        if output.status.success() {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Create a git worktree
pub async fn create_worktree(
    repo_path: &Path,
    worktree_path: &Path,
    branch_name: &str,
    create_branch: bool,
) -> Result<()> {
    // Ensure parent directory exists
    if let Some(parent) = worktree_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let worktree_str = worktree_path.to_str()
        .ok_or_else(|| anyhow::anyhow!("Invalid worktree path"))?;

    let output = if create_branch {
        // Branch doesn't exist: git worktree add -b <branch> <path>
        tokio::process::Command::new("git")
            .args(["worktree", "add", "-b", branch_name, worktree_str])
            .current_dir(repo_path)
            .output()
            .await?
    } else {
        // Branch exists: git worktree add <path> <branch>
        tokio::process::Command::new("git")
            .args(["worktree", "add", worktree_str, branch_name])
            .current_dir(repo_path)
            .output()
            .await?
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Failed to create worktree: {}", stderr.trim());
    }

    Ok(())
}

/// Get repository name from path
pub fn repo_name(repo_path: &Path) -> String {
    repo_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string()
}
