//! Domain error types for amux
//!
//! Provides structured error types for different domains:
//! - `GitError` for git operations
//! - `AgentError` for agent communication
//! - `AmuxError` as the top-level error type

use std::path::PathBuf;
use thiserror::Error;

/// Top-level error type for amux
#[derive(Debug, Error)]
pub enum AmuxError {
    #[error("Git error: {0}")]
    Git(#[from] GitError),

    #[error("Agent error: {0}")]
    Agent(#[from] AgentError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Terminal error: {0}")]
    Terminal(String),

    #[error("{0}")]
    Other(String),
}

/// Errors related to git operations
#[derive(Debug, Error)]
pub enum GitError {
    #[error("Branch '{0}' not found")]
    BranchNotFound(String),

    #[error("Worktree already exists at {0}")]
    WorktreeExists(PathBuf),

    #[error("Failed to create worktree: {0}")]
    WorktreeCreationFailed(String),

    #[error("Failed to remove worktree: {0}")]
    WorktreeRemovalFailed(String),

    #[error("Failed to delete branch '{0}': {1}")]
    BranchDeletionFailed(String, String),

    #[error("Failed to fetch from origin: {0}")]
    FetchFailed(String),

    #[error("Could not determine default branch")]
    NoDefaultBranch,

    #[error("Not a git repository: {0}")]
    NotARepository(PathBuf),

    #[error("Git command failed: {0}")]
    CommandFailed(String),
}

/// Errors related to agent communication
#[derive(Debug, Error)]
pub enum AgentError {
    #[error("Failed to spawn agent: {0}")]
    SpawnFailed(String),

    #[error("Agent connection lost")]
    Disconnected,

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Request timed out after {0}ms")]
    Timeout(u64),

    #[error("Agent initialization failed: {0}")]
    InitFailed(String),

    #[error("Session creation failed: {0}")]
    SessionFailed(String),

    #[error("Prompt failed: {0}")]
    PromptFailed(String),

    #[error("Permission response failed: {0}")]
    PermissionFailed(String),

    #[error("Invalid response: {0}")]
    InvalidResponse(String),
}

/// Result type alias for AmuxError
pub type Result<T> = std::result::Result<T, AmuxError>;

/// Result type alias for GitError
pub type GitResult<T> = std::result::Result<T, GitError>;

/// Result type alias for AgentError
pub type AgentResult<T> = std::result::Result<T, AgentError>;

// Conversion from anyhow::Error for backward compatibility
impl From<anyhow::Error> for AmuxError {
    fn from(err: anyhow::Error) -> Self {
        AmuxError::Other(err.to_string())
    }
}

impl From<String> for AmuxError {
    fn from(msg: String) -> Self {
        AmuxError::Other(msg)
    }
}

impl From<&str> for AmuxError {
    fn from(msg: &str) -> Self {
        AmuxError::Other(msg.to_string())
    }
}
