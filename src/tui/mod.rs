pub mod events;
pub use events::{TuiEvent, UserAction};

use crate::agent::{AgentRole, AppConfig};
use crate::orchestrator::Orchestrator;
use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame, Terminal,
};
use std::io;
use std::fs;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tui_textarea::TextArea;

struct App<'a> {
    pub input: TextArea<'a>,
    pub messages: Vec<MessageDisplay>,
    pub status: String,
    pub agent_thoughts: std::collections::HashMap<AgentRole, String>,
    pub agent_thinking: std::collections::HashMap<AgentRole, bool>,
    pub exit: bool,
}

#[derive(Clone)]
struct MessageDisplay {
    pub role: String,
    pub content: String,
    pub color: Color,
}

impl<'a> App<'a> {
    fn new(config: &AppConfig) -> App<'a> {
        let mut agent_thoughts = std::collections::HashMap::new();
        let mut agent_thinking = std::collections::HashMap::new();
        for agent in &config.agents {
            let role = AgentRole(agent.name.clone());
            agent_thoughts.insert(role.clone(), String::new());
            agent_thinking.insert(role, false);
        }

        App {
            input: TextArea::default(),
            messages: Vec::new(),
            status: "Ready".to_string(),
            agent_thoughts,
            agent_thinking,
            exit: false,
        }
    }
}

pub struct TuiManager;

impl TuiManager {
    pub async fn run() -> Result<()> {
        // Load config
        let config_path = "claudemini.toml";
        let config = if let Ok(content) = fs::read_to_string(config_path) {
            toml::from_str(&content).unwrap_or_else(|_| AppConfig::default_config())
        } else {
            let default = AppConfig::default_config();
            let toml_str = toml::to_string_pretty(&default).unwrap_or_default();
            let _ = fs::write(config_path, toml_str);
            default
        };

        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Create app and channels
        let mut app = App::new(&config);
        let (tui_tx, mut tui_rx) = mpsc::unbounded_channel::<TuiEvent>();
        let (user_tx, user_rx) = mpsc::channel::<UserAction>(10);

        // Create orchestrator
        let mut orchestrator = Orchestrator::new();
        orchestrator.set_channels(tui_tx.clone(), user_rx);

        // Orchestrator running state
        let mut is_running = false;
        let orchestrator = Arc::new(tokio::sync::Mutex::new(orchestrator));

        let tick_rate = Duration::from_millis(100);
        let mut last_tick = Instant::now();

        loop {
            terminal.draw(|f| ui(f, &mut app))?;

            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0));

            if event::poll(timeout)? {
                if let Event::Key(key) = event::read()? {
                    match key.code {
                        KeyCode::Esc => {
                            app.exit = true;
                        }
                        KeyCode::Enter => {
                            let input_text = app.input.lines()[0].clone();
                            if !input_text.is_empty() {
                                if input_text.to_lowercase() == "exit" || input_text.to_lowercase() == "quit" {
                                    app.exit = true;
                                } else if input_text.to_lowercase().contains("team") {
                                    if !is_running {
                                        let orch_clone = Arc::clone(&orchestrator);
                                        let config_clone = config.clone();
                                        let prompt = input_text.clone();
                                        tokio::spawn(async move {
                                            let mut orch = orch_clone.lock().await;
                                            if orch.mcp.is_none() {
                                                if let Err(e) = orch.init(config_clone).await {
                                                    let _ = orch.events_tx.as_ref().unwrap().send(TuiEvent::Error(format!("Init error: {}", e)));
                                                    return;
                                                }
                                            }
                                            if let Err(e) = orch.collaborate(&prompt).await {
                                                let _ = orch.events_tx.as_ref().unwrap().send(TuiEvent::Error(e.to_string()));
                                            }
                                        });
                                        is_running = true;
                                    } else {
                                        let _ = user_tx.send(UserAction::UserMessage(input_text)).await;
                                    }
                                } else if is_running {
                                    let _ = user_tx.send(UserAction::UserMessage(input_text)).await;
                                }
                                
                                // Reset textarea
                                app.input = TextArea::default();
                            }
                        }
                        _ => {
                            app.input.input(key);
                        }
                    }
                }
            }

            // Handle events from Orchestrator
            while let Ok(event) = tui_rx.try_recv() {
                match event {
                    TuiEvent::StatusUpdate(s) => app.status = s,
                    TuiEvent::AgentThinking(r) => {
                        app.agent_thinking.insert(r, true);
                    }
                    TuiEvent::AgentThought(r, t) => {
                        app.agent_thinking.insert(r.clone(), false);
                        app.agent_thoughts.insert(r, t);
                    }
                    TuiEvent::AgentResponse(r, content) => {
                        let color = if r.0.to_lowercase().contains("claude") {
                            Color::Magenta
                        } else if r.0.to_lowercase().contains("gemini") {
                            Color::Blue
                        } else if r.0 == "User" {
                            Color::White
                        } else {
                            Color::Cyan
                        };
                        app.messages.push(MessageDisplay {
                            role: r.0,
                            content,
                            color,
                        });
                    }
                    TuiEvent::ConsensusReached | TuiEvent::RoundComplete => {
                        // Potential to stop running state if desired, but loop continues
                    }
                    TuiEvent::Error(e) => app.status = format!("Error: {}", e),
                    _ => {}
                }
            }

            if app.exit {
                break;
            }

            if last_tick.elapsed() >= tick_rate {
                last_tick = Instant::now();
            }
        }

        // Restore terminal
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        Ok(())
    }
}


fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Status
            Constraint::Min(10),   // Main content
            Constraint::Length(3), // Input
        ])
        .split(f.area());

    // Status Bar
    let status_block = Block::default()
        .borders(Borders::ALL)
        .title("Claudemini - Team Collaboration");
    let status_para = Paragraph::new(app.status.as_str())
        .block(status_block)
        .style(Style::default().fg(Color::Yellow));
    f.render_widget(status_para, chunks[0]);

    // Main Area (Middle)
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30), // Thoughts
            Constraint::Percentage(70), // Dialogue
        ])
        .split(chunks[1]);

    // Thoughts (Left)
    let mut roles: Vec<AgentRole> = app.agent_thoughts.keys().cloned().collect();
    roles.sort_by(|a, b| a.0.cmp(&b.0));

    let thought_constraints: Vec<Constraint> = roles.iter().map(|_| Constraint::Percentage(100 / roles.len() as u16)).collect();
    let thought_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(thought_constraints)
        .split(main_chunks[0]);

    for (i, role) in roles.into_iter().enumerate() {
        render_thought_pane(f, app, role, thought_chunks[i]);
    }

    // Dialogue (Right)
    let dialogue_items: Vec<ListItem> = app.messages.iter().map(|m| {
        let header = Line::from(vec![
            Span::styled(format!("{}: ", m.role), Style::default().fg(m.color).add_modifier(Modifier::BOLD)),
        ]);
        ListItem::new(vec![
            header,
            Line::from(m.content.as_str()),
            Line::from(""),
        ])
    }).collect();

    let dialogue_list = List::new(dialogue_items)
        .block(Block::default().borders(Borders::ALL).title("Dialogue History"))
        .style(Style::default().fg(Color::White));
    f.render_widget(dialogue_list, main_chunks[1]);

    // Input Area (Bottom)
    app.input.set_block(Block::default().borders(Borders::ALL).title("Input (Type 'Team <goal>' to start)"));
    f.render_widget(&app.input, chunks[2]);
}

fn render_thought_pane(f: &mut Frame, app: &App, role: AgentRole, area: Rect) {
    let name = &role.0;
    let is_thinking = app.agent_thinking.get(&role).unwrap_or(&false);
    let thought = app.agent_thoughts.get(&role).cloned().unwrap_or_default();
    
    let title = if *is_thinking {
        format!("{} [Thinking...]", name)
    } else {
        format!("{}'s Private Scratchpad", name)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(if *is_thinking { Style::default().fg(Color::Green) } else { Style::default() });

    let para = Paragraph::new(thought)
        .block(block)
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(Color::Gray));
    
    f.render_widget(para, area);
}
