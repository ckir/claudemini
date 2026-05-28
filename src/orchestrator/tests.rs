#[cfg(test)]
mod tests {
    use crate::orchestrator::DialogueSession;
    use crate::agent::AgentRole;

    #[test]
    fn test_dialogue_session_add_message() {
        let mut session = DialogueSession::new();
        session.add_message(AgentRole::claude(), "Hello".to_string(), false);
        session.add_message(AgentRole::gemini(), "Secret".to_string(), true);

        assert_eq!(session.history.len(), 2);
        assert_eq!(session.history[0].content, "Hello");
        assert_eq!(session.history[1].is_private, true);
    }

    #[test]
    fn test_get_public_history() {
        let mut session = DialogueSession::new();
        session.add_message(AgentRole::claude(), "Public".to_string(), false);
        session.add_message(AgentRole::gemini(), "Private".to_string(), true);

        let public = session.get_public_history();
        assert_eq!(public.len(), 1);
        assert_eq!(public[0].content, "Public");
    }

    #[test]
    fn test_format_history_for_prompt() {
        let mut session = DialogueSession::new();
        session.add_message(AgentRole::claude(), "Hello".to_string(), false);
        session.add_message(AgentRole::gemini(), "World".to_string(), true);

        let formatted = session.format_history_for_prompt();
        assert!(formatted.contains("Claude : Hello"));
        assert!(formatted.contains("Gemini (Private Scratchpad) : World"));
    }
}
