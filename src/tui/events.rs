use crate::agent::AgentRole;

#[derive(Debug, Clone)]
pub enum TuiEvent {
    AgentThinking(AgentRole),
    AgentThought(AgentRole, String),
    AgentResponse(AgentRole, String),
    ToolCall { server: String, tool: String },
    ToolResult { tool: String, result: String },
    ConsensusReached,
    RoundComplete,
    StatusUpdate(String),
    Error(String),
}

#[derive(Debug, Clone)]
pub enum UserAction {
    UserMessage(String),
    Stop,
    Continue,
}
