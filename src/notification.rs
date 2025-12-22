//! Native desktop notification support.
//!
//! Sends system notifications when sessions require user attention:
//! - Permission requests from agents
//! - Clarifying questions from agents
//! - Session becoming idle after completing work

use std::time::Instant;

use notify_rust::{Notification, Timeout};

/// Types of notifications that can be sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationType {
    /// Agent requires permission approval
    PermissionRequired,
    /// Agent is asking a clarifying question
    QuestionAsked,
    /// Session finished work and is now idle
    SessionIdle,
}

/// Configuration for notifications.
#[derive(Debug, Clone)]
pub struct NotificationConfig {
    /// Whether notifications are enabled
    pub enabled: bool,
    /// Seconds to wait after work completes before sending idle notification
    #[allow(dead_code)] // Reserved for future delayed notification feature
    pub idle_delay_secs: u64,
    /// Minimum seconds between same notification type (prevents spam)
    pub dedupe_interval_secs: u64,
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            idle_delay_secs: 5,
            dedupe_interval_secs: 30,
        }
    }
}

/// Manages sending desktop notifications with deduplication.
pub struct NotificationManager {
    config: NotificationConfig,
    last_notification: Option<(NotificationType, Instant)>,
}

impl NotificationManager {
    /// Create a new notification manager with the given configuration.
    pub fn new(config: NotificationConfig) -> Self {
        Self {
            config,
            last_notification: None,
        }
    }

    /// Send a notification if enabled and not a duplicate.
    ///
    /// Returns `true` if the notification was sent.
    pub fn send(&mut self, ntype: NotificationType, title: &str, body: &str) -> bool {
        if !self.config.enabled {
            return false;
        }

        if self.is_duplicate(ntype) {
            return false;
        }

        let result = Notification::new()
            .summary(title)
            .body(body)
            .timeout(Timeout::Milliseconds(5000))
            .show();

        if result.is_ok() {
            self.last_notification = Some((ntype, Instant::now()));
            true
        } else {
            false
        }
    }

    /// Send a permission required notification.
    pub fn notify_permission_required(&mut self, session_name: &str, tool_name: &str) {
        let title = "Permission Required";
        let body = format!("{}: {} needs approval", session_name, tool_name);
        self.send(NotificationType::PermissionRequired, title, &body);
    }

    /// Send a question notification.
    pub fn notify_question(&mut self, session_name: &str) {
        let title = "Question";
        let body = format!("{}: Agent has a question", session_name);
        self.send(NotificationType::QuestionAsked, title, &body);
    }

    /// Send a session idle notification.
    pub fn notify_idle(&mut self, session_name: &str) {
        let title = "Task Complete";
        let body = format!("{} is now idle", session_name);
        self.send(NotificationType::SessionIdle, title, &body);
    }

    /// Check if this notification type was recently sent.
    fn is_duplicate(&self, ntype: NotificationType) -> bool {
        self.last_notification
            .map(|(t, when)| {
                t == ntype && when.elapsed().as_secs() < self.config.dedupe_interval_secs
            })
            .unwrap_or(false)
    }

    /// Get the idle delay configuration.
    #[allow(dead_code)] // Reserved for future delayed notification feature
    pub fn idle_delay_secs(&self) -> u64 {
        self.config.idle_delay_secs
    }

    /// Check if notifications are enabled.
    #[allow(dead_code)] // May be useful for UI display
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disabled_notifications() {
        let config = NotificationConfig {
            enabled: false,
            ..Default::default()
        };
        let mut manager = NotificationManager::new(config);

        assert!(!manager.send(NotificationType::SessionIdle, "Test", "Body"));
    }

    #[test]
    fn test_default_config() {
        let config = NotificationConfig::default();
        assert!(config.enabled);
        assert_eq!(config.idle_delay_secs, 5);
        assert_eq!(config.dedupe_interval_secs, 30);
    }
}
