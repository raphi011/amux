#![allow(dead_code)]

use super::state::{AgentType, Session};

pub struct SessionManager {
    sessions: Vec<Session>,
    selected: usize,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: vec![],
            selected: 0,
        }
    }

    /// Create with mock data for UI development
    pub fn with_mock_data() -> Self {
        let sessions = vec![
            Session::mock("1", "agent-chat", AgentType::ClaudeCode, "main"),
            Session::mock("2", "hugo-paper", AgentType::ClaudeCode, "main"),
            Session::mock("3", "turingpi-k8s", AgentType::GeminiCli, "main"),
        ];

        Self {
            sessions,
            selected: 0,
        }
    }

    pub fn sessions(&self) -> &[Session] {
        &self.sessions
    }

    pub fn sessions_mut(&mut self) -> &mut Vec<Session> {
        &mut self.sessions
    }

    pub fn selected_index(&self) -> usize {
        self.selected
    }

    pub fn selected_session(&self) -> Option<&Session> {
        self.sessions.get(self.selected)
    }

    pub fn selected_session_mut(&mut self) -> Option<&mut Session> {
        self.sessions.get_mut(self.selected)
    }

    pub fn select_next(&mut self) {
        if !self.sessions.is_empty() {
            self.selected = (self.selected + 1) % self.sessions.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.sessions.is_empty() {
            self.selected = self.selected.checked_sub(1).unwrap_or(self.sessions.len() - 1);
        }
    }

    pub fn select_index(&mut self, index: usize) {
        if index < self.sessions.len() {
            self.selected = index;
        }
    }

    pub fn add_session(&mut self, session: Session) {
        self.sessions.push(session);
        // Select the new session
        self.selected = self.sessions.len() - 1;
    }

    pub fn remove_selected(&mut self) -> Option<Session> {
        if self.sessions.is_empty() {
            return None;
        }

        let removed = self.sessions.remove(self.selected);

        // Adjust selection
        if self.selected >= self.sessions.len() && !self.sessions.is_empty() {
            self.selected = self.sessions.len() - 1;
        }

        Some(removed)
    }

    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    pub fn len(&self) -> usize {
        self.sessions.len()
    }
}
