mod client;
pub mod protocol;

pub use client::{AgentConnection, AgentEvent};
pub use protocol::{
    AgentCommand, AskUserOption, AskUserResponse, ContentBlock, McpServer, ModelInfo,
    PermissionKind, PermissionOptionId, PermissionOptionInfo, PlanEntry, PlanStatus, SessionUpdate,
};
