#[cfg(test)]
mod tests {
    use crate::agent::{AgentRole, Agent, AppConfig};

    #[test]
    fn test_agent_role_creation() {
        assert_eq!(AgentRole::claude().0, "Claude");
        assert_eq!(AgentRole::gemini().0, "Gemini");
        assert_eq!(AgentRole::user().0, "User");
        assert_eq!(AgentRole("Custom".to_string()).0, "Custom");
    }

    #[test]
    fn test_agent_new() {
        let agent = Agent::new(AgentRole::claude(), "Claude".to_string(), Some("Persona".to_string()));
        assert_eq!(agent.name, "Claude");
        assert_eq!(agent.persona, "Persona");
        assert_eq!(agent.role, AgentRole::claude());

        let default_agent = Agent::new(AgentRole::gemini(), "Gemini".to_string(), None);
        assert_eq!(default_agent.persona, "Expert AI Assistant");
    }

    #[test]
    fn test_app_config_default() {
        let config = AppConfig::default_config();
        assert_eq!(config.agents.len(), 2);
        assert_eq!(config.agents[0].name, "Claude");
        assert_eq!(config.agents[1].name, "Gemini");
    }
}
