# Claudemini Session Resume

## Current Status
- **Project Structure:** Fully initialized Rust project (`tokio`, `serde`, `rust-mcp-sdk`).
- **Core Orchestrator:** Dynamic turn-based collaboration loop with **Parallel Thinking** (parallel scratchpads).
- **MCP Implementation:** Wrapped `claude.exe` and `gemini.ps1` as MCP servers; supports dynamic agent registration.
- **UI/UX:** Full **Terminal User Interface (TUI)** built with `ratatui`. Features real-time status, side-by-side agent scratchpads, and scrollable dialogue history.
- **Human-in-the-Loop:** Implemented mid-round intervention and "stop" command, integrated into TUI input.
- **Consensus Detection:** Keyword-based consensus signaling implemented.
- **Error Robustness:** Enhanced MCP error reporting (stderr capture) and `is_error` flag handling.
- **Persistent Memory:** Integrated `agentmemory` MCP for recall/save at turn boundaries.
- **Tool Access:** Agents can now use `filesystem` and `search` tools via `<tool_call />` tags.
- **Persona Injection:** Customizable agent roles (e.g. "Developer", "Reviewer") supported.
- **Scaling:** Refactored for **N-Agent Support** (dynamic agent mapping).
- **Compilation:** Project builds cleanly (`cargo build`).

## Roadmap & Future Enhancements

### Phase 1: Immediate Refinements (Completed)
1.  **Consensus Detection:** Implement logic in `src/orchestrator/mod.rs` to identify when agents agree on a solution. (Done)
2.  **Rich UI/UX:** Add terminal colors, Markdown rendering, and improved spacing for better readability. (Done)
3.  **Human-in-the-Loop:** Allow users to interrupt the loop to answer agent questions or provide course corrections mid-round. (Done)
4.  **Error Robustness:** Enhance handling for CLI timeouts and subprocess failures. (Done)

### Phase 2: Cognitive & Memory (Completed)
1.  **Persistent Memory:** Deeper integration with `agentmemory` MCP for long-term context retention. (Done)
2.  **Tool Access:** Enable agents to use external tools (web search, file system) via MCP. (Done)
3.  **Persona/Role Injection:** Allow users to define specific roles (e.g., "Developer", "Security Reviewer") for each agent. (Done)

### Phase 3: Scaling & Architecture (Completed)
1.  **N-Agent Support:** Expand the orchestrator to handle more than two agents simultaneously. (Done)
2.  **Model Agnosticism:** Allow swapping local models (Llama, etc.) into the MCP slots via `claudemini.toml`. (Done)
3.  **Parallel Thinking:** Optimize agents to generate private scratchpads in parallel to reduce turn latency. (Done)
4.  **TUI Implementation:** Transition from a linear CLI to a rich Terminal User Interface (using `ratatui`) for better layout management and real-time agent monitoring. (Done)


## Environment Notes
- **Claude CLI:** Expected in PATH or `%LOCALAPPDATA%\bin\claude.exe`.
- **Gemini CLI:** Expected in PATH or `C:\nvm4w\nodejs\gemini.ps1`.
- **MCP Binaries:** Located in `target/debug/` (or `target/release/`).
