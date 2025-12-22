use anyhow::{Result, bail};
use std::path::Path;

/// Get the git remote origin URL for a repository, normalized for grouping
pub async fn get_origin_url(repo_path: &Path) -> Option<String> {
    let output = tokio::process::Command::new("git")
        .args(["config", "--get", "remote.origin.url"])
        .current_dir(repo_path)
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if url.is_empty() {
        return None;
    }

    // Normalize the URL for grouping (extract repo identifier)
    Some(normalize_git_url(&url))
}

/// Normalize a git URL to a consistent format for grouping
/// Handles SSH (git@github.com:user/repo.git) and HTTPS (https://github.com/user/repo.git)
fn normalize_git_url(url: &str) -> String {
    let url = url.trim();

    // Remove .git suffix
    let url = url.strip_suffix(".git").unwrap_or(url);

    // Handle SSH format: git@github.com:user/repo -> github.com/user/repo
    if let Some(rest) = url.strip_prefix("git@") {
        return rest.replace(':', "/");
    }

    // Handle HTTPS format: https://github.com/user/repo -> github.com/user/repo
    if let Some(rest) = url.strip_prefix("https://") {
        return rest.to_string();
    }

    if let Some(rest) = url.strip_prefix("http://") {
        return rest.to_string();
    }

    // Return as-is if format is unknown
    url.to_string()
}

/// List all branches (local and remote) for a git repository
/// Check if a branch exists locally
pub async fn branch_exists(repo_path: &Path, branch_name: &str) -> Result<bool> {
    let output = tokio::process::Command::new("git")
        .args([
            "rev-parse",
            "--verify",
            &format!("refs/heads/{}", branch_name),
        ])
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
            .args([
                "rev-parse",
                "--verify",
                &format!("refs/remotes/{}/{}", remote, branch_name),
            ])
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

    let worktree_str = worktree_path
        .to_str()
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
    let worktree_str = worktree_path
        .to_str()
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

/// Statistics about git diff
#[derive(Debug, Clone, Default)]
pub struct DiffStats {
    pub files_changed: usize,
    pub insertions: usize,
    pub deletions: usize,
}

/// Get git diff statistics between current branch and base branch (usually origin/main)
pub async fn get_diff_stats(repo_path: &Path, current_branch: &str) -> Result<DiffStats> {
    // Get the default branch
    let base_branch = get_default_branch(repo_path).await?;

    // If we're on the base branch, show uncommitted changes (working directory + staged)
    if current_branch == base_branch {
        let output = tokio::process::Command::new("git")
            .args(["diff", "--shortstat", "HEAD"])
            .current_dir(repo_path)
            .output()
            .await?;

        if output.status.success() {
            return parse_diff_stats(&String::from_utf8_lossy(&output.stdout));
        } else {
            return Ok(DiffStats::default());
        }
    }

    // Run: git diff --shortstat origin/<base_branch>...<current_branch>
    let base_ref = format!("origin/{}", base_branch);
    let compare_ref = format!("{}...{}", base_ref, current_branch);

    let output = tokio::process::Command::new("git")
        .args(["diff", "--shortstat", &compare_ref])
        .current_dir(repo_path)
        .output()
        .await?;

    if !output.status.success() {
        // If the comparison fails (e.g., branch doesn't exist remotely yet),
        // try comparing against local base branch
        let local_compare = format!("{}...{}", base_branch, current_branch);
        let output = tokio::process::Command::new("git")
            .args(["diff", "--shortstat", &local_compare])
            .current_dir(repo_path)
            .output()
            .await?;

        if !output.status.success() {
            // If that also fails, return empty stats
            return Ok(DiffStats::default());
        }

        return parse_diff_stats(&String::from_utf8_lossy(&output.stdout));
    }

    parse_diff_stats(&String::from_utf8_lossy(&output.stdout))
}

/// Parse git diff --shortstat output
/// Example: " 3 files changed, 45 insertions(+), 12 deletions(-)"
fn parse_diff_stats(output: &str) -> Result<DiffStats> {
    let output = output.trim();

    // Empty output means no changes
    if output.is_empty() {
        return Ok(DiffStats::default());
    }

    let mut stats = DiffStats::default();

    // Parse files changed
    if let Some(files_part) = output.split(',').next()
        && let Some(num_str) = files_part.split_whitespace().next()
    {
        stats.files_changed = num_str.parse().unwrap_or(0);
    }

    // Parse insertions
    for part in output.split(',') {
        if part.contains("insertion")
            && let Some(num_str) = part.split_whitespace().next()
        {
            stats.insertions = num_str.parse().unwrap_or(0);
        }
        if part.contains("deletion")
            && let Some(num_str) = part.split_whitespace().next()
        {
            stats.deletions = num_str.parse().unwrap_or(0);
        }
    }

    Ok(stats)
}
