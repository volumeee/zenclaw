use ratatui::{
    backend::CrosstermBackend,
    crossterm::event::{Event, KeyCode, KeyModifiers, MouseEventKind, KeyEventKind},
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Terminal,
};
use std::cell::Cell;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tui_textarea::{Input, TextArea};

use zenclaw_core::agent::Agent;
use zenclaw_core::memory::MemoryStore;
use zenclaw_core::provider::LlmProvider;
use zenclaw_core::bus::EventBus;

use crate::theme::THEME;

#[derive(Clone)]
pub enum AppEvent {
    Terminal(Event),
    Tick,
    AgentStatus(String),
    AgentResponse(String, Vec<String>), // response_text, tool_calls
    AgentError(String),
    ToolStart(String, String),           // tool_name, context
    ToolComplete(String, u64, Duration), // tool_name, result_bytes, elapsed
}

/// A tool call entry displayed inline in the chat.
#[derive(Clone)]
pub struct ToolEntry {
    pub name: String,
    pub context: String,
    pub started: Instant,
    pub done: Option<(u64, Duration)>, // (result_bytes, elapsed)
}

pub struct ChatMessage {
    pub role: String,
    pub content: String,
    pub displayed_length: usize,
    pub is_fully_loaded: bool,
    pub tool_entries: Vec<ToolEntry>, // tool calls that happened before this response
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
    // ─── New fields ──────────────────────────────────
    pub pending_tools: Vec<ToolEntry>,
    pub token_in: u64,
    pub token_out: u64,
    pub copy_feedback: Option<Instant>,
    pub provider_name: String,
    pub model_name: String,
    pub suggestion_active: bool,
    pub selected_suggestion: usize,
    pub input_height: Cell<u16>,
}

const SPINNER: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

impl<'a> Default for App<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> App<'a> {
    pub fn new() -> Self {
        let mut app = Self {
            textarea: TextArea::default(),
            messages: Vec::new(),
            is_processing: false,
            status_text: String::new(),
            spinner_idx: 0,
            should_quit: false,
            scroll_offset: 0,
            current_task_handle: None,
            pending_tools: Vec::new(),
            token_in: 0,
            token_out: 0,
            copy_feedback: None,
            provider_name: String::new(),
            model_name: String::new(),
            suggestion_active: false,
            selected_suggestion: 0,
            input_height: Cell::new(4),
        };
        app.configure_textarea();
        app
    }

    pub fn configure_textarea(&mut self) {
        self.textarea.set_block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Message "),
        );
        self.textarea.set_cursor_line_style(Style::default().add_modifier(Modifier::UNDERLINED));
        self.textarea.set_selection_style(Style::default().bg(Color::Indexed(24)).fg(Color::White));
        self.textarea.set_line_number_style(Style::default().fg(THEME.muted));
        self.textarea.set_style(Style::default().fg(THEME.primary));
        self.textarea.set_tab_length(4);
    }
}

const SLASH_COMMANDS: &[(&str, &str)] = &[
    ("/help", "Show help menu"),
    ("/clear", "Clear chat history"),
    ("/model", "Show model information"),
    ("/tokens", "Show token usage"),
    ("/export", "Export chat to Markdown"),
    ("/copy", "Copy last AI response"),
];

// ─── Slash command handler ──────────────────────────────────────────────────

/// Process a slash command. Returns `true` if the input was a command (handled).
fn handle_slash_command(input: &str, app: &mut App, provider_name: &str, model_name: &str) -> bool {
    let trimmed = input.trim();
    if !trimmed.starts_with('/') {
        return false;
    }

    let parts: Vec<&str> = trimmed.splitn(2, ' ').collect();
    let cmd = parts[0];
    let arg = parts.get(1).unwrap_or(&"").trim();

    match cmd {
        "/help" => {
            let help = concat!(
                "Available Commands:\n\n",
                "  /help           — Show this help\n",
                "  /clear          — Clear chat history\n",
                "  /model          — Show current model\n",
                "  /tokens         — Show token usage\n",
                "  /export [file]  — Export chat to file\n",
                "  /copy           — Copy last AI response\n",
            );
            app.messages.push(ChatMessage {
                role: "System".into(),
                content: help.to_string(),
                displayed_length: help.len(),
                is_fully_loaded: true,
                tool_entries: Vec::new(),
            });
        }
        "/clear" => {
            app.messages.clear();
            app.token_in = 0;
            app.token_out = 0;
        }
        "/model" => {
            let info = format!(
                "Current Model:\n  Provider: {}\n  Model:    {}",
                provider_name, model_name
            );
            app.messages.push(ChatMessage {
                role: "System".into(),
                content: info,
                displayed_length: 999,
                is_fully_loaded: true,
                tool_entries: Vec::new(),
            });
        }
        "/tokens" => {
            let info = format!(
                "Token Usage:\n  Input:  {} tokens\n  Output: {} tokens\n  Total:  {} tokens",
                app.token_in, app.token_out, app.token_in + app.token_out
            );
            app.messages.push(ChatMessage {
                role: "System".into(),
                content: info,
                displayed_length: 999,
                is_fully_loaded: true,
                tool_entries: Vec::new(),
            });
        }
        "/export" => {
            let filename = if arg.is_empty() {
                format!("zenclaw-chat-{}.md", chrono::Local::now().format("%Y%m%d-%H%M%S"))
            } else {
                arg.to_string()
            };
            let mut out = String::from("# ZenClaw Chat Export\n\n");
            for msg in &app.messages {
                out.push_str(&format!("## {}\n\n{}\n\n", msg.role, msg.content));
            }
            match std::fs::write(&filename, &out) {
                Ok(_) => {
                    app.messages.push(ChatMessage {
                        role: "System".into(),
                        content: format!("✅ Chat exported to: {}", filename),
                        displayed_length: 999,
                        is_fully_loaded: true,
                        tool_entries: Vec::new(),
                    });
                }
                Err(e) => {
                    app.messages.push(ChatMessage {
                        role: "System (Error)".into(),
                        content: format!("❌ Failed to export: {}", e),
                        displayed_length: 999,
                        is_fully_loaded: true,
                        tool_entries: Vec::new(),
                    });
                }
            }
        }
        "/copy" => {
            if let Some(last_ai) = app.messages.iter().rev().find(|m| m.role == "AI") {
                copy_to_clipboard(&last_ai.content);
                app.copy_feedback = Some(Instant::now());
            }
        }
        _ => {
            app.messages.push(ChatMessage {
                role: "System".into(),
                content: format!("Unknown command: {}. Type /help for available commands.", cmd),
                displayed_length: 999,
                is_fully_loaded: true,
                tool_entries: Vec::new(),
            });
        }
    }

    true
}

/// Copy text to system clipboard via platform tools.
fn copy_to_clipboard(text: &str) {
    let copied = std::process::Command::new("wl-copy")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .or_else(|_| std::process::Command::new("xclip")
            .args(["-selection", "clipboard"])
            .stdin(std::process::Stdio::piped())
            .spawn())
        .or_else(|_| std::process::Command::new("xsel")
            .args(["--clipboard", "--input"])
            .stdin(std::process::Stdio::piped())
            .spawn());
    if let Ok(mut child) = copied {
        if let Some(ref mut stdin) = child.stdin {
            use std::io::Write;
            let _ = stdin.write_all(text.as_bytes());
        }
        let _ = child.wait();
    }
}

// ─── Main event loop ────────────────────────────────────────────────────────

pub async fn run_tui(
    mut terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
    agent: std::sync::Arc<Agent>,
    provider: std::sync::Arc<dyn LlmProvider>,
    memory: std::sync::Arc<dyn MemoryStore>,
    session_key: String,
    bus: std::sync::Arc<EventBus>,
) -> anyhow::Result<()> {
    let mut app = App::new();
    app.provider_name = provider.name().to_string();
    app.model_name = provider.default_model().to_string();

    let provider_name = app.provider_name.clone();
    let model_name = app.model_name.clone();

    // Event channel
    let (tx, mut rx) = mpsc::channel(100);

    // Tick timer
    let tx_tick = tx.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(100)).await;
            if tx_tick.send(AppEvent::Tick).await.is_err() {
                break;
            }
        }
    });

    // Terminal input reader
    let tx_input = tx.clone();
    tokio::task::spawn_blocking(move || {
        loop {
            #[allow(clippy::collapsible_if)]
            if ratatui::crossterm::event::poll(Duration::from_millis(200)).unwrap_or(false) {
                if let Ok(event) = ratatui::crossterm::event::read() {
                    let _ = tx_input.blocking_send(AppEvent::Terminal(event));
                }
            }
        }
    });

    // Bus event bridge → tool use display
    let mut bus_rx = bus.subscribe_system();
    let tx_bus = tx.clone();
    tokio::spawn(async move {
        while let Ok(ev) = bus_rx.recv().await {
            match ev.event_type.as_str() {
                "tool_use" => {
                    let tool = ev.data["tool"].as_str().unwrap_or("tool").to_string();
                    let status = ev.format_status().unwrap_or_default();
                    let _ = tx_bus.send(AppEvent::ToolStart(tool, status)).await;
                }
                "tool_result" => {
                    let tool = ev.data["tool"].as_str().unwrap_or("tool").to_string();
                    let len = ev.data["result_len"].as_u64().unwrap_or(0);
                    let _ = tx_bus.send(AppEvent::ToolComplete(tool, len, Duration::from_millis(0))).await;
                }
                _ => {
                    if let Some(msg) = ev.format_status() {
                        let _ = tx_bus.send(AppEvent::AgentStatus(msg)).await;
                    }
                }
            }
        }
    });

    // ── Main UI Loop ────────────────────────────────────────
    while !app.should_quit {
        terminal.draw(|f| draw_ui(f, &app))?;

        if let Some(event) = rx.recv().await {
            match event {
                AppEvent::Tick => {
                    app.spinner_idx = (app.spinner_idx + 1) % SPINNER.len();

                    // Progressive streaming effect
                    #[allow(clippy::collapsible_if)]
                    if let Some(msg) = app.messages.last_mut() {
                        if !msg.is_fully_loaded {
                            msg.displayed_length += 20;
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
                AppEvent::ToolStart(name, context) => {
                    app.pending_tools.push(ToolEntry {
                        name,
                        context,
                        started: Instant::now(),
                        done: None,
                    });
                }
                AppEvent::ToolComplete(name, bytes, _dur) => {
                    if let Some(entry) = app.pending_tools.iter_mut().rev().find(|t| t.name == name && t.done.is_none()) {
                        let elapsed = entry.started.elapsed();
                        entry.done = Some((bytes, elapsed));
                    }
                }
                AppEvent::AgentResponse(msg, _blocks) => {
                    app.is_processing = false;
                    app.status_text.clear();

                    // Estimate tokens (rough: 1 token ≈ 4 chars)
                    app.token_out += (msg.len() as u64) / 4;

                    // Move pending tools into the response message
                    let tools = std::mem::take(&mut app.pending_tools);
                    app.messages.push(ChatMessage {
                        role: "AI".into(),
                        content: msg,
                        displayed_length: 0,
                        is_fully_loaded: false,
                        tool_entries: tools,
                    });
                }
                AppEvent::AgentError(msg) => {
                    app.is_processing = false;
                    app.status_text.clear();
                    let tools = std::mem::take(&mut app.pending_tools);
                    app.messages.push(ChatMessage {
                        role: "System (Error)".into(),
                        content: msg,
                        displayed_length: 0,
                        is_fully_loaded: false,
                        tool_entries: tools,
                    });
                }
                AppEvent::Terminal(Event::Paste(text)) => {
                    if !app.is_processing {
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
                    // Ctrl+C → cancel or quit
                    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                        if app.is_processing {
                            if let Some(handle) = app.current_task_handle.take() {
                                handle.abort();
                            }
                            app.is_processing = false;
                            app.status_text.clear();
                            app.pending_tools.clear();
                            app.messages.push(ChatMessage {
                                role: "System".into(),
                                content: "⚠️ Request cancelled by user.".into(),
                                displayed_length: 999,
                                is_fully_loaded: true,
                                tool_entries: Vec::new(),
                            });
                        } else {
                            app.should_quit = true;
                        }
                        continue;
                    }

                    // Ctrl+Y → copy last AI response
                    if key.code == KeyCode::Char('y') && key.modifiers.contains(KeyModifiers::CONTROL) {
                        if let Some(last_ai) = app.messages.iter().rev().find(|m| m.role == "AI") {
                            copy_to_clipboard(&last_ai.content);
                            app.copy_feedback = Some(Instant::now());
                        }
                        continue;
                    }

                    // Home/End → scroll to top/bottom
                    if key.code == KeyCode::Home {
                        app.scroll_offset = 9999;
                        continue;
                    }
                    if key.code == KeyCode::End {
                        app.scroll_offset = 0;
                        continue;
                    }

                    // jk or Up/Down scroll while NOT processing and textarea is empty
                    // This allows using jk for chat history when not typing
                    let is_empty = app.textarea.lines().iter().all(|l| l.is_empty());
                    if !app.is_processing && !app.suggestion_active && is_empty {
                        if key.code == KeyCode::Char('k') || key.code == KeyCode::Up {
                            app.scroll_offset = app.scroll_offset.saturating_add(1);
                            continue;
                        }
                        if key.code == KeyCode::Char('j') || key.code == KeyCode::Down {
                            app.scroll_offset = app.scroll_offset.saturating_sub(1);
                            continue;
                        }
                    }

                    // ── Suggestion handling ──
                    if !app.is_processing {
                        let lines = app.textarea.lines();
                        let (row, _) = app.textarea.cursor();
                        let current_line = lines.get(row).map(|s| s.as_str()).unwrap_or("");
                        
                        // Detect if we should be in suggestion mode
                        if current_line.starts_with('/') && !current_line.contains(' ') {
                            app.suggestion_active = true;
                            
                            // Filter suggestions
                            let filtered: Vec<_> = SLASH_COMMANDS.iter()
                                .filter(|(cmd, _)| cmd.starts_with(current_line))
                                .collect();

                            match key.code {
                                KeyCode::Tab | KeyCode::Enter if !filtered.is_empty() => {
                                    if app.selected_suggestion < filtered.len() {
                                        let (cmd, _) = filtered[app.selected_suggestion];
                                        app.textarea = TextArea::default();
                                        app.configure_textarea();
                                        app.textarea.insert_str(cmd);
                                        app.textarea.insert_str(" ");
                                        app.suggestion_active = false;
                                        app.selected_suggestion = 0;
                                        continue;
                                    }
                                }
                                KeyCode::Down | KeyCode::Tab => {
                                    if !filtered.is_empty() {
                                        app.selected_suggestion = (app.selected_suggestion + 1) % filtered.len();
                                        continue;
                                    }
                                }
                                KeyCode::Up => {
                                    if !filtered.is_empty() {
                                        app.selected_suggestion = app.selected_suggestion.checked_sub(1).unwrap_or(filtered.len() - 1);
                                        continue;
                                    }
                                }
                                KeyCode::Esc => {
                                    app.suggestion_active = false;
                                    app.selected_suggestion = 0;
                                    continue;
                                }
                                _ => {}
                            }
                        } else {
                            app.suggestion_active = false;
                            app.selected_suggestion = 0;
                        }
                    }

                    // Filter out release events if enhancement protocol is active
                    if key.kind == KeyEventKind::Release {
                        continue;
                    }

                    // ── Enter / Send Handling ──
                    let is_enter = matches!(key.code, KeyCode::Enter | KeyCode::Char('\n') | KeyCode::Char('\r'));
                    let has_ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                    let has_alt = key.modifiers.contains(KeyModifiers::ALT);

                    let is_ctrl_s = key.code == KeyCode::Char('s') && has_ctrl;

                    if is_enter || is_ctrl_s {
                        if has_ctrl || has_alt || is_ctrl_s {
                            // Ctrl+Enter, Alt+Enter, or Ctrl+S → Send
                            if !app.is_processing {
                                let text = app.textarea.lines().join("\n").trim().to_string();
                                if !text.is_empty() {
                                    // Submit
                                    app.textarea = TextArea::default();
                                    app.configure_textarea();
                                    app.scroll_offset = 0;

                                    if !handle_slash_command(&text, &mut app, &provider_name, &model_name) {
                                        app.token_in += (text.len() as u64) / 4;
                                        app.messages.push(ChatMessage {
                                            role: "You".into(),
                                            content: text.clone(),
                                            displayed_length: text.chars().count(),
                                            is_fully_loaded: true,
                                            tool_entries: Vec::new(),
                                        });

                                        app.is_processing = true;
                                        app.status_text = "🧠 Analyzing...".into();

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
                                    }
                                }
                            }
                        } else {
                            // Enter → New line + auto-indent
                            if !app.is_processing {
                                let lines = app.textarea.lines();
                                let (row, _) = app.textarea.cursor();
                                let current_line = lines.get(row).map(|s| s.as_str()).unwrap_or("");
                                let indent = current_line.chars().take_while(|c| c.is_whitespace()).collect::<String>();
                                
                                app.textarea.insert_newline();
                                app.textarea.insert_str(indent);
                            }
                        }
                        continue;
                    }

                    // Regular typing
                    if !app.is_processing {
                        app.textarea.input(Input::from(key));
                    }
                }
                AppEvent::Terminal(Event::Mouse(mouse)) => {
                    let size = terminal.size().unwrap_or_default();
                    let chat_height = size.height.saturating_sub(app.input_height.get() + 1);
                    
                    if mouse.row >= chat_height {
                        let mut rel_mouse = mouse;
                        rel_mouse.row = mouse.row.saturating_sub(chat_height + 1);
                        rel_mouse.column = mouse.column.saturating_sub(2);
                        app.textarea.input(Input::from(Event::Mouse(rel_mouse)));
                    } else {
                        match mouse.kind {
                            MouseEventKind::ScrollUp => { app.scroll_offset = app.scroll_offset.saturating_add(3); }
                            MouseEventKind::ScrollDown => { app.scroll_offset = app.scroll_offset.saturating_sub(3); } 
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
    }

    Ok(())
}

// ─── UI Rendering ───────────────────────────────────────────────────────────

fn draw_ui(f: &mut ratatui::Frame, app: &App) {
    let visual_input_lines = app.textarea.lines().len() as u16;

    let max_height = f.area().height.saturating_sub(8).max(4);
    let desired_height = visual_input_lines.saturating_add(2); // +2 for borders
    let actual_height = desired_height.min(max_height).clamp(4, 15);

    // Update state for event synchronization via Cell (interior mutability)
    app.input_height.set(actual_height);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),
            Constraint::Length(actual_height),
            Constraint::Length(1), // status bar
        ].as_ref())
        .split(f.area());

    let chat_width = chunks[0].width.saturating_sub(2) as usize;

    // ── Messages area ──────────────────────────────────
    let mut lines = Vec::new();

    if app.messages.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled("✨ Welcome to ZenClaw AI!", THEME.title())]));
        lines.push(Line::from("I'm ready to help you code, build, and debug. Type your request below."));
        lines.push(Line::from(vec![Span::styled(
            "Type /help for available commands.",
            THEME.hint(),
        )]));
        lines.push(Line::from(""));
    }

    for msg in &app.messages {
        let role_style = if msg.role == "You" {
            THEME.user_role()
        } else if msg.role == "AI" {
            THEME.ai_role()
        } else {
            THEME.system_role()
        };

        lines.push(Line::from(vec![
            Span::styled(format!("{} › ", msg.role), role_style),
        ]));

        // ── Tool entries (before AI response) ──────────
        for tool in &msg.tool_entries {
            let status = if let Some((bytes, dur)) = &tool.done {
                Span::styled(
                    format!("  ✅ {} — {} bytes, {:.1}s", tool.context, bytes, dur.as_secs_f64()),
                    THEME.ok(),
                )
            } else {
                Span::styled(
                    format!("  ⏳ {}", tool.context),
                    THEME.hint(),
                )
            };
            lines.push(Line::from(status));
        }

        // ── Message content ────────────────────────────
        let text_to_show = if msg.is_fully_loaded {
            msg.content.clone()
        } else {
            msg.content.chars().take(msg.displayed_length).collect()
        };

        if msg.role == "AI" {
            // Render AI messages as markdown
            let md_lines = crate::markdown::render_markdown(&text_to_show, chat_width);
            lines.extend(md_lines);
        } else {
            for line in text_to_show.lines() {
                lines.push(Line::from(line.to_string()));
            }
        }

        if msg.role != "System" && msg.is_fully_loaded {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled("─".repeat(50), THEME.hint())));
        }
        lines.push(Line::from(""));
    }

    // ── Pending tools (during processing) ──────────────
    if app.is_processing && !app.pending_tools.is_empty() {
        for tool in &app.pending_tools {
            let status = if let Some((bytes, dur)) = &tool.done {
                Line::from(Span::styled(
                    format!("  ✅ {} — {} bytes, {:.1}s", tool.context, bytes, dur.as_secs_f64()),
                    THEME.ok(),
                ))
            } else {
                Line::from(vec![
                    Span::styled(
                        format!("  {} ", SPINNER[app.spinner_idx]),
                        Style::default().fg(THEME.primary).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(tool.context.clone(), THEME.hint()),
                ])
            };
            lines.push(status);
        }
    }

    // ── Processing indicator ───────────────────────────
    if app.is_processing {
        lines.push(Line::from(vec![
            Span::styled(SPINNER[app.spinner_idx], Style::default().fg(THEME.primary).bold()),
            Span::raw(" "),
            Span::styled(&app.status_text, Style::default().fg(THEME.muted).italic()),
        ]));
    }

    // ── Auto-scroll computation ────────────────────────
    let chat_height = chunks[0].height.saturating_sub(2);
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
                .title(Span::styled(" ZenClaw AI Dashboard ", THEME.title()))
                .borders(Borders::ALL)
                .border_style(THEME.border_style())
        )
        .wrap(Wrap { trim: false })
        .scroll((view_offset, 0));

    f.render_widget(chat_block, chunks[0]);

    // ── Scrollbar ──
    if max_scroll > 0 {
        let mut sb_state = ScrollbarState::new(max_scroll as usize)
            .position(view_offset as usize);
        f.render_stateful_widget(
            Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓")),
            chunks[0],
            &mut sb_state,
        );
    }

    // ── Input area ─────────────────────────────────────
    let help_text = if app.is_processing {
        "[Ctrl+C] Cancel │ [Ctrl+↑↓] Scroll "
    } else {
        "[Ctrl+Enter/S] Send │ [Enter] New Line │ [Ctrl+C] Quit │ [Ctrl+Y] Copy "
    };

    let mut textarea = app.textarea.clone();
    textarea.set_block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(if app.is_processing { THEME.border_style() } else { THEME.border_focus_style() })
            .title(Span::styled(" Message ", Style::default().fg(Color::White).bold()))
            .title_bottom(Span::styled(help_text, THEME.hint()))
    );

    f.render_widget(&textarea, chunks[1]);

    // ── TextArea Scrollbar ──
    let text_lines = app.textarea.lines().len();
    let text_height = chunks[1].height.saturating_sub(2) as usize;
    if text_lines > text_height {
        let (cursor_row, _) = app.textarea.cursor();
        let mut sb_state = ScrollbarState::new(text_lines.saturating_sub(text_height))
            .position(cursor_row.min(text_lines.saturating_sub(text_height)));
        f.render_stateful_widget(
            Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .symbols(ratatui::symbols::scrollbar::VERTICAL),
            chunks[1],
            &mut sb_state,
        );
    }

    // ── Suggestions Popup ──
    if app.suggestion_active && !app.is_processing {
        let lines = app.textarea.lines();
        let (row, _) = app.textarea.cursor();
        let current_line = lines.get(row).map(|s| s.as_str()).unwrap_or("");
        
        let filtered: Vec<_> = SLASH_COMMANDS.iter()
            .filter(|(cmd, _)| cmd.starts_with(current_line))
            .collect();

        if !filtered.is_empty() {
            let popup_height = (filtered.len() as u16 + 2).min(12);
            let popup_width = 45.min(f.area().width.saturating_sub(4));
            
            let popup_area = ratatui::layout::Rect {
                x: chunks[1].x.saturating_add(2),
                y: chunks[1].y.saturating_sub(popup_height),
                width: popup_width,
                height: popup_height,
            };

            f.render_widget(ratatui::widgets::Clear, popup_area);
            
            let items: Vec<Line> = filtered.iter().enumerate().map(|(i, (cmd, desc))| {
                let is_sel = i == app.selected_suggestion;
                let cmd_style = if is_sel {
                    Style::default().bg(THEME.primary).fg(Color::Black).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(THEME.primary).add_modifier(Modifier::BOLD)
                };
                
                let row_style = if is_sel { Style::default().bg(THEME.bg_selected) } else { Style::default() };
                
                Line::from(vec![
                    Span::styled(format!(" {:<8} ", cmd), cmd_style),
                    Span::styled(format!(" {} ", desc), row_style.fg(THEME.muted)),
                ]).style(row_style)
            }).collect();

            let popup_block = Paragraph::new(items)
                .block(Block::default()
                    .borders(Borders::ALL)
                    .border_style(THEME.border_focus_style())
                    .title(Span::styled(" Suggestions ", THEME.title())));
            
            f.render_widget(popup_block, popup_area);
        }
    }

    // ── Status bar ─────────────────────────────────────
    let copy_indicator = if app.copy_feedback.map_or(false, |t| t.elapsed().as_secs() < 2) {
        Span::styled(" 📋 Copied! ", THEME.ok())
    } else {
        Span::raw("")
    };

    let token_info = format!(
        " {} │ {} │ Tokens: ↑{} ↓{} ",
        app.provider_name,
        app.model_name,
        app.token_in,
        app.token_out,
    );

    let status_bar = Paragraph::new(Line::from(vec![
        Span::styled(token_info, THEME.hint()),
        copy_indicator,
    ])).alignment(ratatui::layout::Alignment::Right);

    f.render_widget(status_bar, chunks[2]);
}
