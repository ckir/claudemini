use rust_mcp_sdk::mcp_client::client_runtime::create_client;
use rust_mcp_sdk::mcp_client::{ClientHandler, McpClientOptions, ToMcpClientHandler};
use rust_mcp_sdk::schema::{Implementation, CallToolRequestParams, CallToolResult, InitializeRequestParams, ClientCapabilities, LATEST_PROTOCOL_VERSION};
use rust_mcp_sdk::{StdioTransport, TransportOptions, McpClient};
use std::sync::Arc;
use std::time::Duration;
use std::collections::HashMap;
use async_trait::async_trait;
use crate::agent::{AgentRole, AppConfig};

pub struct McpClientWrapper {
    agents: HashMap<AgentRole, Arc<dyn McpClient>>,
    memory: Arc<dyn McpClient>,
    filesystem: Arc<dyn McpClient>,
    search: Arc<dyn McpClient>,
}

#[derive(Clone)]
pub struct SimpleClientHandler;

#[async_trait]
impl ClientHandler for SimpleClientHandler {}

impl McpClientWrapper {
    pub async fn new(config: AppConfig) -> anyhow::Result<Self> {
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
            protocol_version: LATEST_PROTOCOL_VERSION.to_string(),
        };

        // Resolve paths to the sidecar binaries
        let current_exe = std::env::current_exe()?;
        let bin_dir = current_exe.parent().ok_or_else(|| anyhow::anyhow!("Could not find binary directory"))?;
        
        let mut agents: HashMap<AgentRole, Arc<dyn McpClient>> = HashMap::new();

        for agent_cfg in config.agents {
            let role = AgentRole(agent_cfg.name.clone());
            let mcp_cmd = if agent_cfg.mcp_command.ends_with(".exe") || agent_cfg.mcp_command.contains("/") || agent_cfg.mcp_command.contains("\\") {
                agent_cfg.mcp_command.clone()
            } else {
                let bin_name = if cfg!(windows) { format!("{}.exe", agent_cfg.mcp_command) } else { agent_cfg.mcp_command.clone() };
                bin_dir.join(bin_name).to_string_lossy().to_string()
            };

            let transport = StdioTransport::create_with_server_launch(
                mcp_cmd,
                agent_cfg.mcp_args,
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
            memory: memory_client,
            filesystem: filesystem_client,
            search: search_client,
        })
    }

    pub async fn call_agent(&self, role: &AgentRole, prompt: &str) -> anyhow::Result<String> {
        let client = self.agents.get(role).ok_or_else(|| anyhow::anyhow!("Agent not found for role: {:?}", role))?;
        let mut args = serde_json::Map::new();
        args.insert("prompt".to_string(), serde_json::json!(prompt));

        let result = client.request_tool_call(CallToolRequestParams {
            name: "chat".to_string(),
            arguments: Some(args),
            meta: None,
            task: None,
        }).await.map_err(|e| anyhow::anyhow!("Agent tool call error: {:?}", e))?;

        if result.is_error.unwrap_or(false) {
            return Err(anyhow::anyhow!("Agent error: {}", self.format_result(result)));
        }

        Ok(self.format_result(result))
    }

    pub async fn memory_recall(&self, query: &str) -> anyhow::Result<String> {
        let mut args = serde_json::Map::new();
        args.insert("query".to_string(), serde_json::json!(query));

        let result = self.memory.request_tool_call(CallToolRequestParams {
            name: "memory_recall".to_string(),
            arguments: Some(args),
            meta: None,
            task: None,
        }).await.map_err(|e| anyhow::anyhow!("Memory recall error: {:?}", e))?;

        Ok(self.format_result(result))
    }

    pub async fn memory_save(&self, content: &str) -> anyhow::Result<()> {
        let mut args = serde_json::Map::new();
        args.insert("content".to_string(), serde_json::json!(content));

        let _result = self.memory.request_tool_call(CallToolRequestParams {
            name: "memory_save".to_string(),
            arguments: Some(args),
            meta: None,
            task: None,
        }).await.map_err(|e| anyhow::anyhow!("Memory save error: {:?}", e))?;

        Ok(())
    }

    pub async fn call_tool(&self, server: &str, tool_name: &str, arguments: serde_json::Value) -> anyhow::Result<String> {
        let client = match server {
            "filesystem" => &self.filesystem,
            "search" => &self.search,
            _ => return Err(anyhow::anyhow!("Unknown tool server: {}", server)),
        };

        let args = arguments.as_object().cloned();
        let result = client.request_tool_call(CallToolRequestParams {
            name: tool_name.to_string(),
            arguments: args,
            meta: None,
            task: None,
        }).await.map_err(|e| anyhow::anyhow!("Tool call error: {:?}", e))?;

        Ok(self.format_result(result))
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
