use ratatui::{
    crossterm::event::{self, Event, KeyCode, KeyModifiers, MouseEventKind},
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap, Clear, Scrollbar, ScrollbarOrientation, ScrollbarState},
};
use std::io;

pub struct MenuItem {
    pub label: String,
    pub description: String,
    pub action_key: String,
}

pub fn run_tui_menu(title: &str, items: &[MenuItem], default_idx: usize) -> io::Result<Option<String>> {
    use crate::theme::THEME;
    use crate::tui_guard::TuiGuard;

    let mut guard = TuiGuard::new()?;

    let mut list_state = ListState::default();
    list_state.select(Some(default_idx));

    let mut selected_action = None;
    let mut filter_mode = false;
    let mut filter_query = String::new();

    loop {
        // Build filtered list
        let filtered: Vec<(usize, &MenuItem)> = if filter_query.is_empty() {
            items.iter().enumerate().collect()
        } else {
            let q = filter_query.to_lowercase();
            items.iter().enumerate()
                .filter(|(_, item)| item.label.to_lowercase().contains(&q) || item.action_key.to_lowercase().contains(&q))
                .collect()
        };
        if let Some(sel) = list_state.selected() {
            if sel >= filtered.len() {
                list_state.select(if filtered.is_empty() { None } else { Some(filtered.len() - 1) });
            }
        }

        guard.terminal.draw(|f| {
            f.render_widget(Clear, f.area());
            let size = f.area();
            
            // Header
            let main_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), 
                    Constraint::Min(2),
                    Constraint::Length(1)
                ].as_ref())
                .split(size);

            let version = env!("CARGO_PKG_VERSION");
            let header = Paragraph::new(Line::from(vec![
                Span::styled(title, Style::default().fg(THEME.primary).add_modifier(Modifier::BOLD)),
                Span::raw("  "),
                Span::styled(format!("v{}", version), Style::default().fg(THEME.muted)),
            ]))
                .block(Block::default().borders(Borders::ALL).border_style(THEME.border_style()))
                .alignment(ratatui::layout::Alignment::Center);
            f.render_widget(header, main_chunks[0]);

            // Responsive Body Layout
            let is_narrow = size.width < 100;
            let chunks = if is_narrow {
                Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .split(main_chunks[1])
            } else {
                Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
                    .split(main_chunks[1])
            };

            // Left — Menu with number shortcuts
            let list_items: Vec<ListItem> = filtered.iter().enumerate()
                .map(|(vi, (_, item))| {
                    let sc = if vi < 9 {
                        Span::styled(format!(" {} ", vi + 1), Style::default().fg(THEME.muted))
                    } else { Span::raw("   ") };
                    ListItem::new(Line::from(vec![sc, Span::raw(&item.label)]))
                })
                .collect();

            let list_title = if filter_mode {
                if filter_query.is_empty() { " 🔍 Type to filter... ".to_string() }
                else { format!(" 🔍 \"{}\" ({} results) ", filter_query, filtered.len()) }
            } else { " Select Option ".to_string() };

            let list = List::new(list_items)
                .block(Block::default()
                    .title(Span::styled(list_title, Style::default().fg(THEME.accent).bold()))
                    .borders(Borders::ALL)
                    .border_style(if filter_mode { THEME.border_focus_style() } else { Style::default().fg(THEME.info) }))
                .highlight_style(Style::default().bg(THEME.primary).fg(Color::Black).add_modifier(Modifier::BOLD))
                .highlight_symbol("▶ ");

            f.render_stateful_widget(list, chunks[0], &mut list_state);

            // Menu Scrollbar
            let mut sb_state = ScrollbarState::new(filtered.len().saturating_sub(chunks[0].height as usize))
                .position(list_state.selected().unwrap_or(0));
            f.render_stateful_widget(
                Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight),
                chunks[0], &mut sb_state,
            );

            // Right side - Description
            let sel_idx = list_state.selected().unwrap_or(0);
            let (desc_title, desc_text) = if let Some((_, item)) = filtered.get(sel_idx) {
                (format!(" About: {} ", item.label), format!("\n{}", item.description))
            } else {
                (" No matches ".to_string(), "\nTry a different search.".to_string())
            };

            let right = Paragraph::new(desc_text)
                .block(Block::default()
                    .title(Span::styled(desc_title, Style::default().fg(THEME.success).bold()))
                    .borders(Borders::ALL).border_style(THEME.border_style()))
                .wrap(Wrap { trim: true })
                .style(Style::default().fg(Color::White));
            f.render_widget(right, chunks[1]);
            
            // Footer
            let ft = if filter_mode {
                " [↑↓] Navigate  |  [Enter] Select  |  [Esc] Exit Filter  |  Type to search "
            } else {
                " [↑↓/jk] Navigate  |  [1-9] Quick  |  [/] Filter  |  [Enter] Select  |  [q/Esc] Close "
            };
            f.render_widget(
                Paragraph::new(Span::styled(ft, THEME.hint())).alignment(ratatui::layout::Alignment::Center),
                main_chunks[2],
            );
        })?;

        match event::read()? {
            Event::Key(key) => {
                if filter_mode {
                    match key.code {
                        KeyCode::Esc => { filter_mode = false; filter_query.clear(); list_state.select(Some(0)); }
                        KeyCode::Backspace => { filter_query.pop(); list_state.select(Some(0)); }
                        KeyCode::Enter => {
                            if let Some(sel) = list_state.selected() {
                                if let Some((_, item)) = filtered.get(sel) {
                                    selected_action = Some(item.action_key.clone());
                                }
                            }
                            break;
                        }
                        KeyCode::Down => {
                            let max = filtered.len().saturating_sub(1);
                            let i = list_state.selected().map_or(0, |i| if i >= max { 0 } else { i + 1 });
                            list_state.select(Some(i));
                        }
                        KeyCode::Up => {
                            let max = filtered.len().saturating_sub(1);
                            let i = list_state.selected().map_or(0, |i| if i == 0 { max } else { i - 1 });
                            list_state.select(Some(i));
                        }
                        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                            filter_query.push(c);
                            list_state.select(Some(0));
                        }
                        _ => {}
                    }
                } else {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                        KeyCode::Char('/') => { filter_mode = true; filter_query.clear(); list_state.select(Some(0)); }
                        KeyCode::Down | KeyCode::Char('j') => {
                            let max = filtered.len().saturating_sub(1);
                            let i = list_state.selected().map_or(0, |i| if i >= max { 0 } else { i + 1 });
                            list_state.select(Some(i));
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            let max = filtered.len().saturating_sub(1);
                            let i = list_state.selected().map_or(0, |i| if i == 0 { max } else { i - 1 });
                            list_state.select(Some(i));
                        }
                        KeyCode::Char(c @ '1'..='9') => {
                            let idx = c.to_digit(10).unwrap_or(1) as usize - 1;
                            if idx < filtered.len() {
                                selected_action = Some(filtered[idx].1.action_key.clone());
                                break;
                            }
                        }
                        KeyCode::Enter => {
                            if let Some(i) = list_state.selected() {
                                if let Some((_, item)) = filtered.get(i) {
                                    selected_action = Some(item.action_key.clone());
                                }
                            }
                            break;
                        }
                        _ => {}
                    }
                }
            }
            Event::Mouse(mouse) => {
                let max = filtered.len().saturating_sub(1);
                match mouse.kind {
                    MouseEventKind::ScrollUp => {
                        let i = list_state.selected().map_or(0, |i| if i == 0 { max } else { i - 1 });
                        list_state.select(Some(i));
                    }
                    MouseEventKind::ScrollDown => {
                        let i = list_state.selected().map_or(0, |i| if i >= max { 0 } else { i + 1 });
                        list_state.select(Some(i));
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    drop(guard);
    Ok(selected_action)
}

pub fn run_main_menu(has_config: bool) -> io::Result<Option<String>> {
    let mut items = vec![];
    
    if !has_config {
        items.push(MenuItem {
            label: "⚡ Setup Wizard".to_string(),
            description: "Initializes ZenClaw.\n\nSets up the default AI provider, model, and API keys so you can start using it immediately.".to_string(),
            action_key: "setup".to_string(),
        });
    }

    items.extend(vec![
        MenuItem {
            label: "💬 Chat (Interactive)".to_string(),
            description: "Launch the beautiful, interactive terminal interface.\n\nChat directly with ZenClaw AI to write code, debug, and get answers dynamically.".to_string(),
            action_key: "chat".to_string(),
        },
        MenuItem {
            label: "🔄 Switch AI Model".to_string(),
            description: "Quickly change between different AI providers and models.\n\nE.g. Switch from OpenAI GPT-4o to Anthropic Claude 3.5 Sonnet on the fly.".to_string(),
            action_key: "switch".to_string(),
        },
        MenuItem {
            label: "🤖 Start Telegram Bot".to_string(),
            description: "Run ZenClaw as a Telegram Bot.\n\nAllows you to interact with the AI assistant via Telegram.\n\nRequires a Telegram Bot Token.".to_string(),
            action_key: "telegram".to_string(),
        },
        MenuItem {
            label: "🎮 Start Discord Bot".to_string(),
            description: "Run ZenClaw as a Discord bot.\n\nBring AI magic to your Discord servers and let the community use its tools.\n\nRequires a Discord Bot Token.".to_string(),
            action_key: "discord".to_string(),
        },
        MenuItem {
            label: "📱 Start WhatsApp Bot".to_string(),
            description: "Run ZenClaw via WhatsApp using HTTP Bridge.\n\nTurns your phone into a powerful AI gateway.".to_string(),
            action_key: "whatsapp".to_string(),
        },
        MenuItem {
            label: "🌐 Start REST API Server".to_string(),
            description: "Start the ZenClaw REST API Server.\n\nServes an API that your own apps can consume to connect with the ZenClaw agent ecosystem over HTTP.".to_string(),
            action_key: "api".to_string(),
        },
        MenuItem {
            label: "📚 Manage Skills".to_string(),
            description: "View and manage available skills.\n\nSkills give ZenClaw special instructions and capabilities for specific domains (like coding, system diagnostics, writing, etc).".to_string(),
            action_key: "skills".to_string(),
        },
        MenuItem {
            label: "⚙️  Settings".to_string(),
            description: "View or modify configuration parameters.\n\nCheck the path to your settings file or re-configure default behavior.".to_string(),
            action_key: "settings".to_string(),
        },
        MenuItem {
            label: "🔄 Check for Updates".to_string(),
            description: "Check if a newer version of ZenClaw is available and pull updates to your current installation.".to_string(),
            action_key: "updates".to_string(),
            },
        MenuItem {
            label: "🐛 View Live Logs".to_string(),
            description: "Tail the ZenClaw internal diagnostic logs.\n\nGood for debugging connection issues, plugin errors, tools usages, and latency tracking.".to_string(),
            action_key: "logs".to_string(),
        },
        MenuItem {
            label: "❌ Exit".to_string(),
            description: "Quit ZenClaw CLI application.\n\nSee you next time! 🦀".to_string(),
            action_key: "exit".to_string(),
        },
    ]);

    run_tui_menu("✨ ZenClaw AI Dashboard ✨", &items, 0)
}

pub fn run_tui_text_viewer(title: &str, content: &str) -> io::Result<()> {
    use crate::theme::THEME;
    use crate::tui_guard::TuiGuard;

    let mut guard = TuiGuard::new()?;
    let lines: Vec<&str> = content.lines().collect();
    let mut scroll_offset = 0;

    loop {
        guard.terminal.draw(|f| {
            let area = f.area();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(3), Constraint::Length(if area.height > 10 { 1 } else { 0 })])
                .split(area);

            let paragraph = Paragraph::new(
                lines.iter().map(|l| Line::from(Span::raw(l.to_string()))).collect::<Vec<_>>()
            )
            .block(Block::default().borders(Borders::ALL).title(format!(" {} ", title)))
            .wrap(Wrap { trim: true })
            .scroll((scroll_offset, 0));

            f.render_widget(paragraph, chunks[0]);

            // Scrollbar rendering
            let mut scrollbar_state = ScrollbarState::new(lines.len().saturating_sub(chunks[0].height as usize))
                .position(scroll_offset as usize);
            let scrollbar = Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓"));
            
            f.render_stateful_widget(scrollbar, chunks[0], &mut scrollbar_state);

            if chunks[1].height > 0 {
                let footer = Paragraph::new(Span::styled(
                    " [↑↓] Scroll  |  [PgUp/PgDn] Page  |  [q/Esc/Enter] Close ",
                    THEME.hint()
                )).alignment(ratatui::layout::Alignment::Center);
                f.render_widget(footer, chunks[1]);
            }
        })?;

        match event::read()? {
            Event::Key(key) => {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc | KeyCode::Enter => break,
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                    KeyCode::Up => scroll_offset = scroll_offset.saturating_sub(1),
                    KeyCode::Down => {
                        let max_scroll = lines.len().saturating_sub(1);
                        if scroll_offset < max_scroll as u16 {
                            scroll_offset += 1;
                        }
                    }
                    KeyCode::PageUp => scroll_offset = scroll_offset.saturating_sub(20),
                    KeyCode::PageDown => {
                        let max_scroll = lines.len().saturating_sub(1) as u16;
                        scroll_offset = (scroll_offset + 20).min(max_scroll);
                    }
                    _ => {}
                }
            }
            Event::Mouse(mouse) => {
                let max_scroll = lines.len().saturating_sub(1) as u16;
                match mouse.kind {
                    MouseEventKind::ScrollUp => {
                        scroll_offset = scroll_offset.saturating_sub(3);
                    }
                    MouseEventKind::ScrollDown => {
                        scroll_offset = (scroll_offset + 3).min(max_scroll);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    drop(guard);
    Ok(())
}

pub fn run_tui_error(title: &str, message: &str) -> io::Result<()> {
    use crate::theme::THEME;
    use crate::tui_guard::TuiGuard;

    let mut guard = TuiGuard::new()?;

    loop {
        guard.terminal.draw(|f| {
            let size = f.area();
            let rect = centered_rect(60, 30, size);

            f.render_widget(Clear, rect);

            let block = Block::default()
                .title(Span::styled(format!(" ❌ {} ", title), THEME.err()))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(THEME.error));

            let p = Paragraph::new(format!("\n  {}\n\n  Press any key to return...", message))
                .block(block)
                .wrap(Wrap { trim: true })
                .alignment(ratatui::layout::Alignment::Center);
            
            f.render_widget(p, rect);

            let footer_hint = Paragraph::new(Span::styled(" [Any Key] Return ", THEME.hint()))
                .alignment(ratatui::layout::Alignment::Center);
            let footer_rect = ratatui::layout::Rect::new(rect.x, rect.y + rect.height - 1, rect.width, 1);
            f.render_widget(footer_hint, footer_rect);
        })?;

        if let Event::Key(_) = event::read()? {
            break;
        }
        // No scrollable content, so no mouse scroll handling needed.
    }

    drop(guard);
    Ok(())
}

fn centered_rect(percent_x: u16, percent_y: u16, r: ratatui::layout::Rect) -> ratatui::layout::Rect {
    // Adaptive sizing for small terminals
    let dynamic_x = if r.width < 100 { 95 } else if r.width < 150 { 80 } else { percent_x };
    let dynamic_y = if r.height < 30 { 90 } else if r.height < 50 { 50 } else { percent_y };

    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage((100 - dynamic_y) / 2),
                Constraint::Percentage(dynamic_y),
                Constraint::Percentage((100 - dynamic_y) / 2),
            ]
            .as_ref(),
        )
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage((100 - dynamic_x) / 2),
                Constraint::Percentage(dynamic_x),
                Constraint::Percentage((100 - dynamic_x) / 2),
            ]
            .as_ref(),
        )
        .split(popup_layout[1])[1]
}

pub fn run_tui_input(title: &str, prompt: &str, default: &str, hide_input: bool) -> io::Result<Option<String>> {
    use crate::theme::THEME;
    use crate::tui_guard::TuiGuard;

    let mut guard = TuiGuard::new()?;
    let mut input = default.to_string();

    loop {
        guard.terminal.draw(|f| {
            let size = f.area();
            let rect = centered_rect(60, 20, size);

            f.render_widget(Clear, rect);

            let display_input = if hide_input && !input.is_empty() {
                "*".repeat(input.len())
            } else {
                input.clone()
            };

            let block = Block::default()
                .title(Span::styled(format!(" {} ", title), THEME.title()))
                .borders(Borders::ALL)
                .border_style(THEME.border_focus_style());

            let p = Paragraph::new(format!("\n  {}\n\n  > {}_", prompt, display_input))
                .block(block)
                .wrap(Wrap { trim: false });

            f.render_widget(p, rect);

            let footer_hint = Paragraph::new(Span::styled(" [Enter] Confirm  |  [Esc] Cancel ", THEME.hint()))
                .alignment(ratatui::layout::Alignment::Center);
            let footer_rect = ratatui::layout::Rect::new(rect.x, rect.y + rect.height - 1, rect.width, 1);
            f.render_widget(footer_hint, footer_rect);
        })?;

        match event::read()? {
            Event::Key(key) => {
                match key.code {
                    KeyCode::Esc => { return Ok(None); }
                    KeyCode::Enter => { break; }
                    KeyCode::Backspace => { input.pop(); }
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => { return Ok(None); }
                    KeyCode::Char(c) => { input.push(c); }
                    _ => {}
                }
            }
            // No scrollable content, so no mouse scroll handling needed.
            _ => {}
        }
    }

    drop(guard);
    Ok(if input.is_empty() && default.is_empty() { None } else { Some(input) })
}
pub fn run_bot_dashboard(
    bot_name: &str,
    provider: &str,
    model: &str,
    details: &[(&str, &str)],
    mut log_rx: Option<tokio::sync::mpsc::Receiver<String>>,
) -> io::Result<()> {
    use crate::theme::THEME;
    use crate::tui_guard::TuiGuard;

    let mut guard = TuiGuard::new()?;

    let mut logs: Vec<String> = Vec::new();
    let mut log_scroll: usize = 0;
    let mut auto_scroll = true;

    // Strip ANSI codes from logs to prevent distorted rendering
    let ansi_re = regex::Regex::new(r"\x1B\[[0-9;]*[mK]").unwrap();

    loop {
        // Collect any new log lines
        let mut new_logs = false;
        if let Some(ref mut rx) = log_rx {
            while let Ok(line) = rx.try_recv() {
                logs.push(line);
                new_logs = true;
                if logs.len() > 500 { logs.remove(0); }
            }
        }

        if new_logs && auto_scroll {
            log_scroll = logs.len().saturating_sub(1);
        }

        guard.terminal.draw(|f| {
            let size = f.area();
            
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(10),
                    Constraint::Length(if size.height > 20 { 3 } else { 1 }),
                ])
                .split(size);

            // 1. Header (Responsive)
            let header_color = match bot_name.to_lowercase().as_str() {
                "telegram" => Color::Blue,
                "discord" => Color::Rgb(88, 101, 242),
                "whatsapp" => Color::Green,
                "rest api" => Color::Magenta,
                _ => THEME.primary,
            };

            let title = if size.width < 60 {
                format!("🤖 {}", bot_name)
            } else {
                format!(" 🤖 {} Bot Active (CTRL+C to Stop) ", bot_name)
            };

            let header = Paragraph::new(Line::from(vec![
                Span::styled(title, Style::default().fg(header_color).bold()),
            ]))
            .block(Block::default().borders(Borders::ALL).border_style(THEME.border_style()))
            .alignment(ratatui::layout::Alignment::Center);

            f.render_widget(header, chunks[0]);

            // 2. Info Panels - Responsive Layout (Stack vertically if narrow)
            let is_narrow = size.width < 100;
            let body_chunks = if is_narrow {
                Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(8), Constraint::Min(5)])
                    .split(chunks[1])
            } else {
                Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
                    .split(chunks[1])
            };

            // Left Side: Core Metrics
            let mut info_text = vec![
                Line::from(vec![Span::raw(" Status:   "), Span::styled("● Active", Style::default().fg(THEME.success))]),
                Line::from(vec![Span::raw(" Provider: "), Span::styled(provider, Style::default().fg(THEME.info))]),
                Line::from(vec![Span::raw(" Model:    "), Span::styled(model, Style::default().fg(THEME.accent))]),
                Line::from(""),
            ];

            if !is_narrow || size.height > 25 {
                info_text.push(Line::from(vec![Span::styled(" Configuration Details", Style::default().add_modifier(Modifier::UNDERLINED))]));
                for (k, v) in details {
                    info_text.push(Line::from(vec![
                        Span::raw(format!(" {}: ", k)),
                        Span::styled(*v, THEME.hint()),
                    ]));
                }
            }

            let left_panel = Paragraph::new(info_text)
                .block(Block::default().borders(Borders::ALL).title(" Metrics ").border_style(THEME.border_style()))
                .wrap(Wrap { trim: true });

            f.render_widget(left_panel, body_chunks[0]);

            // Right Side: Shortcuts or Live Logs
            if logs.is_empty() {
                let activity = vec![
                    Line::from(""),
                    Line::from(vec![Span::styled("  Keyboard Shortcuts", Style::default().fg(THEME.accent).bold())]),
                    Line::from("  ───────────────────────"),
                    Line::from(vec![Span::styled("  [q] / [Esc]", Style::default().fg(THEME.primary)), Span::raw("  Stop Bot & Exit")]),
                    Line::from(vec![Span::styled("  [r]", Style::default().fg(THEME.primary)), Span::raw("          Refresh (Manual)")]),
                    Line::from(""),
                    Line::from(vec![Span::styled("  Bot Insights", Style::default().fg(THEME.success).bold())]),
                    Line::from("  ───────────────────────"),
                    Line::from(vec![Span::raw("  • Bot is running in a background async task.")]),
                    Line::from(vec![Span::raw("  • SQLite memory.db will store chat history.")]),
                ];
                let right_panel = Paragraph::new(activity)
                    .block(Block::default().borders(Borders::ALL).title(" Dashboard ").border_style(THEME.border_style()))
                    .wrap(Wrap { trim: true });
                f.render_widget(right_panel, body_chunks[1]);
            } else {
                let log_count = logs.len();
                let viewport_height = body_chunks[1].height.saturating_sub(2) as usize;
                
                let display_offset = if auto_scroll {
                    log_count.saturating_sub(viewport_height) as u16
                } else {
                    log_scroll as u16
                };

                let log_lines: Vec<Line> = logs.iter()
                    .map(|l| {
                        let clean_line = ansi_re.replace_all(l, "").to_string();
                        Line::from(Span::raw(clean_line))
                    })
                    .collect();

                let paragraph = Paragraph::new(log_lines)
                    .block(Block::default().borders(Borders::ALL).title(" Live Logs / Activity ").border_style(THEME.border_focus_style()))
                    .wrap(Wrap { trim: false })
                    .scroll((display_offset, 0))
                    .alignment(ratatui::layout::Alignment::Left);
                
                f.render_widget(paragraph, body_chunks[1]);

                if log_count > viewport_height {
                    let mut scrollbar_state = ScrollbarState::new(log_count.saturating_sub(viewport_height))
                        .position(display_offset as usize);
                    f.render_stateful_widget(
                        Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight),
                        body_chunks[1], &mut scrollbar_state
                    );
                }
            }

            // 3. Footer
            let footer = Paragraph::new(Line::from(vec![
                Span::styled(format!("ZenClaw AI • v{} • ", env!("CARGO_PKG_VERSION")), THEME.hint()),
                if auto_scroll { 
                     Span::styled("● Auto-Scrolling", Style::default().fg(THEME.success))
                } else {
                     Span::styled("○ Static (Manual Scroll)", Style::default().fg(THEME.warning))
                },
                Span::styled("  |  [↑↓/jk] Scroll  |  [s] Auto-scroll  |  [q/Esc] Exit", THEME.hint()),
            ]))
            .alignment(ratatui::layout::Alignment::Center)
            .wrap(Wrap { trim: true })
            .block(Block::default().borders(Borders::TOP).border_style(THEME.border_style()));

            f.render_widget(footer, chunks[2]);
        })?;

        if event::poll(std::time::Duration::from_millis(100))? {
            let viewport_height = guard.terminal.size()?.height.saturating_sub(10) as usize;
            let max_possible_scroll = logs.len().saturating_sub(viewport_height);

            match event::read()? {
                Event::Key(key) => {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                        KeyCode::Char('s') => { auto_scroll = !auto_scroll; }
                        KeyCode::Up | KeyCode::Char('k') => {
                            auto_scroll = false;
                            log_scroll = log_scroll.saturating_sub(1);
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            auto_scroll = false;
                            if log_scroll < max_possible_scroll { log_scroll += 1; }
                        }
                        KeyCode::PageUp => {
                            auto_scroll = false;
                            log_scroll = log_scroll.saturating_sub(10);
                        }
                        KeyCode::PageDown => {
                            auto_scroll = false;
                            log_scroll = (log_scroll + 10).min(max_possible_scroll);
                        }
                        _ => {}
                    }
                }
                Event::Mouse(mouse) => {
                    match mouse.kind {
                        MouseEventKind::ScrollUp => {
                            auto_scroll = false;
                            log_scroll = log_scroll.saturating_sub(3);
                        }
                        MouseEventKind::ScrollDown => {
                            auto_scroll = false;
                            log_scroll = (log_scroll + 3).min(max_possible_scroll);
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    drop(guard);
    Ok(())
}


pub fn run_tui_skill_editor(
    skill_name: &str,
    skill_title: &str,
    skill_desc: &str,
    content: &str,
) -> io::Result<Option<(String, String, String)>> {
    use crate::theme::THEME;
    use crate::tui_guard::TuiGuard;
    use tui_textarea::{TextArea, Input, Key};

    let mut guard = TuiGuard::new()?;

    let mut title_input = skill_title.to_string();
    let mut desc_input = skill_desc.to_string();
    
    let mut textarea = TextArea::new(content.lines().map(|s| s.to_string()).collect());
    textarea.set_block(Block::default().borders(Borders::ALL).title(" Content (Markdown) "));
    textarea.set_cursor_line_style(Style::default().add_modifier(Modifier::UNDERLINED));

    let mut active_field = 2; // 0: Title, 1: Desc, 2: Content
    let mut result = None;

    loop {
        guard.terminal.draw(|f| {
            let size = f.area();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // Header
                    Constraint::Min(10),   // Editor Area
                    Constraint::Length(1), // Footer/Shortcuts
                ])
                .split(size);

            // Header
            let header = Paragraph::new(Line::from(vec![
                Span::styled(format!(" 📝 Editing Skill: {} ", skill_name), THEME.title()),
            ]))
            .block(Block::default().borders(Borders::ALL).border_style(THEME.border_style()))
            .alignment(ratatui::layout::Alignment::Center);
            f.render_widget(header, chunks[0]);

            // Body Layout (Responsive)
            let is_narrow = size.width < 100;
            let body_chunks = if is_narrow {
                Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(6), Constraint::Min(10)])
                    .split(chunks[1])
            } else {
                Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
                    .split(chunks[1])
            };

            // Sidebar (Title & Desc)
            let sidebar_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Length(3)])
                .split(body_chunks[0]);

            let title_block = Block::default()
                .borders(Borders::ALL)
                .title(" Title ")
                .border_style(if active_field == 0 { THEME.border_focus_style() } else { THEME.border_style() });
            let title_p = Paragraph::new(title_input.as_str()).block(title_block);
            f.render_widget(title_p, sidebar_chunks[0]);

            let desc_block = Block::default()
                .borders(Borders::ALL)
                .title(" Description ")
                .border_style(if active_field == 1 { THEME.border_focus_style() } else { THEME.border_style() });
            let desc_p = Paragraph::new(desc_input.as_str()).block(desc_block).wrap(Wrap { trim: true });
            f.render_widget(desc_p, sidebar_chunks[1]);

            // Main Editor (Textarea)
            let editor_style = if active_field == 2 { THEME.border_focus_style() } else { THEME.border_style() };
            textarea.set_block(Block::default().borders(Borders::ALL).title(" Content (Markdown) ").border_style(editor_style));
            
            f.render_widget(&textarea, body_chunks[1]);

            // Footer
            let footer_hint = Paragraph::new(Span::styled(
                " [Tab] Switch Field  |  [Ctrl+S] Save & Exit  |  [Esc] Cancel ",
                THEME.hint()
            )).alignment(ratatui::layout::Alignment::Center);
            f.render_widget(footer_hint, chunks[2]);
        })?;

        match event::read()?.into() {
            Input { key: Key::Esc, .. } => break,
            Input { key: Key::Char('s'), ctrl: true, .. } => {
                result = Some((title_input, desc_input, textarea.lines().join("\n")));
                break;
            }
            Input { key: Key::Tab, .. } => {
                active_field = (active_field + 1) % 3;
            }
            input => {
                // Manual mouse scroll for textarea if needed, but since we use Input from Event,
                // let's just handle scroll events if we can convert them.
                // However, tui-textarea doesn't have Mouse in Key enum.
                if active_field == 0 {
                    match input.key {
                        Key::Char(c) => title_input.push(c),
                        Key::Backspace => { title_input.pop(); },
                        _ => {}
                    }
                } else if active_field == 1 {
                    match input.key {
                        Key::Char(c) => desc_input.push(c),
                        Key::Backspace => { desc_input.pop(); },
                        _ => {}
                    }
                } else {
                    textarea.input(input);
                }
            }
        }
    }

    drop(guard);
    Ok(result)
}

/// Live log viewer TUI.
/// 
/// `initial_logs` — lines already loaded from file.
/// `rx`           — channel receiver for new live-tailed lines.
/// `file_label`   — display name shown in the header (e.g. filename).
pub fn run_tui_log_viewer(
    initial_logs: Vec<String>,
    rx: &mut std::sync::mpsc::Receiver<String>,
    file_label: &str,
) -> io::Result<()> {
    use crate::theme::THEME;
    use crate::tui_guard::TuiGuard;

    let mut guard = TuiGuard::new()?;

    let mut logs = initial_logs;
    let mut list_state = ListState::default();
    let mut auto_scroll = true;
    let mut search_mode = false;
    let mut search_query = String::new();
    let mut marked_lines: std::collections::HashSet<usize> = std::collections::HashSet::new();
    let mut copy_feedback: Option<std::time::Instant> = None;

    if !logs.is_empty() {
        list_state.select(Some(logs.len() - 1));
    }

    loop {
        // Drain new live-tailed lines
        while let Ok(line) = rx.try_recv() {
            logs.push(line);
        }
        if auto_scroll && !logs.is_empty() {
            list_state.select(Some(logs.len() - 1));
        }

        guard.terminal.draw(|f| {
            f.render_widget(Clear, f.area());
            let area = f.area();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(5),
                    Constraint::Length(if search_mode { 3 } else { 1 }),
                    Constraint::Length(1),
                ])
                .split(area);

            // ── Header ───────────────────────────────────────
            let log_count = logs.len();
            let selected_idx = list_state.selected().unwrap_or(0);
            
            let scroll_span = if auto_scroll {
                Span::styled(" ● AUTO ", THEME.ok().bold())
            } else {
                Span::styled(" ○ MANUAL ", Style::default().fg(THEME.warning).bold())
            };

            let search_span = if !search_query.is_empty() {
                Span::styled(format!(" 🔍 \"{}\" ", search_query), THEME.title())
            } else {
                Span::raw("")
            };

            let marked_span = if !marked_lines.is_empty() {
                Span::styled(format!(" ✓{} selected ", marked_lines.len()), Style::default().fg(THEME.success).bold())
            } else {
                Span::raw("")
            };

            let feedback_span = if copy_feedback.map_or(false, |t| t.elapsed().as_secs() < 2) {
                Span::styled(" 📋 Copied! ", THEME.ok().bold())
            } else {
                Span::raw("")
            };

            let header = Paragraph::new(Line::from(vec![
                Span::styled(" 🐛 Live Logs ", THEME.title()),
                Span::styled(format!("[{}/{}]", selected_idx + 1, log_count), THEME.hint()),
                Span::raw("  "),
                scroll_span,
                marked_span,
                feedback_span,
                search_span,
            ]))
            .block(Block::default().borders(Borders::ALL)
                .title(format!(" {} ", file_label))
                .border_style(THEME.border_style()));
            f.render_widget(header, chunks[0]);

            // ── Log list ─────────────────────────────────────
            let query_lower = search_query.to_lowercase();
            // Compute usable width
            let visible_width = chunks[1].width.saturating_sub(12) as usize; 

            let items: Vec<ListItem> = logs.iter().enumerate().map(|(i, line)| {
                let (fg_color, bold) = if line.contains("ERROR") {
                    (THEME.log_error, true)
                } else if line.contains("WARN") {
                    (THEME.log_warn, true)
                } else if line.contains(" INFO ") {
                    (THEME.log_info, false)
                } else if line.contains(" DEBUG ") {
                    (THEME.log_debug, false)
                } else if line.contains(" TRACE ") {
                    (THEME.log_trace, false)
                } else {
                    (THEME.muted, false)
                };

                let is_match = !query_lower.is_empty() && line.to_lowercase().contains(&query_lower);
                let is_selected = list_state.selected() == Some(i);
                let is_marked = marked_lines.contains(&i);

                let mut style = Style::default().fg(fg_color);
                if bold { style = style.add_modifier(Modifier::BOLD); }
                if is_match { style = style.bg(THEME.bg_selected).add_modifier(Modifier::BOLD); }
                if is_marked { style = style.bg(THEME.bg_marked); }

                let mark_char = if is_marked { "✓" } else { " " };
                let num_style = Style::default().fg(if is_marked { THEME.success } else if is_selected { THEME.primary } else { THEME.muted });
                let prefix = format!("{}{:>5} │ ", mark_char, i + 1);
                let prefix_len = prefix.chars().count();

                // Simple wrap
                let chars: Vec<char> = line.chars().collect();
                let mut lines_out: Vec<Line> = Vec::new();

                if visible_width > prefix_len && !chars.is_empty() {
                    let text_width = visible_width.saturating_sub(prefix_len);
                    let mut pos = 0;
                    let mut first = true;
                    while pos < chars.len() {
                        let w = text_width;
                        let end = (pos + w).min(chars.len());
                        let chunk: String = chars[pos..end].iter().collect();
                        if first {
                            lines_out.push(Line::from(vec![
                                Span::styled(prefix.clone(), num_style),
                                Span::styled(chunk, style),
                            ]));
                            first = false;
                        } else {
                            let pad = " ".repeat(prefix_len);
                            lines_out.push(Line::from(vec![
                                Span::styled(pad, num_style),
                                Span::styled(chunk, style),
                            ]));
                        }
                        pos = end;
                    }
                } else {
                    lines_out.push(Line::from(vec![
                        Span::styled(prefix, num_style),
                        Span::styled(line.clone(), style),
                    ]));
                }

                ListItem::new(lines_out)
            }).collect();


            let logs_list = List::new(items)
                .block(Block::default().borders(Borders::ALL).border_style(THEME.border_style()))
                .highlight_style(Style::default().bg(THEME.bg_selected).add_modifier(Modifier::BOLD))
                .highlight_symbol("▶ ");

            f.render_stateful_widget(logs_list, chunks[1], &mut list_state);

            // Scrollbar
            let sb_total = log_count.saturating_sub(chunks[1].height.saturating_sub(2) as usize);
            if sb_total > 0 {
                let mut sb_state = ScrollbarState::new(sb_total).position(selected_idx.min(sb_total));
                f.render_stateful_widget(
                    Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight),
                    chunks[1], &mut sb_state
                );
            }

            // ── Search bar / Status / Footer ─────────────────
            if search_mode {
                let search_bar = Paragraph::new(format!("Search: {}_", search_query))
                    .block(Block::default().borders(Borders::ALL)
                        .title(" 🔍 Filter Logs ")
                        .border_style(THEME.border_focus_style()));
                f.render_widget(search_bar, chunks[2]);
            } else {
                let match_count = if !query_lower.is_empty() {
                    logs.iter().filter(|l| l.to_lowercase().contains(&query_lower)).count()
                } else { 0 };

                let status_text = if !marked_lines.is_empty() {
                    format!(" ✓ {} line(s) selected  |  [y] Copy  [Space] Toggle  [Ctrl+A] Select All ", marked_lines.len())
                } else if !query_lower.is_empty() {
                    format!(" Filter: \"{}\" — {} match(es) ", search_query, match_count)
                } else {
                    format!(" {} total lines registered for {} ", log_count, file_label)
                };

                f.render_widget(
                    Paragraph::new(Span::styled(status_text, THEME.hint()))
                        .alignment(ratatui::layout::Alignment::Center),
                    chunks[2],
                );
                f.render_widget(
                    Paragraph::new(Span::styled(
                        " [↑↓/jk/PgUp/Dn] Scroll | [Space] Mark | [Ctrl+A] All | [y] Copy | [/] Search | [c] Clear | [s] Auto | [q] Exit ",
                        THEME.hint(),
                    )).alignment(ratatui::layout::Alignment::Center),
                    chunks[3],
                );
            }
        })?;

        if event::poll(std::time::Duration::from_millis(80))? {
            match event::read()? {
            Event::Key(key) => {
                if search_mode {
                    match key.code {
                        KeyCode::Esc | KeyCode::Enter => search_mode = false,
                        KeyCode::Char(c)              => { search_query.push(c); auto_scroll = false; }
                        KeyCode::Backspace            => { search_query.pop(); }
                        _                             => {}
                    }
                } else {
                    let max_idx = logs.len().saturating_sub(1);
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                        KeyCode::Char('s') => {
                            auto_scroll = !auto_scroll;
                            if auto_scroll { list_state.select(Some(max_idx)); }
                        }
                        KeyCode::Char('/') => { search_mode = true; auto_scroll = false; }
                        KeyCode::Char('c') => search_query.clear(),
                        KeyCode::Char(' ') => {
                            if let Some(idx) = list_state.selected() {
                                if marked_lines.contains(&idx) {
                                    marked_lines.remove(&idx);
                                } else {
                                    marked_lines.insert(idx);
                                }
                                let next = idx.saturating_add(1).min(max_idx);
                                list_state.select(Some(next));
                            }
                        }
                        KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            if marked_lines.len() == logs.len() {
                                marked_lines.clear();
                            } else {
                                marked_lines = (0..logs.len()).collect();
                            }
                        }
                        KeyCode::Char('y') => {
                            let lines_to_copy: Vec<String> = if marked_lines.is_empty() {
                                list_state.selected().map(|i| vec![logs[i].clone()]).unwrap_or_default()
                            } else {
                                let mut indices: Vec<usize> = marked_lines.iter().copied().collect();
                                indices.sort();
                                indices.iter().filter_map(|&i| logs.get(i).cloned()).collect()
                            };
                            if !lines_to_copy.is_empty() {
                                let text = lines_to_copy.join("\n");
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
                                copy_feedback = Some(std::time::Instant::now());
                                marked_lines.clear();
                            }
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            auto_scroll = false;
                            list_state.select(Some(list_state.selected().unwrap_or(max_idx).saturating_sub(1)));
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            let curr = list_state.selected().unwrap_or(0);
                            let next = curr.saturating_add(1).min(max_idx);
                            if next >= max_idx { auto_scroll = true; }
                            list_state.select(Some(next));
                        }
                        KeyCode::PageUp => {
                            auto_scroll = false;
                            list_state.select(Some(list_state.selected().unwrap_or(max_idx).saturating_sub(20)));
                        }
                        KeyCode::PageDown => {
                            let i = list_state.selected().unwrap_or(0).saturating_add(20).min(max_idx);
                            if i >= max_idx { auto_scroll = true; }
                            list_state.select(Some(i));
                        }
                        KeyCode::Home => {
                            auto_scroll = false;
                            list_state.select(Some(0));
                        }
                        KeyCode::End => {
                            auto_scroll = true;
                            list_state.select(Some(max_idx));
                        }
                        _ => {}
                    }
                }
            }
            Event::Mouse(mouse) => {
                let max_idx = logs.len().saturating_sub(1);
                match mouse.kind {
                    MouseEventKind::ScrollUp => {
                        auto_scroll = false;
                        list_state.select(Some(list_state.selected().unwrap_or(max_idx).saturating_sub(3)));
                    }
                    MouseEventKind::ScrollDown => {
                        let i = list_state.selected().unwrap_or(0).saturating_add(3).min(max_idx);
                        if i >= max_idx { auto_scroll = true; }
                        list_state.select(Some(i));
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    }

    drop(guard);
    println!();
    Ok(())
}
