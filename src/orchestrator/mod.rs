use crate::agent::{Agent, AgentRole, Message, AppConfig};
use crate::mcp::McpClientWrapper;
use crate::tui::{TuiEvent, UserAction};
use anyhow::Result;
use tokio::sync::mpsc;
use tracing::{info, debug, trace, error, warn, instrument};

pub struct DialogueSession {
    pub history: Vec<Message>,
}

impl DialogueSession {
    pub fn new() -> Self {
        Self { history: Vec::new() }
    }

    pub fn add_message(&mut self, role: AgentRole, content: String, is_private: bool) {
        trace!(role = %role.0, is_private, "Adding message to session");
        self.history.push(Message {
            role,
            content,
            is_private,
        });
    }
// ... rest of DialogueSession ...

    pub fn get_public_history(&self) -> Vec<Message> {
        self.history
            .iter()
            .filter(|m| !m.is_private)
            .cloned()
            .collect()
    }
    
    pub fn format_history_for_prompt(&self) -> String {
        let mut output = String::new();
        for msg in &self.history {
            let role_str = &msg.role.0;
            let private_str = if msg.is_private { " (Private Scratchpad)" } else { "" };
            output.push_str(&format!("{}{} : {}\n", role_str, private_str, msg.content));
        }
        output
    }
}

pub struct Orchestrator {
    pub session: DialogueSession,
    pub agents: Vec<Agent>,
    pub mcp: Option<McpClientWrapper>,
    pub events_tx: Option<mpsc::UnboundedSender<TuiEvent>>,
    pub user_rx: Option<mpsc::Receiver<UserAction>>,
    pub debug_mode: bool,
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_logic;

impl Orchestrator {
    pub fn new() -> Self {
        Self {
            session: DialogueSession::new(),
            agents: vec![],
            mcp: None,
            events_tx: None,
            user_rx: None,
            debug_mode: false,
        }
    }

    pub fn set_channels(&mut self, tx: mpsc::UnboundedSender<TuiEvent>, rx: mpsc::Receiver<UserAction>) {
        self.events_tx = Some(tx);
        self.user_rx = Some(rx);
    }

    fn emit(&self, event: TuiEvent) {
        if let Some(tx) = &self.events_tx {
            let _ = tx.send(event);
        }
    }

    #[instrument(skip(self, config))]
    pub async fn init(&mut self, config: AppConfig) -> Result<()> {
        info!(debug_mode = self.debug_mode, "Initializing Orchestrator");
        self.emit(TuiEvent::StatusUpdate("Initializing MCP agents...".to_string()));

        // Initialize agents list from config
        self.agents = config.agents.iter().map(|a| {
            debug!(agent_name = %a.name, "Configuring agent");
            Agent::new(AgentRole(a.name.clone()), a.name.clone(), Some(a.persona.clone()))
        }).collect();

        self.mcp = Some(McpClientWrapper::new(config, self.debug_mode).await?);
        info!("Orchestrator initialized successfully");
        Ok(())
    }


    /// Run the collaboration loop for a given user prompt.
    #[instrument(skip(self, user_prompt), fields(prompt = %user_prompt, request_id = %uuid::Uuid::new_v4()))]
    pub async fn collaborate(&mut self, user_prompt: &str) -> Result<()> {
        info!("Starting collaboration loop");
        if self.mcp.is_none() {
            error!("Orchestrator not initialized");
            return Err(anyhow::anyhow!("Orchestrator not initialized. Call init() first."));
        }

        self.emit(TuiEvent::StatusUpdate(format!("Team collaboration started for: \"{}\"", user_prompt)));
        
        let mut turn = 0;
        let max_turns = 6; 

        while turn < max_turns {
            info!(turn, "Starting turn");
            // 1. Parallel Thinking Phase
            self.emit(TuiEvent::StatusUpdate("Team Thinking Phase...".to_string()));
            let mut thought_futures = Vec::new();
            for agent in &self.agents {
                debug!(agent = %agent.name, "Requesting thought");
                self.emit(TuiEvent::AgentThinking(agent.role.clone()));
                thought_futures.push(self.generate_thought(agent, user_prompt));
            }
            
            let thoughts = futures::future::join_all(thought_futures).await;
            for (i, thought_result) in thoughts.into_iter().enumerate() {
                let thought = thought_result?;
                trace!(agent = %self.agents[i].name, thought_len = thought.len(), "Received thought");
                self.session.add_message(self.agents[i].role.clone(), thought.clone(), true);
                self.emit(TuiEvent::AgentThought(self.agents[i].role.clone(), thought));
            }
            self.emit(TuiEvent::StatusUpdate("Team thoughts synchronized.".to_string()));

            // 2. Sequential Response Phase
            for agent in &self.agents {
                info!(agent = %agent.name, "Agent responding");
                self.emit(TuiEvent::StatusUpdate(format!("Waiting for {}'s response...", agent.name)));
                
                let mut tool_loop_count = 0;
                let max_tool_loops = 3;

                loop {
                    let public_response = self.generate_response(agent, user_prompt).await?;
                    trace!(agent = %agent.name, response_len = public_response.len(), "Received public response");
                    self.session.add_message(agent.role.clone(), public_response.clone(), false);
                    self.emit(TuiEvent::AgentResponse(agent.role.clone(), public_response.clone()));

                    // 3. Tool Call Detection
                    if let Some(tool_call) = self.parse_tool_call(&public_response) {
                        debug!(server = %tool_call.server, tool = %tool_call.tool, "Detected tool call");
                        if tool_loop_count < max_tool_loops {
                            self.emit(TuiEvent::ToolCall { server: tool_call.server.clone(), tool: tool_call.tool.clone() });
                            let result = self.execute_tool_call(&tool_call).await?;
                            trace!(tool = %tool_call.tool, result_len = result.len(), "Tool execution result");
                            self.emit(TuiEvent::ToolResult { tool: tool_call.tool.clone(), result: result.clone() });
                            
                            self.session.add_message(AgentRole::user(), format!("Tool Result ({}): {}", tool_call.tool, result), false);
                            tool_loop_count += 1;
                            continue; // Agent gets another go to process tool result
                        } else {
                            warn!(agent = %agent.name, "Max tool loops reached");
                        }
                    }

                    // 4. Check for Consensus
                    if self.check_consensus() {
                        info!("Consensus reached");
                        self.emit(TuiEvent::ConsensusReached);
                        // Save result to memory
                        if let Some(mcp) = &self.mcp {
                            debug!("Saving result to memory");
                            let history = self.session.format_history_for_prompt();
                            let memory_content = format!("Task: {}\nResult: {}\nHistory: {}", user_prompt, public_response, history);
                            let _ = mcp.memory_save(&memory_content).await;
                        }
                        return Ok(());
                    }
                    break;
                }
            }

            // Human Intervention Phase
            self.emit(TuiEvent::RoundComplete);
            if self.user_rx.is_some() {
                info!("Waiting for user intervention");
                self.emit(TuiEvent::StatusUpdate("Waiting for user intervention...".to_string()));
                if let Some(rx) = &mut self.user_rx {
                    if let Some(action) = rx.recv().await {
                        match action {
                            UserAction::UserMessage(msg) => {
                                info!(msg_len = msg.len(), "Received user message");
                                self.session.add_message(AgentRole::user(), msg, false);
                            }
                            UserAction::Stop => {
                                info!("Collaboration stopped by user");
                                self.emit(TuiEvent::StatusUpdate("Collaboration stopped by user.".to_string()));
                                return Ok(());
                            }
                            UserAction::Continue => {
                                info!("User chose to continue");
                            }
                        }
                    }
                }
            }

            turn += 1;
        }

        warn!("Maximum turns reached without explicit consensus");
        self.emit(TuiEvent::StatusUpdate("Maximum turns reached without explicit consensus.".to_string()));
        Ok(())
    }


    #[instrument(skip(self, agent, user_prompt), fields(agent = %agent.name))]
    async fn generate_thought(&self, agent: &Agent, user_prompt: &str) -> Result<String> {
        trace!("Generating thought");
        let history = self.session.format_history_for_prompt();
        let mcp = self.mcp.as_ref().ok_or_else(|| anyhow::anyhow!("MCP not initialized"))?;
        
        // Recall relevant context
        let recalled_memory = mcp.memory_recall(user_prompt).await.unwrap_or_default();
        let memory_context = if recalled_memory.is_empty() {
            "".to_string()
        } else {
            format!("\nRelevant Past Memories:\n{}\n", recalled_memory)
        };

        let prompt = format!(
            "System: You are in a collaboration with another agent. \
            Your Role: {}\n\
            Current User Goal: {}\n\
            {}\n\
            Current Dialogue History:\n{}\n\n\
            Task: Write your PRIVATE thoughts about the current state of the conversation and what you should suggest next. \
            Be critical and analytical. Only output your scratchpad thoughts.",
            agent.persona, user_prompt, memory_context, history
        );

        self.call_mcp(agent, &prompt).await
    }

    #[instrument(skip(self, agent, user_prompt), fields(agent = %agent.name))]
    async fn generate_response(&self, agent: &Agent, user_prompt: &str) -> Result<String> {
        trace!("Generating response");
        let history = self.session.format_history_for_prompt();
        let mcp = self.mcp.as_ref().ok_or_else(|| anyhow::anyhow!("MCP not initialized"))?;

        // Recall relevant context (could be different if we wanted to refine)
        let recalled_memory = mcp.memory_recall(user_prompt).await.unwrap_or_default();
        let memory_context = if recalled_memory.is_empty() {
            "".to_string()
        } else {
            format!("\nRelevant Past Memories:\n{}\n", recalled_memory)
        };

        let prompt = format!(
            "System: You are in a collaboration with another agent. \
            Your Role: {}\n\
            Current User Goal: {}\n\
            {}\n\
            Available Tools:\n\
            - filesystem: list_directory(path), read_file(path), write_file(path, content), search_files(root, pattern)\n\
            - search: brave_search(query)\n\n\
            To use a tool, include this tag in your response: <tool_call server=\"server_name\" tool=\"tool_name\" args='{{ \"key\": \"value\" }}' />\n\n\
            Current Dialogue History (including your private thoughts):\n{}\n\n\
            Task: Now, provide your PUBLIC response to the team. \
            If you believe the team has reached a final solution or agreement that satisfies the User Goal, \
            include the tag <consensus>true</consensus> in your response. \
            Otherwise, provide a better technical path or critique. Be concise.",
            agent.persona, user_prompt, memory_context, history
        );

        self.call_mcp(agent, &prompt).await
    }

    #[instrument(skip(self, agent, prompt), fields(agent = %agent.name))]
    async fn call_mcp(&self, agent: &Agent, prompt: &str) -> Result<String> {
        trace!(prompt_len = prompt.len(), "Calling MCP agent");
        let mcp = self.mcp.as_ref().ok_or_else(|| anyhow::anyhow!("MCP not initialized"))?;
        mcp.call_agent(&agent.role, prompt).await
    }

    fn check_consensus(&self) -> bool {
        let reached = self.session.history
            .iter()
            .filter(|m| !m.is_private)
            .last()
            .map(|m| m.content.contains("<consensus>true</consensus>"))
            .unwrap_or(false);
        if reached {
            debug!("Consensus tag detected in last message");
        }
        reached
    }

    fn parse_tool_call(&self, content: &str) -> Option<ToolCall> {
        let re = regex::Regex::new(r#"<tool_call\s+server="([^"]+)"\s+tool="([^"]+)"\s+args='([^']+)'\s*/>"#).ok()?;
        if let Some(caps) = re.captures(content) {
            let server = caps.get(1)?.as_str().to_string();
            let tool = caps.get(2)?.as_str().to_string();
            let args_str = caps.get(3)?.as_str();
            trace!(server, tool, args_str, "Parsed tool call");
            let args = serde_json::from_str(args_str).unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
            Some(ToolCall { server, tool, args })
        } else {
            None
        }
    }

    #[instrument(skip(self, call), fields(server = %call.server, tool = %call.tool))]
    async fn execute_tool_call(&self, call: &ToolCall) -> Result<String> {
        info!("Executing tool call");
        let mcp = self.mcp.as_ref().ok_or_else(|| anyhow::anyhow!("MCP not initialized"))?;
        match mcp.call_tool(&call.server, &call.tool, call.args.clone()).await {
            Ok(res) => {
                debug!(result_len = res.len(), "Tool call success");
                Ok(res)
            }
            Err(e) => {
                error!(error = %e, "Tool call failed");
                Err(e)
            }
        }
    }
}

struct ToolCall {
    server: String,
    tool: String,
    args: serde_json::Value,
}
