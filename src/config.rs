//! Configuration file support for amux.
//!
//! Configuration is loaded from `~/.config/amux/config.toml` with the following precedence:
//! 1. CLI arguments (highest priority)
//! 2. Environment variables
//! 3. Configuration file
//! 4. Default values (lowest priority)
//!
//! # Example Configuration
//!
//! ```toml
//! # ~/.config/amux/config.toml
//! worktree_dir = "~/.amux/worktrees"
//! default_agent = "ClaudeCode"
//! theme = "dark"
//!
//! # MCP servers available to all sessions
//! [[mcp_servers]]
//! name = "filesystem"
//! command = "npx"
//! args = ["-y", "@modelcontextprotocol/server-filesystem", "/path/to/dir"]
//!
//! [[mcp_servers]]
//! name = "github"
//! command = "npx"
//! args = ["-y", "@modelcontextprotocol/server-github"]
//! env = { GITHUB_TOKEN = "xxx" }
//! ```

#![allow(dead_code)]

use std::collections::HashMap;
use std::path::PathBuf;

use serde::Deserialize;

use crate::session::AgentType;

/// Main configuration structure.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    /// Directory for git worktrees
    pub worktree_dir: Option<PathBuf>,

    /// Default agent to use for new sessions
    pub default_agent: Option<AgentType>,

    /// Theme name to use (reserved for future use)
    pub theme: Option<String>,

    /// Keybinding customization (reserved for future use)
    #[serde(default)]
    pub keybindings: KeyBindings,

    /// MCP servers to make available to agent sessions
    #[serde(default)]
    pub mcp_servers: Vec<McpServerConfig>,
}

/// MCP server configuration
#[derive(Debug, Clone, Deserialize)]
pub struct McpServerConfig {
    /// Unique name for this MCP server
    pub name: String,

    /// Command to run (for stdio transport)
    pub command: String,

    /// Arguments to pass to the command
    #[serde(default)]
    pub args: Vec<String>,

    /// Environment variables (name -> value)
    #[serde(default)]
    pub env: HashMap<String, String>,
}

/// Custom keybinding configuration (reserved for future use).
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct KeyBindings {
    // Placeholder for future keybinding customization
    // e.g., quit: Option<String>,
    // e.g., next_session: Option<String>,
}

impl Config {
    /// Load configuration from the default config file path.
    ///
    /// Returns default configuration if file doesn't exist or can't be parsed.
    pub fn load() -> Self {
        let config_path = Self::config_path();

        if !config_path.exists() {
            return Self::default();
        }

        match std::fs::read_to_string(&config_path) {
            Ok(contents) => match toml::from_str(&contents) {
                Ok(config) => config,
                Err(e) => {
                    eprintln!("Warning: Failed to parse config file: {}", e);
                    Self::default()
                }
            },
            Err(e) => {
                eprintln!("Warning: Failed to read config file: {}", e);
                Self::default()
            }
        }
    }

    /// Get the default configuration file path.
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("amux")
            .join("config.toml")
    }

    /// Get the configuration directory path.
    pub fn config_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("amux")
    }

    /// Merge with CLI overrides.
    ///
    /// CLI arguments take precedence over config file values.
    pub fn with_overrides(
        mut self,
        worktree_dir: Option<PathBuf>,
        default_agent: Option<AgentType>,
    ) -> Self {
        if worktree_dir.is_some() {
            self.worktree_dir = worktree_dir;
        }
        if default_agent.is_some() {
            self.default_agent = default_agent;
        }
        self
    }

    /// Get the worktree directory, falling back to environment variable or default.
    pub fn worktree_dir(&self) -> PathBuf {
        self.worktree_dir
            .clone()
            .or_else(|| std::env::var("AMUX_WORKTREE_DIR").ok().map(PathBuf::from))
            .unwrap_or_else(|| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".amux/worktrees")
            })
    }

    /// Get the default agent type.
    pub fn default_agent(&self) -> AgentType {
        self.default_agent.unwrap_or(AgentType::ClaudeCode)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(config.worktree_dir.is_none());
        assert!(config.default_agent.is_none());
        assert!(config.theme.is_none());
    }

    #[test]
    fn test_parse_config() {
        let toml = r#"
            worktree_dir = "/tmp/worktrees"
            default_agent = "ClaudeCode"
            theme = "dark"
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.worktree_dir, Some(PathBuf::from("/tmp/worktrees")));
        assert_eq!(config.default_agent, Some(AgentType::ClaudeCode));
        assert_eq!(config.theme, Some("dark".to_string()));
    }
}
