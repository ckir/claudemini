use async_trait::async_trait;
use rust_mcp_sdk::mcp_server::ServerHandler;
use rust_mcp_sdk::schema::{
    CallToolError, CallToolRequestParams, CallToolResult, ListToolsResult, TextContent, Tool,
    ToolInputSchema,
};
use rust_mcp_sdk::McpServer;
use std::process::Command;
use std::sync::Arc;
use std::collections::BTreeMap;
use serde_json::Map;

pub struct CliAgentHandler {
    pub name: String,
    pub command: String,
    pub arg_flag: String,
}

#[async_trait]
impl ServerHandler for CliAgentHandler {
    async fn handle_list_tools_request(
        &self,
        _params: Option<rust_mcp_sdk::schema::PaginatedRequestParams>,
        _runtime: Arc<dyn McpServer>,
    ) -> Result<ListToolsResult, rust_mcp_sdk::schema::RpcError> {
        let mut properties = BTreeMap::new();
        let mut prompt_schema = Map::new();
        prompt_schema.insert("type".to_string(), serde_json::json!("string"));
        prompt_schema.insert("description".to_string(), serde_json::json!("The message to send"));
        properties.insert("prompt".to_string(), prompt_schema);

        Ok(ListToolsResult {
            tools: vec![Tool {
                name: "chat".to_string(),
                description: Some(format!("Send a message to {}", self.name)),
                input_schema: ToolInputSchema::new(
                    vec!["prompt".to_string()],
                    Some(properties),
                    None,
                ),
                annotations: None,
                execution: None,
                icons: vec![],
                meta: None,
                output_schema: None,
                title: None,
            }],
            next_cursor: None,
            meta: None,
        })
    }

    async fn handle_call_tool_request(
        &self,
        params: CallToolRequestParams,
        _runtime: Arc<dyn McpServer>,
    ) -> Result<CallToolResult, CallToolError> {
        if params.name != "chat" {
            return Err(CallToolError(anyhow::anyhow!("Tool not found").into()));
        }

        let prompt = params
            .arguments
            .and_then(|args| {
                args.get("prompt")
                    .and_then(|p| p.as_str())
                    .map(|s| s.to_string())
            })
            .ok_or_else(|| CallToolError(anyhow::anyhow!("Missing prompt argument").into()))?;

        // Execute CLI
        let output = if cfg!(windows) && self.command.ends_with(".ps1") {
            Command::new("powershell.exe")
                .arg("-NoProfile")
                .arg("-Command")
                .arg(format!("& '{}' {} '{}'", self.command, self.arg_flag, prompt.replace("'", "''")))
                .output()
        } else if cfg!(unix) && self.command.ends_with(".ps1") {
            Command::new("pwsh")
                .arg("-NoProfile")
                .arg("-Command")
                .arg(format!("& '{}' {} '{}'", self.command, self.arg_flag, prompt.replace("'", "''")))
                .output()
        } else {
            Command::new(&self.command)
                .arg(&self.arg_flag)
                .arg(&prompt)
                .output()
        }
        .map_err(|e| CallToolError(anyhow::anyhow!("Failed to execute CLI: {}", e).into()))?;

        let response_text = if output.status.success() {
            String::from_utf8_lossy(&output.stdout).to_string()
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            if stderr.is_empty() {
                String::from_utf8_lossy(&output.stdout).to_string()
            } else {
                format!("Error: {}", stderr)
            }
        };

        Ok(CallToolResult {
            content: vec![TextContent::new(
                response_text,
                None,
                None,
            ).into()],
            is_error: Some(!output.status.success()),
            meta: None,
            structured_content: None,
        })
    }
}
