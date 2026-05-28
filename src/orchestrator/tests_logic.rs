#[cfg(test)]
mod tests {
    use crate::orchestrator::Orchestrator;
    use crate::agent::AgentRole;

    #[test]
    fn test_parse_tool_call() {
        let orchestrator = Orchestrator::new();
        let content = r#"I will help you. <tool_call server="filesystem" tool="read_file" args='{"path": "test.txt"}' />"#;
        let tool_call = orchestrator.parse_tool_call(content).unwrap();
        
        assert_eq!(tool_call.server, "filesystem");
        assert_eq!(tool_call.tool, "read_file");
        assert_eq!(tool_call.args["path"], "test.txt");
    }

    #[test]
    fn test_parse_tool_call_invalid() {
        let orchestrator = Orchestrator::new();
        let content = "No tool call here";
        assert!(orchestrator.parse_tool_call(content).is_none());
    }

    #[test]
    fn test_check_consensus() {
        let mut orchestrator = Orchestrator::new();
        
        // No consensus
        orchestrator.session.add_message(AgentRole::claude(), "Hello".to_string(), false);
        assert!(!orchestrator.check_consensus());

        // Consensus reached
        orchestrator.session.add_message(AgentRole::gemini(), "I agree. <consensus>true</consensus>".to_string(), false);
        assert!(orchestrator.check_consensus());
    }
}
