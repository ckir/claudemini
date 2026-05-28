use rust_mcp_sdk::mcp_client::client_runtime::create_client;
use rust_mcp_sdk::mcp_client::{ClientHandler, McpClientOptions, ToMcpClientHandler};
use rust_mcp_sdk::schema::{Implementation, CallToolRequestParams, CallToolResult, InitializeRequestParams, ClientCapabilities};
use rust_mcp_sdk::{StdioTransport, TransportOptions, McpClient};
use std::sync::Arc;
use std::time::Duration;
use std::collections::HashMap;
use async_trait::async_trait;
use crate::agent::{AgentRole, AppConfig};
use tracing::{info, debug, trace, error, instrument};

pub struct McpClientWrapper {
    agents: HashMap<AgentRole, Arc<dyn McpClient>>,
    memory: Option<Arc<dyn McpClient>>,
    filesystem: Option<Arc<dyn McpClient>>,
    search: Option<Arc<dyn McpClient>>,
    pub is_stub: bool,
}

#[derive(Clone)]
pub struct SimpleClientHandler;

#[async_trait]
impl ClientHandler for SimpleClientHandler {}

impl McpClientWrapper {
    pub fn new_stub() -> Self {
        Self {
            agents: HashMap::new(),
            memory: None,
            filesystem: None,
            search: None,
            is_stub: true,
        }
    }

    #[instrument(skip(config))]
    pub async fn new(config: AppConfig, debug_mode: bool) -> anyhow::Result<Self> {
        info!(debug_mode, "Creating new McpClientWrapper");
        let options = TransportOptions {
            timeout: Duration::from_secs(30),
        };

        let client_info = Implementation {
            name: "claudemini-orchestrator".to_string(),
            version: "0.1.0".to_string(),
            description: Some("Orchestrator client for custom AI agent teams".to_string()),
            icons: vec![],
            title: None,
            website_url: None,
        };

        let client_details = InitializeRequestParams {
            capabilities: ClientCapabilities {
                elicitation: None,
                experimental: None,
                roots: None,
                sampling: None,
                tasks: None,
            },
            client_info,
            meta: None,
            protocol_version: "2024-11-05".to_string(),
        };

        // Resolve paths to the sidecar binaries
        let current_exe = std::env::current_exe()?;
        let bin_dir = current_exe.parent().ok_or_else(|| anyhow::anyhow!("Could not find binary directory"))?;
        
        let mut agents: HashMap<AgentRole, Arc<dyn McpClient>> = HashMap::new();

        for agent_cfg in config.agents {
            let role = AgentRole(agent_cfg.name.clone());
            let mcp_cmd = if agent_cfg.mcp_command.contains("/") || agent_cfg.mcp_command.contains("\\") {
                agent_cfg.mcp_command.clone()
            } else {
                let bin_name = if cfg!(windows) && !agent_cfg.mcp_command.ends_with(".exe") {
                    format!("{}.exe", agent_cfg.mcp_command)
                } else {
                    agent_cfg.mcp_command.clone()
                };

                let local_bin = bin_dir.join(&bin_name);
                if local_bin.exists() {
                    local_bin.to_string_lossy().to_string()
                } else {
                    // Fallback to system PATH
                    agent_cfg.mcp_command.clone()
                }
            };

            let mut mcp_args = agent_cfg.mcp_args.clone();
            if debug_mode {
                mcp_args.push("--debug".to_string());
            }

            let transport = StdioTransport::create_with_server_launch(
                mcp_cmd,
                mcp_args,
                None,
                options.clone(),
            ).map_err(|e| anyhow::anyhow!("Transport error for {}: {:?}", agent_cfg.name, e))?;

            let client = create_client(McpClientOptions {
                client_details: client_details.clone(),
                transport,
                handler: SimpleClientHandler.to_mcp_client_handler(),
                task_store: None,
                server_task_store: None,
                message_observer: None,
            });

            client.clone().start().await.map_err(|e| anyhow::anyhow!("Client start error for {}: {:?}", agent_cfg.name, e))?;
            agents.insert(role, client as Arc<dyn McpClient>);
        }

        let memory_transport = StdioTransport::create_with_server_launch(
            if cfg!(windows) { "npx.cmd" } else { "npx" },
            vec!["-y".to_string(), "@agentmemory/mcp".to_string()],
            None,
            options.clone(),
        ).map_err(|e| anyhow::anyhow!("Memory transport error: {:?}", e))?;

        let filesystem_transport = StdioTransport::create_with_server_launch(
            if cfg!(windows) { "npx.cmd" } else { "npx" },
            vec!["-y".to_string(), "@modelcontextprotocol/server-filesystem".to_string(), ".".to_string()],
            None,
            options.clone(),
        ).map_err(|e| anyhow::anyhow!("Filesystem transport error: {:?}", e))?;

        let search_transport = StdioTransport::create_with_server_launch(
            if cfg!(windows) { "npx.cmd" } else { "npx" },
            vec!["-y".to_string(), "@brave/brave-search-mcp-server".to_string()],
            None,
            options,
        ).map_err(|e| anyhow::anyhow!("Search transport error: {:?}", e))?;

        let memory_client = create_client(McpClientOptions {
            client_details: client_details.clone(),
            transport: memory_transport,
            handler: SimpleClientHandler.to_mcp_client_handler(),
            task_store: None,
            server_task_store: None,
            message_observer: None,
        });

        let filesystem_client = create_client(McpClientOptions {
            client_details: client_details.clone(),
            transport: filesystem_transport,
            handler: SimpleClientHandler.to_mcp_client_handler(),
            task_store: None,
            server_task_store: None,
            message_observer: None,
        });

        let search_client = create_client(McpClientOptions {
            client_details,
            transport: search_transport,
            handler: SimpleClientHandler.to_mcp_client_handler(),
            task_store: None,
            server_task_store: None,
            message_observer: None,
        });

        memory_client.clone().start().await.map_err(|e| anyhow::anyhow!("Memory client start error: {:?}", e))?;
        filesystem_client.clone().start().await.map_err(|e| anyhow::anyhow!("Filesystem client start error: {:?}", e))?;
        search_client.clone().start().await.map_err(|e| anyhow::anyhow!("Search client start error: {:?}", e))?;

        Ok(Self {
            agents,
            memory: Some(memory_client),
            filesystem: Some(filesystem_client),
            search: Some(search_client),
            is_stub: false,
        })
    }

    #[instrument(skip(self, prompt), fields(role = %role.0))]
    pub async fn call_agent(&self, role: &AgentRole, prompt: &str) -> anyhow::Result<String> {
        if self.is_stub {
            debug!(role = %role.0, "Stubbed agent call");
            let response = if prompt.contains("PRIVATE thoughts") {
                format!("Stubbed PRIVATE thoughts for {}. I am analyzing the prompt: '{}'", role.0, prompt.chars().take(20).collect::<String>())
            } else {
                format!("Stubbed PUBLIC response from {}. I agree with the plan. <consensus>true</consensus>", role.0)
            };
            return Ok(response);
        }
        debug!(prompt_len = prompt.len(), "Calling agent");
        let client = self.agents.get(role).ok_or_else(|| anyhow::anyhow!("Agent not found for role: {:?}", role))?;
        let mut args = serde_json::Map::new();
        args.insert("prompt".to_string(), serde_json::json!(prompt));

        let result = client.request_tool_call(CallToolRequestParams {
            name: "chat".to_string(),
            arguments: Some(args),
            meta: None,
            task: None,
        }).await.map_err(|e| {
            error!(error = %e, "Agent tool call failed");
            anyhow::anyhow!("Agent tool call error: {:?}", e)
        })?;

        if result.is_error.unwrap_or(false) {
            let err_msg = self.format_result(result);
            error!(error = %err_msg, "Agent returned error");
            return Err(anyhow::anyhow!("Agent error: {}", err_msg));
        }

        let output = self.format_result(result);
        trace!(output_len = output.len(), "Agent call success");
        Ok(output)
    }

    #[instrument(skip(self, query))]
    pub async fn memory_recall(&self, query: &str) -> anyhow::Result<String> {
        if self.is_stub {
            return Ok("Stubbed memory recall".to_string());
        }
        debug!(query, "Recalling memory");
        let client = self.memory.as_ref().ok_or_else(|| anyhow::anyhow!("Memory client not available"))?;
        let mut args = serde_json::Map::new();
        args.insert("query".to_string(), serde_json::json!(query));

        let result = client.request_tool_call(CallToolRequestParams {
            name: "memory_recall".to_string(),
            arguments: Some(args),
            meta: None,
            task: None,
        }).await.map_err(|e| {
            error!(error = %e, "Memory recall failed");
            anyhow::anyhow!("Memory recall error: {:?}", e)
        })?;

        let output = self.format_result(result);
        trace!(output_len = output.len(), "Memory recall success");
        Ok(output)
    }

    #[instrument(skip(self, content))]
    pub async fn memory_save(&self, content: &str) -> anyhow::Result<()> {
        if self.is_stub {
            return Ok(());
        }
        debug!(content_len = content.len(), "Saving memory");
        let client = self.memory.as_ref().ok_or_else(|| anyhow::anyhow!("Memory client not available"))?;
        let mut args = serde_json::Map::new();
        args.insert("content".to_string(), serde_json::json!(content));

        let _result = client.request_tool_call(CallToolRequestParams {
            name: "memory_save".to_string(),
            arguments: Some(args),
            meta: None,
            task: None,
        }).await.map_err(|e| {
            error!(error = %e, "Memory save failed");
            anyhow::anyhow!("Memory save error: {:?}", e)
        })?;

        debug!("Memory save success");
        Ok(())
    }

    #[instrument(skip(self, arguments))]
    pub async fn call_tool(&self, server: &str, tool_name: &str, arguments: serde_json::Value) -> anyhow::Result<String> {
        if self.is_stub {
            return Ok(format!("Stubbed tool result for {}:{}", server, tool_name));
        }
        info!(server, tool_name, "Calling tool");
        let client = match server {
            "filesystem" => self.filesystem.as_ref().ok_or_else(|| anyhow::anyhow!("Filesystem client not available"))?,
            "search" => self.search.as_ref().ok_or_else(|| anyhow::anyhow!("Search client not available"))?,
            _ => {
                error!(server, "Unknown tool server");
                return Err(anyhow::anyhow!("Unknown tool server: {}", server));
            }
        };

        let args = arguments.as_object().cloned();
        let result = client.request_tool_call(CallToolRequestParams {
            name: tool_name.to_string(),
            arguments: args,
            meta: None,
            task: None,
        }).await.map_err(|e| {
            error!(error = %e, "Tool call failed");
            anyhow::anyhow!("Tool call error: {:?}", e)
        })?;

        let output = self.format_result(result);
        trace!(output_len = output.len(), "Tool call success");
        Ok(output)
    }

    fn format_result(&self, result: CallToolResult) -> String {
        let mut output = String::new();
        for block in result.content {
            match block {
                rust_mcp_sdk::schema::ContentBlock::TextContent(t) => {
                    output.push_str(&t.text);
                }
                _ => {}
            }
        }
        output
    }
}
