use claudemini::mcp_server::CliAgentHandler;
use rust_mcp_sdk::mcp_server::server_runtime::create_server;
use rust_mcp_sdk::mcp_server::{McpServerOptions, ToMcpServerHandler};
use rust_mcp_sdk::schema::{Implementation, InitializeResult, ServerCapabilities, ServerCapabilitiesTools, LATEST_PROTOCOL_VERSION};
use rust_mcp_sdk::{StdioTransport, TransportOptions, McpServer};
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let handler = CliAgentHandler {
        name: "Claude".to_string(),
        command: "claude".to_string(),
        arg_flag: "-p".to_string(),
    };

    let server_info = Implementation {
        name: "claude-mcp-wrapper".to_string(),
        version: "0.1.0".to_string(),
        description: Some("MCP wrapper for Claude CLI".to_string()),
        icons: vec![],
        title: None,
        website_url: None,
    };

    let capabilities = ServerCapabilities {
        completions: None,
        experimental: None,
        logging: None,
        prompts: None,
        resources: None,
        tasks: None,
        tools: Some(ServerCapabilitiesTools {
            list_changed: Some(false),
        }),
    };

    let server_details = InitializeResult {
        capabilities,
        instructions: None,
        meta: None,
        protocol_version: LATEST_PROTOCOL_VERSION.to_string(),
        server_info,
    };

    let options = TransportOptions {
        timeout: Duration::from_secs(30),
    };

    let transport = StdioTransport::new(options)
        .map_err(|e| anyhow::anyhow!("Failed to create transport: {:?}", e))?;

    let mcp_options = McpServerOptions {
        server_details,
        transport,
        handler: handler.to_mcp_server_handler(),
        task_store: None,
        client_task_store: None,
        message_observer: None,
    };

    let server = create_server(mcp_options);
    server.start().await.map_err(|e| anyhow::anyhow!("Server error: {:?}", e))?;

    Ok(())
}
