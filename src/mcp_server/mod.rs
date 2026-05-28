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
use tracing::{info, debug, trace, error, instrument};

pub struct CliAgentHandler {
    pub name: String,
    pub command: String,
    pub arg_flag: String,
}

#[async_trait]
impl ServerHandler for CliAgentHandler {
    #[instrument(skip(self, _params, _runtime))]
    async fn handle_list_tools_request(
        &self,
        _params: Option<rust_mcp_sdk::schema::PaginatedRequestParams>,
        _runtime: Arc<dyn McpServer>,
    ) -> Result<ListToolsResult, rust_mcp_sdk::schema::RpcError> {
        debug!(agent = %self.name, "Handling list_tools request");
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

    #[instrument(skip(self, params, _runtime), fields(tool = %params.name))]
    async fn handle_call_tool_request(
        &self,
        params: CallToolRequestParams,
        _runtime: Arc<dyn McpServer>,
    ) -> Result<CallToolResult, CallToolError> {
        info!(agent = %self.name, "Handling call_tool request");
        if params.name != "chat" {
            error!(tool = %params.name, "Tool not found");
            return Err(CallToolError(anyhow::anyhow!("Tool not found").into()));
        }

        let prompt = params
            .arguments
            .and_then(|args| {
                args.get("prompt")
                    .and_then(|p| p.as_str())
                    .map(|s| s.to_string())
            })
            .ok_or_else(|| {
                error!("Missing prompt argument");
                CallToolError(anyhow::anyhow!("Missing prompt argument").into())
            })?;

        debug!(prompt_len = prompt.len(), "Executing CLI command");

        // Execute CLI
        let output_result = if self.command.ends_with(".ps1") {
            if cfg!(windows) {
                let cmd = format!("& '{}' {} '{}'", self.command, self.arg_flag, prompt.replace("'", "''"));
                trace!(powershell_cmd = %cmd, "Spawning powershell");
                Command::new("powershell.exe")
                    .arg("-NoProfile")
                    .arg("-Command")
                    .arg(cmd)
                    .output()
            } else {
                let cmd = format!("& '{}' {} '{}'", self.command, self.arg_flag, prompt.replace("'", "''"));
                trace!(pwsh_cmd = %cmd, "Spawning pwsh");
                Command::new("pwsh")
                    .arg("-NoProfile")
                    .arg("-Command")
                    .arg(cmd)
                    .output()
            }
        } else if self.command.ends_with(".sh") {
            if cfg!(windows) {
                 error!(command = %self.command, "Shell scripts (.sh) not supported on Windows");
                 return Err(CallToolError(anyhow::anyhow!("Shell scripts not supported on Windows natively").into()));
            } else {
                trace!(command = %self.command, "Spawning bash script");
                Command::new("bash")
                    .arg(&self.command)
                    .arg(&self.arg_flag)
                    .arg(&prompt)
                    .output()
            }
        } else {
            trace!(command = %self.command, flag = %self.arg_flag, "Spawning direct command");
            Command::new(&self.command)
                .arg(&self.arg_flag)
                .arg(&prompt)
                .output()
        };

        let output = output_result.map_err(|e| {
            error!(error = %e, "Failed to execute CLI");
            CallToolError(anyhow::anyhow!("Failed to execute CLI: {}", e).into())
        })?;

        let response_text = if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            trace!(stdout_len = stdout.len(), "CLI executed successfully");
            stdout
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            error!(
                status = ?output.status.code(),
                stderr_len = stderr.len(),
                stdout_len = stdout.len(),
                "CLI execution failed"
            );
            if stderr.is_empty() {
                stdout
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
