mod client;
pub mod protocol;

pub use client::{AgentConnection, AgentEvent};
pub use protocol::{
    AskUserOption, AskUserResponse, ContentBlock, ModelInfo, PermissionKind, PermissionOptionId,
    PermissionOptionInfo, PlanEntry, PlanStatus, SessionUpdate,
};
