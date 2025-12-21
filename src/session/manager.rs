#![allow(dead_code)]

use super::state::{AgentType, Session};
use crate::picker::Picker;

pub struct SessionManager {
    sessions: Vec<Session>,
    selected: usize,
}

impl Picker for SessionManager {
    type Item = Session;

    fn items(&self) -> &[Session] {
        &self.sessions
    }

    fn selected_index(&self) -> usize {
        self.selected
    }

    fn set_selected_index(&mut self, index: usize) {
        self.selected = index;
    }
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

    pub fn selected_session(&self) -> Option<&Session> {
        self.selected_item()
    }

    pub fn selected_session_mut(&mut self) -> Option<&mut Session> {
        self.sessions.get_mut(self.selected)
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

    /// Find a session by its unique ID and return a mutable reference
    pub fn get_by_id_mut(&mut self, id: &str) -> Option<&mut Session> {
        self.sessions.iter_mut().find(|s| s.id == id)
    }

    /// Find a session by its unique ID
    pub fn get_by_id(&self, id: &str) -> Option<&Session> {
        self.sessions.iter().find(|s| s.id == id)
    }
}
