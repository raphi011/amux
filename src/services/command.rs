//! Command dispatcher service
//!
//! Centralizes sending commands to agents, reducing duplication
//! in event handling code.

use std::collections::HashMap;
use tokio::sync::mpsc;

use crate::acp::{ContentBlock, PermissionOptionId};
use crate::app::App;
use crate::session::{OutputType, SessionState};

/// Command to send to an agent
#[derive(Debug)]
pub enum AgentCommand {
    Prompt { session_id: String, text: String },
    PromptWithContent { session_id: String, content: Vec<ContentBlock> },
    PermissionResponse { request_id: u64, option_id: Option<PermissionOptionId> },
    AskUserResponse { request_id: u64, answer: String },
    CancelPrompt,
    SetModel { session_id: String, model_id: String },
}

/// Dispatcher for sending commands to agents
pub struct CommandDispatcher;

impl CommandDispatcher {
    /// Send a permission response to an agent
    pub async fn send_permission_response(
        commands: &HashMap<String, mpsc::Sender<AgentCommand>>,
        session_id: &str,
        request_id: u64,
        option_id: Option<PermissionOptionId>,
    ) -> bool {
        if let Some(cmd_tx) = commands.get(session_id) {
            cmd_tx.send(AgentCommand::PermissionResponse {
                request_id,
                option_id,
            }).await.is_ok()
        } else {
            false
        }
    }

    /// Send a question response to an agent
    pub async fn send_question_response(
        commands: &HashMap<String, mpsc::Sender<AgentCommand>>,
        session_id: &str,
        request_id: u64,
        answer: String,
    ) -> bool {
        if let Some(cmd_tx) = commands.get(session_id) {
            cmd_tx.send(AgentCommand::AskUserResponse {
                request_id,
                answer,
            }).await.is_ok()
        } else {
            false
        }
    }

    /// Send a prompt to the selected session
    pub async fn send_prompt(
        app: &mut App,
        commands: &HashMap<String, mpsc::Sender<AgentCommand>>,
        text: &str,
    ) {
        // Take attachments before borrowing session
        let attachments = std::mem::take(&mut app.attachments);
        let has_attachments = !attachments.is_empty();

        if let Some(session) = app.sessions.selected_session_mut() {
            // Add spacing before user message
            session.add_output(String::new(), OutputType::Text);

            // Show user input with attachment indicator
            if has_attachments {
                let attachment_names: Vec<_> = attachments.iter().map(|a| a.filename.as_str()).collect();
                session.add_output(
                    format!("> {} [+{}]", text, attachment_names.join(", ")),
                    OutputType::UserInput,
                );
            } else {
                session.add_output(format!("> {}", text), OutputType::UserInput);
            }
            session.state = SessionState::Prompting;

            let session_id = session.id.clone();

            // Build content blocks
            if has_attachments {
                let mut content: Vec<ContentBlock> = vec![];

                // Add text if present
                if !text.is_empty() {
                    content.push(ContentBlock::Text { text: text.to_string() });
                }

                // Add image attachments
                for attachment in attachments {
                    content.push(ContentBlock::Image {
                        mime_type: attachment.mime_type,
                        data: attachment.data,
                    });
                }

                // Send with content blocks
                if let Some(cmd_tx) = commands.get(&session_id) {
                    let _ = cmd_tx.send(AgentCommand::PromptWithContent {
                        session_id,
                        content,
                    }).await;
                }
            } else {
                // Send simple text prompt
                if let Some(cmd_tx) = commands.get(&session_id) {
                    let _ = cmd_tx.send(AgentCommand::Prompt {
                        session_id,
                        text: text.to_string(),
                    }).await;
                }
            }
        }
    }

    /// Cancel the current prompt for a session
    pub async fn cancel_prompt(
        commands: &HashMap<String, mpsc::Sender<AgentCommand>>,
        session_id: &str,
    ) -> bool {
        if let Some(cmd_tx) = commands.get(session_id) {
            cmd_tx.send(AgentCommand::CancelPrompt).await.is_ok()
        } else {
            false
        }
    }

    /// Set the model for a session
    pub async fn set_model(
        commands: &HashMap<String, mpsc::Sender<AgentCommand>>,
        session_id: &str,
        model_id: String,
    ) -> bool {
        if let Some(cmd_tx) = commands.get(session_id) {
            cmd_tx.send(AgentCommand::SetModel {
                session_id: session_id.to_string(),
                model_id,
            }).await.is_ok()
        } else {
            false
        }
    }
}
