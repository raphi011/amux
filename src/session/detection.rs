//! Agent detection and availability checking
//!
//! This module provides functionality to detect which agents are available
//! on the system by checking their preconditions (commands installed, etc.)

use std::process::Command;

use super::AgentType;

/// A precondition that must be met for an agent to be available
#[derive(Debug, Clone)]
pub struct Precondition {
    /// Human-readable description of the precondition
    pub description: &'static str,
    /// Whether this precondition is satisfied
    pub satisfied: bool,
}

/// Information about an agent's availability
#[derive(Debug, Clone)]
pub struct AgentAvailability {
    pub agent_type: AgentType,
    pub preconditions: Vec<Precondition>,
}

impl AgentAvailability {
    /// Check if all preconditions are satisfied
    pub fn is_available(&self) -> bool {
        self.preconditions.iter().all(|p| p.satisfied)
    }

    /// Count of satisfied preconditions
    #[allow(dead_code)]
    pub fn satisfied_count(&self) -> usize {
        self.preconditions.iter().filter(|p| p.satisfied).count()
    }

    /// Total number of preconditions
    #[allow(dead_code)]
    pub fn total_count(&self) -> usize {
        self.preconditions.len()
    }
}

/// Check if a command exists in PATH
fn command_exists(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// Check if an npm package is globally installed
fn npm_global_package_exists(package: &str) -> bool {
    Command::new("npm")
        .args(["list", "-g", package, "--depth=0"])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// Check preconditions for Claude Code
fn check_claude_code() -> Vec<Precondition> {
    // Claude Code can be installed either via npm or as the claude-code-acp command
    let acp_command = command_exists("claude-code-acp");
    let npx_available = command_exists("npx");

    vec![Precondition {
        description: "claude-code-acp command",
        satisfied: acp_command || npx_available,
    }]
}

/// Check preconditions for Gemini CLI
fn check_gemini_cli() -> Vec<Precondition> {
    let gemini_command = command_exists("gemini");
    let npm_global = npm_global_package_exists("@google/gemini-cli");

    vec![Precondition {
        description: "gemini command installed",
        satisfied: gemini_command || npm_global,
    }]
}

/// Check availability for a specific agent type
pub fn check_agent(agent_type: AgentType) -> AgentAvailability {
    let preconditions = match agent_type {
        AgentType::ClaudeCode => check_claude_code(),
        AgentType::GeminiCli => check_gemini_cli(),
    };

    AgentAvailability {
        agent_type,
        preconditions,
    }
}

/// Check availability for all supported agents
pub fn check_all_agents() -> Vec<AgentAvailability> {
    vec![
        check_agent(AgentType::ClaudeCode),
        check_agent(AgentType::GeminiCli),
    ]
}

/// Get all agent types with their availability status
#[allow(dead_code)]
pub fn get_agents_with_status() -> Vec<(AgentType, bool)> {
    check_all_agents()
        .into_iter()
        .map(|a| (a.agent_type, a.is_available()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_all_agents_returns_both() {
        let agents = check_all_agents();
        assert_eq!(agents.len(), 2);
        assert!(agents.iter().any(|a| a.agent_type == AgentType::ClaudeCode));
        assert!(agents.iter().any(|a| a.agent_type == AgentType::GeminiCli));
    }

    #[test]
    fn test_availability_calculation() {
        let available = AgentAvailability {
            agent_type: AgentType::ClaudeCode,
            preconditions: vec![
                Precondition {
                    description: "test1",
                    satisfied: true,
                },
                Precondition {
                    description: "test2",
                    satisfied: true,
                },
            ],
        };
        assert!(available.is_available());
        assert_eq!(available.satisfied_count(), 2);

        let unavailable = AgentAvailability {
            agent_type: AgentType::GeminiCli,
            preconditions: vec![
                Precondition {
                    description: "test1",
                    satisfied: true,
                },
                Precondition {
                    description: "test2",
                    satisfied: false,
                },
            ],
        };
        assert!(!unavailable.is_available());
        assert_eq!(unavailable.satisfied_count(), 1);
    }
}
