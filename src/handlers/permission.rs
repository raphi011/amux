//! Permission handling service
//!
//! Centralizes logic for responding to permission requests from agents.

use crate::acp::PermissionOptionId;
use crate::session::{PendingPermission, Session, SessionState};

/// Service for handling permission requests
pub struct PermissionService;

impl PermissionService {
    /// Accept a permission request with the selected or default option
    ///
    /// Returns the request_id and option_id to send back to the agent,
    /// or None if there's no pending permission.
    pub fn accept(session: &mut Session) -> Option<(u64, PermissionOptionId)> {
        let perm = session.pending_permission.take()?;
        
        let option_id = perm.selected_option()
            .or_else(|| perm.allow_once_option())
            .map(|o| PermissionOptionId::from(o.option_id.clone()))?;
        
        session.state = SessionState::Prompting;
        Self::restore_saved_input(session);
        
        Some((perm.request_id, option_id))
    }

    /// Reject/cancel a permission request
    ///
    /// Returns the request_id to send cancellation to the agent,
    /// or None if there's no pending permission.
    pub fn reject(session: &mut Session) -> Option<u64> {
        let perm = session.pending_permission.take()?;
        
        session.state = SessionState::Idle;
        Self::restore_saved_input(session);
        
        Some(perm.request_id)
    }

    /// Navigate to next permission option
    pub fn select_next(session: &mut Session) {
        if let Some(perm) = &mut session.pending_permission {
            perm.select_next();
        }
    }

    /// Navigate to previous permission option
    pub fn select_prev(session: &mut Session) {
        if let Some(perm) = &mut session.pending_permission {
            perm.select_prev();
        }
    }

    /// Get the pending permission for display
    pub fn pending(session: &Session) -> Option<&PendingPermission> {
        session.pending_permission.as_ref()
    }

    /// Check if session has a pending permission
    pub fn has_pending(session: &Session) -> bool {
        session.pending_permission.is_some()
    }

    /// Restore any saved input after permission dialog closes
    fn restore_saved_input(session: &mut Session) -> Option<(String, usize)> {
        session.take_saved_input()
    }
}
