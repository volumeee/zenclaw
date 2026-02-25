//! TUI Application state & loop

use ratatui::crossterm::event::{Event, KeyCode, KeyModifiers};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Terminal,
};
use std::time::Duration;
use tokio::sync::mpsc;
use tui_textarea::{Input, TextArea};

use zenclaw_core::agent::Agent;
use zenclaw_core::memory::MemoryStore;
use zenclaw_core::provider::LlmProvider;
use zenclaw_core::bus::EventBus;

#[derive(Clone)]
pub enum AppEvent {
    Terminal(Event),
    Tick,
    AgentStatus(String),
    AgentResponse(String, Vec<String>), // response_text, tool_calls
    AgentError(String),
}

pub struct ChatMessage {
    pub role: String,
    pub content: String,
    pub displayed_length: usize,
    pub is_fully_loaded: bool,
}

pub struct App<'a> {
    pub textarea: TextArea<'a>,
    pub messages: Vec<ChatMessage>,
    pub is_processing: bool,
    pub status_text: String,
    pub spinner_idx: usize,
    pub should_quit: bool,
    pub scroll_offset: u16,
    pub current_task_handle: Option<tokio::task::JoinHandle<()>>,
}

const SPINNER: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

impl<'a> App<'a> {
    pub fn new() -> Self {
        let mut textarea = TextArea::default();
        textarea.set_block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Message (Press Enter to Send, Ctrl+C to Quit, Ctrl+S to Scroll) "),
        );
        textarea.set_style(Style::default().fg(Color::Cyan));
        Self {
            textarea,
            messages: Vec::new(),
            is_processing: false,
            status_text: String::new(),
            spinner_idx: 0,
            should_quit: false,
            scroll_offset: 0,
            current_task_handle: None,
        }
    }
}

pub async fn run_tui(
    mut terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
    agent: std::sync::Arc<Agent>,
    provider: std::sync::Arc<dyn LlmProvider>,
    memory: std::sync::Arc<dyn MemoryStore>,
    session_key: String,
    bus: std::sync::Arc<EventBus>,
) -> anyhow::Result<()> {
    let mut app = App::new();

    // Spawn an event loop so we don't block the UI
    let (tx, mut rx) = mpsc::channel(100);

    let tx_tick = tx.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(100)).await;
            if tx_tick.send(AppEvent::Tick).await.is_err() {
                break;
            }
        }
    });

    let tx_input = tx.clone();
    tokio::task::spawn_blocking(move || {
        loop {
            if ratatui::crossterm::event::poll(Duration::from_millis(200)).unwrap_or(false) {
                if let Ok(event) = ratatui::crossterm::event::read() {
                    let _ = tx_input.blocking_send(AppEvent::Terminal(event));
                }
            }
        }
    });

    let mut bus_rx = bus.subscribe_system();
    let tx_bus = tx.clone();
    tokio::spawn(async move {
        while let Ok(ev) = bus_rx.recv().await {
            if let Some(msg) = ev.format_status() {
                let _ = tx_bus.send(AppEvent::AgentStatus(msg)).await;
            }
        }
    });

    // Main UI Loop
    while !app.should_quit {
        terminal.draw(|f| draw_ui(f, &app))?;

        if let Some(event) = rx.recv().await {
            match event {
                AppEvent::Tick => {
                    app.spinner_idx = (app.spinner_idx + 1) % SPINNER.len();
                    
                    // Progressive streaming effect for the last message
                    if let Some(msg) = app.messages.last_mut() {
                        if !msg.is_fully_loaded {
                            msg.displayed_length += 20; // Show 20 chars per tick (approx 200 chars/sec)
                            let total_chars = msg.content.chars().count();
                            if msg.displayed_length >= total_chars {
                                msg.displayed_length = total_chars;
                                msg.is_fully_loaded = true;
                            }
                        }
                    }
                }
                AppEvent::AgentStatus(msg) => {
                    app.status_text = msg;
                }
                AppEvent::AgentResponse(msg, _blocks) => {
                    app.is_processing = false;
                    app.status_text.clear();
                    app.messages.push(ChatMessage {
                        role: "AI".into(),
                        content: msg,
                        displayed_length: 0,
                        is_fully_loaded: false,
                    });
                }
                AppEvent::AgentError(msg) => {
                    app.is_processing = false;
                    app.status_text.clear();
                    app.messages.push(ChatMessage {
                        role: "System (Error)".into(),
                        content: msg,
                        displayed_length: 0,
                        is_fully_loaded: false,
                    });
                }
                AppEvent::Terminal(Event::Paste(text)) => {
                    if !app.is_processing {
                        // Properly insert multiline pasted text cleanly (autoformat/no corruption)
                        let normalized = text.replace("\r\n", "\n");
                        for (i, line) in normalized.split('\n').enumerate() {
                            if i > 0 {
                                app.textarea.insert_newline();
                            }
                            app.textarea.insert_str(line);
                        }
                    }
                }
                AppEvent::Terminal(Event::Key(key)) => {
                    // Check for Ctrl+C to exit or cancel
                    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                        if app.is_processing {
                            if let Some(handle) = app.current_task_handle.take() {
                                handle.abort();
                            }
                            app.is_processing = false;
                            app.status_text.clear();
                            app.messages.push(ChatMessage {
                                role: "System".into(),
                                content: "⚠️ Request cancelled by user.".into(),
                                displayed_length: 999,
                                is_fully_loaded: true,
                            });
                        } else {
                            app.should_quit = true;
                        }
                        continue;
                    }
                    
                    // Simple Scrolling mapped to Up/Down while holding Shift or Ctrl for chat view
                    // (Assuming you can customize keys. Shift+Up/Down is nice)
                    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Up {
                        app.scroll_offset = app.scroll_offset.saturating_add(1);
                        continue;
                    }
                    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Down {
                        app.scroll_offset = app.scroll_offset.saturating_sub(1);
                        continue;
                    }

                    // Check if user is typing
                    let is_enter = key.code == KeyCode::Enter;
                    let has_shift = key.modifiers.contains(KeyModifiers::SHIFT);
                    let has_alt = key.modifiers.contains(KeyModifiers::ALT);

                    // If Alt+Enter or Shift+Enter -> Insert newline manually
                    if is_enter && (has_shift || has_alt) {
                        if !app.is_processing {
                            app.textarea.insert_newline();
                        }
                        continue;
                    }

                    // If pure Enter -> Send message
                    if is_enter && !has_shift && !has_alt {
                        if app.is_processing {
                            continue; // Block sending while processing!
                        }

                        let text = app.textarea.lines().join("\n").trim().to_string();
                        if text.is_empty() {
                            continue;
                        }

                        // Add to chat and clear input
                        app.messages.push(ChatMessage {
                            role: "You".into(),
                            content: text.clone(),
                            displayed_length: text.chars().count(),
                            is_fully_loaded: true,
                        });
                        app.textarea = TextArea::default(); // refresh
                        app.textarea.set_block(Block::default().borders(Borders::ALL).title(" Message "));
                        app.textarea.set_style(Style::default().fg(Color::Cyan));
                        app.scroll_offset = 0; // jump back to bottom on send

                        app.is_processing = true;
                        app.status_text = "Processing...".into();

                        let agent_c = agent.clone();
                        let provider_c = provider.clone();
                        let memory_c = memory.clone();
                        let tx_c = tx.clone();
                        let sk = session_key.clone();
                        let bus_c = bus.clone();

                        let handle = tokio::spawn(async move {
                            match agent_c.process(&*provider_c, &*memory_c, &text, &sk, Some(&bus_c)).await {
                                Ok(resp) => { let _ = tx_c.send(AppEvent::AgentResponse(resp, vec![])).await; },
                                Err(e) => { let _ = tx_c.send(AppEvent::AgentError(e.to_string())).await; },
                            }
                        });
                        app.current_task_handle = Some(handle);
                    } else {
                        // Regular typing in textarea
                        if !app.is_processing {
                            app.textarea.input(Input::from(key));
                        }
                    }
                }
                _ => {}
            }
        }
    }

    Ok(())
}

fn draw_ui(f: &mut ratatui::Frame, app: &App) {
    let input_lines = app.textarea.lines().len() as u16;
    // Calculate required height based on content + 2 for borders
    let max_height = f.area().height.saturating_sub(10); // leave space for chat
    let desired_height = std::cmp::max(3, input_lines + 2); // At least 3 lines
    let actual_height = std::cmp::max(3, std::cmp::min(15, std::cmp::min(desired_height, max_height)));

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(actual_height)].as_ref())
        .split(f.area());

    // Messages Area
    let mut lines = Vec::new();

    // Welcome message if chat empty
    if app.messages.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled("✨ Welcome to ZenClaw AI!", Style::default().fg(Color::Yellow).bold())]));
        lines.push(Line::from("I'm ready to help you code, build, and debug. Type your request below."));
        lines.push(Line::from(""));
    }

    for msg in &app.messages {
        let role_style = if msg.role == "You" {
            Style::default().fg(Color::Yellow).bold()
        } else if msg.role == "AI" {
            Style::default().fg(Color::Cyan).bold()
        } else {
            Style::default().fg(Color::Red).bold()
        };

        lines.push(Line::from(vec![
            Span::styled(format!("{} › ", msg.role), role_style),
        ]));

        let text_to_show = if msg.is_fully_loaded {
            msg.content.clone()
        } else {
            msg.content.chars().take(msg.displayed_length).collect()
        };

        for line in text_to_show.lines() {
            lines.push(Line::from(line.to_string()));
        }
        
        if msg.role != "System" && msg.is_fully_loaded {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled("─".repeat(50), Style::default().fg(Color::DarkGray))));
        }
        lines.push(Line::from(""));
    }

    if app.is_processing {
        lines.push(Line::from(vec![
            Span::styled(SPINNER[app.spinner_idx], Style::default().fg(Color::Cyan).bold()),
            Span::raw(" "),
            Span::styled(&app.status_text, Style::default().fg(Color::DarkGray).italic()),
        ]));
    }

    let chat_width = chunks[0].width.saturating_sub(2) as usize;
    let chat_height = chunks[0].height.saturating_sub(2);
    
    // Compute total wrapped lines
    let mut total_visual_lines: u16 = 0;
    for l in &lines {
        let text_len = l.width();
        if text_len == 0 {
            total_visual_lines += 1;
        } else {
            let wrapped = (text_len + chat_width.saturating_sub(1)) / std::cmp::max(1, chat_width);
            total_visual_lines += wrapped as u16;
        }
    }

    let max_scroll = total_visual_lines.saturating_sub(chat_height);
    let view_offset = max_scroll.saturating_sub(app.scroll_offset);

    let chat_block = Paragraph::new(lines)
        .block(
            Block::default()
                .title(Span::styled(" ZenClaw AI Dashboard ", Style::default().fg(Color::Cyan).bold()))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
        )
        .wrap(Wrap { trim: false })
        .scroll((view_offset, 0));

    f.render_widget(chat_block, chunks[0]);

    // Input Area
    let help_text = if app.is_processing {
        "[Ctrl+C] Cancel Process │ [Ctrl+Up/Down] Scroll "
    } else {
        "[Enter] Send │ [Alt/Shift+Enter] New Line │ [Ctrl+C] Quit │ [Ctrl+Up/Down] Scroll "
    };

    let mut textarea = app.textarea.clone();
    textarea.set_block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(if app.is_processing { Style::default().fg(Color::DarkGray) } else { Style::default().fg(Color::Cyan) })
            .title(Span::styled(" Message ", Style::default().fg(Color::White).bold()))
            .title_bottom(Span::styled(help_text, Style::default().fg(Color::DarkGray)))
    );

    f.render_widget(&textarea, chunks[1]);
}
