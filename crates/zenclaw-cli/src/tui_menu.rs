use ratatui::{
    backend::CrosstermBackend,
    crossterm::{
        event::{self, Event, KeyCode, KeyModifiers},
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
        execute,
    },
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Terminal,
};
use std::io;

pub struct MenuItem {
    pub label: String,
    pub description: String,
    pub action_key: String,
}

pub fn run_tui_menu(title: &str, items: &[MenuItem], default_idx: usize) -> io::Result<Option<String>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut list_state = ListState::default();
    list_state.select(Some(default_idx));

    let mut selected_action = None;

    loop {
        terminal.draw(|f| {
            let size = f.area();
            
            // Header
            let main_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), 
                    Constraint::Min(10),
                    Constraint::Length(1)
                ].as_ref())
                .split(size);

            let header = Paragraph::new(Span::styled(title, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)))
                .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)))
                .alignment(ratatui::layout::Alignment::Center);
            
            f.render_widget(header, main_chunks[0]);

            // Body
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(40), Constraint::Percentage(60)].as_ref())
                .split(main_chunks[1]);

            // Left side - List
            let list_items: Vec<ListItem> = items
                .iter()
                .map(|i| {
                    ListItem::new(Line::from(vec![Span::raw(" "), Span::raw(&i.label)]))
                })
                .collect();

            let list = List::new(list_items)
                .block(Block::default().title(Span::styled(format!(" {} ", "Select Option"), Style::default().fg(Color::Yellow).bold())).borders(Borders::ALL).border_style(Style::default().fg(Color::Blue)))
                .highlight_style(Style::default().bg(Color::Cyan).fg(Color::Black).add_modifier(Modifier::BOLD))
                .highlight_symbol("▶ ");

            f.render_stateful_widget(list, chunks[0], &mut list_state);

            // Right side - Description
            let selected_idx = list_state.selected().unwrap_or(0);
            let (desc_title, description_text) = if let Some(item) = items.get(selected_idx) {
                (format!(" About: {} ", item.label), format!("\n{}", item.description))
            } else {
                (" Description ".to_string(), "".to_string())
            };

            let right_panel = Paragraph::new(description_text)
                .block(
                    Block::default()
                        .title(Span::styled(desc_title, Style::default().fg(Color::Green).bold()))
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::DarkGray))
                )
                .wrap(Wrap { trim: true })
                .style(Style::default().fg(Color::White));

            f.render_widget(right_panel, chunks[1]);
            
            // Footer
            let footer = Paragraph::new(Span::styled("[Up/Down] Navigate  |  [Enter] Select  |  [Esc] Quit", Style::default().fg(Color::DarkGray)))
                .alignment(ratatui::layout::Alignment::Center);
            f.render_widget(footer, main_chunks[2]);
        })?;

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => {
                    break;
                }
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    break;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let i = match list_state.selected() {
                        Some(i) => {
                            if i >= items.len() - 1 {
                                0
                            } else {
                                i + 1
                            }
                        }
                        None => 0,
                    };
                    list_state.select(Some(i));
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    let i = match list_state.selected() {
                        Some(i) => {
                            if i == 0 {
                                items.len() - 1
                            } else {
                                i - 1
                            }
                        }
                        None => 0,
                    };
                    list_state.select(Some(i));
                }
                KeyCode::Enter => {
                    if let Some(i) = list_state.selected() {
                        selected_action = Some(items[i].action_key.clone());
                    }
                    break;
                }
                _ => {}
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

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
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let lines: Vec<&str> = content.lines().collect();
    let mut scroll_offset = 0;

    loop {
        terminal.draw(|f| {
            let paragraph = Paragraph::new(
                lines.iter().map(|l| Line::from(Span::raw(l.to_string()))).collect::<Vec<_>>()
            )
            .block(Block::default().borders(Borders::ALL).title(format!(" {} ", title)))
            .scroll((scroll_offset, 0));

            f.render_widget(paragraph, f.area());
        })?;

        if let Event::Key(key) = event::read()? {
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
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}
