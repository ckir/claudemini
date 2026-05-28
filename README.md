# Claudemini

Claudemini is a robust, multi-agent collaboration platform built in Rust. It enables seamless coordination between different AI models (like Claude and Gemini, or local models via Ollama) through a professional Terminal User Interface (TUI).

## Key Features

- **Rich TUI (Ratatui):** A sophisticated, multi-pane interface for real-time monitoring of agent thinking and public dialogue.
- **Dynamic Team Collaboration:** Define an arbitrary number of agents with specialized personas in a simple configuration file.
- **Model Agnosticism:** Easily swap between cloud-based APIs and local models (e.g., Llama via Ollama) using the Model Context Protocol (MCP).
- **Parallel Thinking:** Agents generate their private scratchpads simultaneously, significantly reducing latency.
- **Persistent Memory:** Integrated with `agentmemory` MCP for long-term context retention and learning across sessions.
- **Tool Access:** Agents can interact with the real world using tools like `filesystem` (read/write/search) and `brave-search` via `<tool_call />` tags.
- **Human-in-the-Loop:** Users can intervene between rounds to provide feedback or course corrections.

## Getting Started

### Prerequisites

- Rust (latest stable)
- Node.js & npm (for MCP servers)
- Claude CLI and/or Gemini CLI (for default configuration)

### Installation

1. Clone the repository:
   ```bash
   git clone https://github.com/your-username/claudemini.git
   cd claudemini
   ```

2. Build the project:
   ```bash
   cargo build --release
   ```

### Usage

1. Run the application:
   ```bash
   cargo run
   ```

2. In the TUI, type `Team <your goal>` to start a collaboration.
3. To customize your agent team, edit the automatically generated `claudemini.toml` file in the root directory.

## Configuration (`claudemini.toml`)

Example configuration:

```toml
[[agents]]
name = "Claude"
persona = "Expert Rust Developer"
mcp_command = "claude_mcp"
mcp_args = []

[[agents]]
name = "Gemini"
persona = "Strategic Solution Architect"
mcp_command = "gemini_mcp"
mcp_args = []
```

## Architecture

Claudemini uses a modular architecture:
- **Orchestrator:** Manages the collaboration loop, parallel thinking, and consensus detection.
- **MCP Client:** Handles communication with various AI models and external tools.
- **TUI:** Provides a responsive and interactive user interface.
- **Agent System:** Flexible struct-based agents with customizable personas.

## License

MIT
