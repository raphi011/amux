//! Question handling service
//!
//! Centralizes logic for responding to user questions from agents.

use crate::session::{PendingQuestion, Session, SessionState};

/// Service for handling user question requests
pub struct QuestionService;

impl QuestionService {
    /// Submit an answer to a pending question
    ///
    /// Returns the request_id and answer string to send back to the agent,
    /// or None if there's no pending question.
    pub fn submit(session: &mut Session) -> Option<(u64, String)> {
        let question = session.pending_question.take()?;
        
        let answer = question.get_answer();
        let request_id = question.request_id;
        
        session.state = SessionState::Prompting;
        Self::restore_saved_input(session);
        
        Some((request_id, answer))
    }

    /// Cancel/dismiss a question with empty response
    ///
    /// Returns the request_id to send empty answer to the agent,
    /// or None if there's no pending question.
    pub fn cancel(session: &mut Session) -> Option<u64> {
        let question = session.pending_question.take()?;
        
        session.state = SessionState::Idle;
        Self::restore_saved_input(session);
        
        Some(question.request_id)
    }

    /// Type a character into the question input
    pub fn input_char(session: &mut Session, c: char) {
        if let Some(question) = &mut session.pending_question {
            question.input_char(c);
        }
    }

    /// Delete character before cursor
    pub fn input_backspace(session: &mut Session) {
        if let Some(question) = &mut session.pending_question {
            question.input_backspace();
        }
    }

    /// Delete character at cursor
    pub fn input_delete(session: &mut Session) {
        if let Some(question) = &mut session.pending_question {
            question.input_delete();
        }
    }

    /// Move cursor left
    pub fn input_left(session: &mut Session) {
        if let Some(question) = &mut session.pending_question {
            question.input_left();
        }
    }

    /// Move cursor right
    pub fn input_right(session: &mut Session) {
        if let Some(question) = &mut session.pending_question {
            question.input_right();
        }
    }

    /// Move cursor to start
    pub fn input_home(session: &mut Session) {
        if let Some(question) = &mut session.pending_question {
            question.input_home();
        }
    }

    /// Move cursor to end
    pub fn input_end(session: &mut Session) {
        if let Some(question) = &mut session.pending_question {
            question.input_end();
        }
    }

    /// Navigate to next option (for non-free-text questions)
    pub fn select_next(session: &mut Session) {
        if let Some(question) = &mut session.pending_question {
            if !question.is_free_text() {
                question.select_next();
            }
        }
    }

    /// Navigate to previous option (for non-free-text questions)
    pub fn select_prev(session: &mut Session) {
        if let Some(question) = &mut session.pending_question {
            if !question.is_free_text() {
                question.select_prev();
            }
        }
    }

    /// Get the pending question for display
    pub fn pending(session: &Session) -> Option<&PendingQuestion> {
        session.pending_question.as_ref()
    }

    /// Check if session has a pending question
    pub fn has_pending(session: &Session) -> bool {
        session.pending_question.is_some()
    }

    /// Check if the pending question is free-text (no options)
    pub fn is_free_text(session: &Session) -> bool {
        session.pending_question
            .as_ref()
            .map(|q| q.is_free_text())
            .unwrap_or(true)
    }

    /// Restore any saved input after question dialog closes
    fn restore_saved_input(session: &mut Session) -> Option<(String, usize)> {
        session.take_saved_input()
    }
}
