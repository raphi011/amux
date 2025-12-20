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

/// Information about a git worktree
#[derive(Debug, Clone)]
pub struct WorktreeInfo {
    pub path: std::path::PathBuf,
    pub branch: Option<String>,
    pub is_clean: bool,
    pub is_merged: bool,
}

/// List all worktrees for a repository
pub async fn list_worktrees(repo_path: &Path) -> Result<Vec<WorktreeInfo>> {
    let output = tokio::process::Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(repo_path)
        .output()
        .await?;

    if !output.status.success() {
        bail!("Failed to list worktrees");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut worktrees = Vec::new();
    let mut current_path: Option<std::path::PathBuf> = None;
    let mut current_branch: Option<String> = None;

    for line in stdout.lines() {
        if let Some(path_str) = line.strip_prefix("worktree ") {
            // Save previous worktree if any
            if let Some(path) = current_path.take() {
                worktrees.push((path, current_branch.take()));
            }
            current_path = Some(std::path::PathBuf::from(path_str));
        } else if let Some(branch_ref) = line.strip_prefix("branch ") {
            // Extract branch name from refs/heads/branch-name
            current_branch = branch_ref
                .strip_prefix("refs/heads/")
                .map(|s| s.to_string())
                .or_else(|| Some(branch_ref.to_string()));
        }
    }

    // Don't forget the last worktree
    if let Some(path) = current_path {
        worktrees.push((path, current_branch));
    }

    // Now check each worktree for clean/merged status
    let mut result = Vec::new();
    for (path, branch) in worktrees {
        // Skip the main worktree (the original repo)
        if path == repo_path {
            continue;
        }

        let is_clean = is_worktree_clean(&path).await.unwrap_or(false);
        let is_merged = if let Some(ref b) = branch {
            is_branch_merged(repo_path, b).await.unwrap_or(false)
        } else {
            false
        };

        result.push(WorktreeInfo {
            path,
            branch,
            is_clean,
            is_merged,
        });
    }

    Ok(result)
}

/// Check if a worktree has no uncommitted changes
pub async fn is_worktree_clean(worktree_path: &Path) -> Result<bool> {
    let output = tokio::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(worktree_path)
        .output()
        .await?;

    if !output.status.success() {
        bail!("Failed to check worktree status");
    }

    // If output is empty, the worktree is clean
    Ok(output.stdout.is_empty())
}

/// Check if a branch has been merged into origin's default branch
pub async fn is_branch_merged(repo_path: &Path, branch_name: &str) -> Result<bool> {
    // First, determine the default branch (main or master)
    let default_branch = get_default_branch(repo_path).await?;

    // Check if the branch tip is an ancestor of origin/<default_branch>
    // This is more reliable than `git branch --merged` as it checks against
    // the remote branch, which reflects merged PRs after a fetch
    let output = tokio::process::Command::new("git")
        .args([
            "merge-base",
            "--is-ancestor",
            branch_name,
            &format!("origin/{}", default_branch),
        ])
        .current_dir(repo_path)
        .output()
        .await?;

    // Exit code 0 = is ancestor (merged), 1 = not ancestor, other = error
    Ok(output.status.success())
}

/// Get the default branch (main or master)
pub async fn get_default_branch(repo_path: &Path) -> Result<String> {
    // Try to get the default branch from origin
    let output = tokio::process::Command::new("git")
        .args(["symbolic-ref", "refs/remotes/origin/HEAD"])
        .current_dir(repo_path)
        .output()
        .await?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Some(branch) = stdout.trim().strip_prefix("refs/remotes/origin/") {
            return Ok(branch.to_string());
        }
    }

    // Fallback: check if main or master exists
    for branch in &["main", "master"] {
        let output = tokio::process::Command::new("git")
            .args(["rev-parse", "--verify", &format!("refs/heads/{}", branch)])
            .current_dir(repo_path)
            .output()
            .await?;

        if output.status.success() {
            return Ok(branch.to_string());
        }
    }

    bail!("Could not determine default branch")
}

/// Remove a git worktree
pub async fn remove_worktree(repo_path: &Path, worktree_path: &Path, force: bool) -> Result<()> {
    let worktree_str = worktree_path.to_str()
        .ok_or_else(|| anyhow::anyhow!("Invalid worktree path"))?;

    let mut args = vec!["worktree", "remove", worktree_str];
    if force {
        args.push("--force");
    }

    let output = tokio::process::Command::new("git")
        .args(&args)
        .current_dir(repo_path)
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Failed to remove worktree: {}", stderr.trim());
    }

    Ok(())
}

/// Fetch from origin with prune to update remote refs
pub async fn fetch_origin(repo_path: &Path) -> Result<()> {
    let output = tokio::process::Command::new("git")
        .args(["fetch", "--prune", "origin"])
        .current_dir(repo_path)
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Failed to fetch from origin: {}", stderr.trim());
    }

    Ok(())
}

/// Delete the branch associated with a worktree
pub async fn delete_branch(repo_path: &Path, branch_name: &str, force: bool) -> Result<()> {
    let flag = if force { "-D" } else { "-d" };

    let output = tokio::process::Command::new("git")
        .args(["branch", flag, branch_name])
        .current_dir(repo_path)
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Failed to delete branch: {}", stderr.trim());
    }

    Ok(())
}
