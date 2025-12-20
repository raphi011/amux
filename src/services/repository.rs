//! Repository service
//!
//! Handles folder scanning and git repository operations.
//! Extracted from main.rs to improve organization and testability.

use std::path::{Path, PathBuf};
use crate::app::{FolderEntry, WorktreeEntry, CleanupEntry};
use crate::git;

/// Service for repository and folder operations
pub struct RepositoryService;

impl RepositoryService {
    /// Get the current git branch for a directory
    pub async fn get_git_branch(cwd: &Path) -> String {
        match tokio::process::Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(cwd)
            .output()
            .await
        {
            Ok(output) if output.status.success() => {
                String::from_utf8_lossy(&output.stdout).trim().to_string()
            }
            _ => String::new(),
        }
    }

    /// Check if a directory is a git repository and get its branch
    pub async fn get_git_branch_if_repo(dir: &Path) -> Option<String> {
        let git_dir = dir.join(".git");
        if git_dir.exists() {
            let branch = Self::get_git_branch(dir).await;
            if !branch.is_empty() {
                return Some(branch);
            }
        }
        None
    }

    /// Scan a directory for subdirectories
    pub async fn scan_folder_entries(dir: &Path) -> Vec<FolderEntry> {
        let mut entries = vec![];

        // Add parent directory entry if not at root
        if dir.parent().is_some() {
            entries.push(FolderEntry {
                name: "..".to_string(),
                path: dir.parent().unwrap().to_path_buf(),
                git_branch: None,
                is_parent: true,
            });
        }

        // Read directory entries
        if let Ok(mut read_dir) = tokio::fs::read_dir(dir).await {
            let mut dirs = vec![];
            while let Ok(Some(entry)) = read_dir.next_entry().await {
                if let Ok(file_type) = entry.file_type().await {
                    if file_type.is_dir() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        // Skip hidden directories
                        if !name.starts_with('.') {
                            dirs.push((name, entry.path()));
                        }
                    }
                }
            }

            // Sort alphabetically
            dirs.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

            // Check for git repos
            for (name, path) in dirs {
                let git_branch = Self::get_git_branch_if_repo(&path).await;
                entries.push(FolderEntry {
                    name,
                    path,
                    git_branch,
                    is_parent: false,
                });
            }
        }

        entries
    }

    /// Scan the worktree directory for existing worktrees
    pub async fn scan_worktrees(worktree_dir: &Path, fetch_first: bool) -> Vec<WorktreeEntry> {
        let mut entries = vec![];

        // Always add "Create new worktree" option first
        entries.push(WorktreeEntry {
            name: "+ Create new worktree".to_string(),
            path: PathBuf::new(),
            is_create_new: true,
            is_clean: false,
            is_merged: false,
        });

        // Scan existing worktrees
        if let Ok(mut read_dir) = tokio::fs::read_dir(worktree_dir).await {
            let mut worktree_paths = vec![];
            while let Ok(Some(entry)) = read_dir.next_entry().await {
                if let Ok(file_type) = entry.file_type().await {
                    if file_type.is_dir() {
                        let path = entry.path();
                        // Only include if it looks like a git worktree
                        let git_path = path.join(".git");
                        if git_path.exists() {
                            worktree_paths.push(path);
                        }
                    }
                }
            }

            // Fetch from all unique parent repos first (for accurate merge status)
            if fetch_first {
                let mut fetched_repos = std::collections::HashSet::new();
                for path in &worktree_paths {
                    if let Some(parent_repo) = Self::get_worktree_parent_repo(path).await {
                        if fetched_repos.insert(parent_repo.clone()) {
                            crate::log::log(&format!("Fetching from origin in {}", parent_repo.display()));
                            if let Err(e) = git::fetch_origin(&parent_repo).await {
                                crate::log::log(&format!("Failed to fetch: {}", e));
                            }
                        }
                    }
                }
            }

            // Now get status for each worktree
            let mut worktrees = vec![];
            for path in worktree_paths {
                let name = path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                let is_clean = git::is_worktree_clean(&path).await.unwrap_or(false);
                let is_merged = Self::get_worktree_merged_status(&path).await;
                worktrees.push((name, path, is_clean, is_merged));
            }

            // Sort alphabetically
            worktrees.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

            for (name, path, is_clean, is_merged) in worktrees {
                entries.push(WorktreeEntry {
                    name,
                    path,
                    is_create_new: false,
                    is_clean,
                    is_merged,
                });
            }
        }

        entries
    }

    /// Get the parent repo path for a worktree
    pub async fn get_worktree_parent_repo(worktree_path: &Path) -> Option<PathBuf> {
        let gitdir_output = tokio::process::Command::new("git")
            .args(["rev-parse", "--git-common-dir"])
            .current_dir(worktree_path)
            .output()
            .await;

        match gitdir_output {
            Ok(output) if output.status.success() => {
                let dir = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let common_dir = PathBuf::from(dir);
                common_dir.parent().map(|p| p.to_path_buf())
            }
            _ => None,
        }
    }

    /// Get the merged status for a worktree
    pub async fn get_worktree_merged_status(worktree_path: &Path) -> bool {
        // Get current branch
        let branch_output = tokio::process::Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(worktree_path)
            .output()
            .await;

        let branch = match branch_output {
            Ok(output) if output.status.success() => {
                String::from_utf8_lossy(&output.stdout).trim().to_string()
            }
            _ => return false,
        };

        // Get the common git dir (parent repo)
        let gitdir_output = tokio::process::Command::new("git")
            .args(["rev-parse", "--git-common-dir"])
            .current_dir(worktree_path)
            .output()
            .await;

        let common_dir = match gitdir_output {
            Ok(output) if output.status.success() => {
                let dir = String::from_utf8_lossy(&output.stdout).trim().to_string();
                PathBuf::from(dir)
            }
            _ => return false,
        };

        // The parent repo is one level up from the .git directory
        let parent_repo = common_dir.parent().unwrap_or(&common_dir);

        git::is_branch_merged(parent_repo, &branch).await.unwrap_or(false)
    }

    /// Convert worktree entries to cleanup entries
    pub fn worktrees_to_cleanup_entries(worktree_entries: &[WorktreeEntry]) -> Vec<CleanupEntry> {
        worktree_entries.iter()
            .filter(|e| !e.is_create_new)
            .map(|e| {
                // Extract branch name from worktree name (format: repo-branch)
                let branch = e.name.split_once('-')
                    .map(|(_, b)| b.to_string());
                CleanupEntry {
                    path: e.path.clone(),
                    branch,
                    is_clean: e.is_clean,
                    is_merged: e.is_merged,
                    selected: false,
                }
            })
            .collect()
    }
}
