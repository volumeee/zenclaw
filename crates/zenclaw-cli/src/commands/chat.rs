use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use colored::*;

use zenclaw_core::bus::EventBus;
use zenclaw_hub::skills::SkillManager;

use crate::{build_agent, setup_bot_env};

pub async fn run_chat(
    provider_name: Option<&str>,
    model: Option<&str>,
    api_key: Option<&str>,
    api_base: Option<&str>,
    active_skills: Vec<String>,
) -> anyhow::Result<()> {
    let skill_prompt = if active_skills.is_empty() {
        None
    } else {
        let data = crate::setup::data_dir();
        let mut skill_mgr = SkillManager::new(&data.join("skills"));
        let _ = skill_mgr.load_all().await;
        let prompt = skill_mgr.build_prompt(&active_skills);
        if prompt.is_empty() { None } else { Some(prompt) }
    };

    let (agent, provider, memory, provider_name, model) = setup_bot_env(
        provider_name,
        model,
        api_key,
        api_base,
        skill_prompt.as_deref()
    ).await?;

    crate::ui::print_session_info(&provider_name, &model, agent.tools.len(), &active_skills);

    let session_key = "cli:default";
    
    // Set up Alternate Screen & Raw Mode for TUI
    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    crossterm::execute!(
        stdout, 
        crossterm::terminal::EnterAlternateScreen, 
        crossterm::event::EnableMouseCapture,
        crossterm::event::EnableBracketedPaste
    )?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;

    let bus = std::sync::Arc::new(EventBus::new(32));

    // RUN THE TUI
    let res = crate::tui_app::run_tui(
        terminal,
        std::sync::Arc::new(agent),
        std::sync::Arc::new(provider),
        std::sync::Arc::new(memory),
        session_key.to_string(),
        bus,
    ).await;

    // Restore terminal exactly as before
    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        std::io::stdout(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture,
        crossterm::event::DisableBracketedPaste
    )?;

    if let Err(e) = res {
        eprintln!("{} {}", "UI Error:".red(), e);
    }

    Ok(())
}

pub async fn run_ask(
    provider_name: Option<&str>,
    model: Option<&str>,
    api_key: Option<&str>,
    message: &str,
) -> anyhow::Result<()> {
    let (provider_name, model, api_key, _api_base) =
        crate::resolve_config(provider_name, model, api_key, None)?;

    let provider = crate::create_provider(&provider_name, &api_key, &model, None);
    let memory = zenclaw_core::memory::InMemoryStore::new();
    let agent = build_agent(&model, None).await;

    // Create bus for streaming status to user
    let bus = EventBus::new(32);
    let mut rx = bus.subscribe_system();

    // Background task: print thinking/tool status to stderr
    let status_handle = tokio::spawn(async move {
        use std::io::Write;
        while let Ok(event) = rx.recv().await {
            if let Some(msg) = event.format_status() {
                // Print status on stderr with carriage return for overwrite effect
                eprint!("\r\x1b[2K\x1b[90m{}\x1b[0m", msg);
                let _ = std::io::stderr().flush();
            }
        }
    });

    match agent.process(&provider, &memory, message, "oneshot", Some(&bus)).await {
        Ok(response) => {
            // Clear the status line before printing response
            eprint!("\r\x1b[2K");
            println!("{}", response);
        }
        Err(e) => {
            eprint!("\r\x1b[2K");
            eprintln!("{}: {}", "Error".red(), e);
        }
    }

    // Cleanup
    status_handle.abort();

    Ok(())
}
