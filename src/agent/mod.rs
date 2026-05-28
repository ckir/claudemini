use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct AgentRole(pub String);

impl AgentRole {
    pub fn claude() -> Self { Self("Claude".to_string()) }
    pub fn gemini() -> Self { Self("Gemini".to_string()) }
    pub fn user() -> Self { Self("User".to_string()) }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: AgentRole,
    pub content: String,
    pub is_private: bool,
}

pub struct Agent {
    pub role: AgentRole,
    pub name: String,
    pub persona: String,
}

impl Agent {
    pub fn new(role: AgentRole, name: String, persona: Option<String>) -> Self {
        let persona = persona.unwrap_or_else(|| "Expert AI Assistant".to_string());
        Self { role, name, persona }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub name: String,
    pub persona: String,
    pub mcp_command: String,
    pub mcp_args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub agents: Vec<AgentConfig>,
}

#[cfg(test)]
mod tests;

impl AppConfig {
    pub fn default_config() -> Self {
        Self {
            agents: vec![
                AgentConfig {
                    name: "Claude".to_string(),
                    persona: "Expert Rust Developer".to_string(),
                    mcp_command: "claude_mcp".to_string(),
                    mcp_args: vec![],
                },
                AgentConfig {
                    name: "Gemini".to_string(),
                    persona: "Strategic Solution Architect".to_string(),
                    mcp_command: "gemini_mcp".to_string(),
                    mcp_args: vec![],
                },
            ],
        }
    }
}
