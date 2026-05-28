use crate::tui::TuiManager;
use anyhow::Result;
use clap::Parser;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct CliArgs {
    /// Enable "flight recorder" logging to file for debugging
    #[arg(short, long)]
    pub debug: bool,

    /// Validate configuration and MCP sidecars without starting the app
    #[arg(long)]
    pub dry_run: bool,
}

pub struct Cli;

impl Cli {
    pub async fn run() -> Result<()> {
        let args = CliArgs::parse();
        if args.dry_run {
            return Self::dry_run(args.debug).await;
        }
        TuiManager::run(args.debug).await
    }

    async fn dry_run(debug: bool) -> Result<()> {
        let _guard = if debug { Some(crate::logging::init()) } else { None };
        tracing::info!("Starting dry run validation...");
        println!("🔍 Starting Claudemini Dry Run...");

        // 1. Load Config
        let config_path = "claudemini.toml";
        let config_content = std::fs::read_to_string(config_path)
            .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", config_path, e))?;
        let config: crate::agent::AppConfig = toml::from_str(&config_content)
            .map_err(|e| anyhow::anyhow!("Invalid config format: {}", e))?;
        println!("✅ Configuration file is valid.");

        // 2. Validate Agents
        let current_exe = std::env::current_exe()?;
        let bin_dir = current_exe.parent().ok_or_else(|| anyhow::anyhow!("Could not find binary directory"))?;

        for agent in &config.agents {
            print!("Checking agent '{}' ({})... ", agent.name, agent.mcp_command);

            let cmd_exists = if agent.mcp_command.contains("/") || agent.mcp_command.contains("\\") {
                std::path::Path::new(&agent.mcp_command).exists()
            } else {
                let bin_name = if cfg!(windows) && !agent.mcp_command.ends_with(".exe") {
                    format!("{}.exe", agent.mcp_command)
                } else {
                    agent.mcp_command.clone()
                };

                bin_dir.join(&bin_name).exists() || which::which(&agent.mcp_command).is_ok()
            };
            if cmd_exists {
                println!("OK");
            } else {
                println!("FAILED");
                return Err(anyhow::anyhow!("MCP command '{}' for agent '{}' not found.", agent.mcp_command, agent.name));
            }
        }

        // 3. Emulate Conversation
        println!("💬 Emulating full conversation loop...");
        let mut orchestrator = crate::orchestrator::Orchestrator::new();
        orchestrator.debug_mode = debug;

        // Setup stubbed MCP
        orchestrator.mcp = Some(crate::mcp::McpClientWrapper::new_stub());

        // Manual agent population for stub mode
        orchestrator.agents = config.agents.iter().map(|a| {
            crate::agent::Agent::new(crate::agent::AgentRole(a.name.clone()), a.name.clone(), Some(a.persona.clone()))
        }).collect();

        // Run collaboration
        orchestrator.collaborate("Dry run test prompt").await?;

        println!("🚀 Dry run successful! All systems plumbed and loop emulation complete.");
        Ok(())
    }

}
