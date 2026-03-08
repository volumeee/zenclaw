use colored::Colorize;
use zenclaw_core::config::ZenClawConfig;
use zenclaw_hub::skills::SkillManager;
use crate::{setup, tui_menu, SkillAction};

pub async fn run_status() -> anyhow::Result<()> {
    let has_config = ZenClawConfig::default_path().exists();
    let config = setup::load_saved_config();

    let mut out = String::new();
    out.push_str(&format!("  ZenClaw v{}\n", env!("CARGO_PKG_VERSION")));
    out.push_str(&format!("  Data dir: {:?}\n", setup::data_dir()));
    out.push_str(&format!(
        "  Config: {} {}\n",
        ZenClawConfig::default_path().display(),
        if has_config { "✅" } else { "❌ (run `zenclaw setup`)" }
    ));

    if let Some(ref cfg) = config {
        out.push_str("\n  Current Settings:\n");
        out.push_str(&format!("    Provider: {}\n", cfg.provider.provider));
        out.push_str(&format!("    Model:    {}\n", cfg.provider.model));
        out.push_str(&format!(
            "    API Key:  {}\n",
            if cfg.provider.api_key.is_some() { "✅ configured" } else { "❌ not set" }
        ));
    }

    out.push_str("\n  Environment Variables:\n");
    for p in &["OPENAI_API_KEY", "GEMINI_API_KEY", "OPENROUTER_API_KEY", "ANTHROPIC_API_KEY"] {
        let status = if std::env::var(p).is_ok() { "✅" } else { "·" };
        out.push_str(&format!("    {} {}\n", status, p));
    }
    out.push_str("    🟡 Ollama (localhost:11434)\n");

    let data = setup::data_dir();
    let mut skill_mgr = SkillManager::new(&data.join("skills"));
    let skill_count = skill_mgr.load_all().await.unwrap_or(0);
    out.push_str(&format!(
        "\n  Skills: {} loaded from {}\n",
        skill_count,
        data.join("skills").display()
    ));

    tui_menu::run_tui_text_viewer("📊 System Status", &out).ok();
    Ok(())
}

pub async fn run_skills(action: Option<SkillAction>) -> anyhow::Result<()> {
    let data = setup::data_dir();
    let mut skill_mgr = SkillManager::new(&data.join("skills"));
    skill_mgr.load_all().await?;

    match action {
        Some(SkillAction::Show { name }) => {
            if let Some(skill) = skill_mgr.get(&name) {
                let content = format!(
                    "Skill: {}\nDescription: {}\nFile: {}\n\n{}",
                    skill.title, skill.description, skill.path.display(), skill.content
                );
                tui_menu::run_tui_text_viewer(&skill.title, &content).ok();
            } else {
                let available = skill_mgr.list().iter().map(|s| s.name.as_str()).collect::<Vec<_>>().join(", ");
                tui_menu::run_tui_error("Skill Not Found", &format!("Skill '{}' not found.\n\nAvailable: {}", name, available)).ok();
            }
        }
        _ => {
            loop {
                let mut items = vec![];
                items.push(tui_menu::MenuItem {
                    label: "➕ Create New Skill".to_string(),
                    description: "Create a new specialized AI behavior from scratch.".to_string(),
                    action_key: "create_new".to_string(),
                });

                for skill in skill_mgr.list() {
                    items.push(tui_menu::MenuItem {
                        label: format!("{} {}", "•".cyan(), skill.name),
                        description: format!("Skill: {}\n\n{}\n\nFile: {}", skill.title, skill.description, skill.path.display()),
                        action_key: skill.name.clone(),
                    });
                }
                items.push(tui_menu::MenuItem {
                    label: "❌ Back".to_string(),
                    description: "Return to previous menu.".to_string(),
                    action_key: "back".to_string(),
                });

                if let Ok(Some(action_key)) = tui_menu::run_tui_menu("📚 Manage Skills", &items, 0) {
                    if action_key == "back" {
                        break;
                    }

                    if action_key == "create_new" {
                        let name_input = tui_menu::run_tui_input("New Skill", "Enter internal name (id):", "", false)?;
                        if let Some(name) = name_input 
                            && !name.trim().is_empty()
                            && let Ok(Some((t, d, c))) = tui_menu::run_tui_skill_editor(&name, &name, "", "") 
                        {
                            skill_mgr.save_skill(&name, &t, &d, &c).await?;
                        }
                        continue;
                    }

                    // Clone skill data to release immutable borrow on skill_mgr
                    let skill_data = skill_mgr.get(&action_key).map(|s| {
                        (s.name.clone(), s.title.clone(), s.description.clone(), s.content.clone(), s.path.display().to_string())
                    });

                    if let Some((s_name, s_title, s_desc, s_content, s_path)) = skill_data {
                        loop {
                            let skill_options = vec![
                                tui_menu::MenuItem { label: "📄 View Content".into(), description: "Read the skill markdown content.".into(), action_key: "view".into() },
                                tui_menu::MenuItem { label: "📝 Edit Skill".into(), description: "Modify title, description, or content.".into(), action_key: "edit".into() },
                                tui_menu::MenuItem { label: "🗑️  Delete Skill".into(), description: "Permanently remove this skill from disk.".into(), action_key: "delete".into() },
                                tui_menu::MenuItem { label: "⬅️  Back".into(), description: "Return to skills list.".into(), action_key: "back".into() },
                            ];
                            let skill_sel = tui_menu::run_tui_menu(&format!("Manage: {}", s_name), &skill_options, 0)?;
                            
                            match skill_sel.as_deref() {
                                Some("view") => {
                                    let content = format!("Skill: {}\nDescription: {}\nFile: {}\n\n{}\n", s_title, s_desc, s_path, s_content);
                                    tui_menu::run_tui_text_viewer(&s_title, &content).ok();
                                },
                                Some("edit") => {
                                    if let Ok(Some((t, d, c))) = tui_menu::run_tui_skill_editor(&s_name, &s_title, &s_desc, &s_content) {
                                        skill_mgr.save_skill(&s_name, &t, &d, &c).await?;
                                        break;
                                    }
                                },
                                Some("delete") => {
                                    let confirm = tui_menu::run_tui_input("Confirm Delete", &format!("Delete '{}'? Type 'yes' to confirm:", s_name), "", false)?;
                                    if confirm.as_deref() == Some("yes") {
                                        skill_mgr.delete_skill(&s_name).await?;
                                        break;
                                    }
                                },
                                _ => break,
                            }
                        }
                    }
                } else {
                    break;
                }
            }
        }
    }

    Ok(())
}

pub async fn run_update_check() -> anyhow::Result<()> {
    match zenclaw_hub::updater::check_for_updates().await {
        Ok(Some(info)) => {
            let mut out = String::new();
            out.push_str("  🆕 New version available!\n\n");
            out.push_str(&format!("  Current: v{}\n", info.current));
            out.push_str(&format!("  Latest:  v{}\n", info.latest));
            out.push_str(&format!("  URL:     {}\n", info.url));

            if !info.changelog.is_empty() {
                let preview = if info.changelog.len() > 500 {
                    format!("{}...", &info.changelog[..500])
                } else {
                    info.changelog.clone()
                };
                out.push_str("\n  Changelog:\n");
                for line in preview.lines().take(15) {
                    out.push_str(&format!("    {}\n", line));
                }
            }

            let install_cmd = match std::env::consts::OS {
                "windows" => format!(
                    "Invoke-WebRequest -Uri https://github.com/volumeee/zenclaw/releases/download/v{}/zenclaw-windows-amd64.exe -OutFile zenclaw.exe",
                    info.latest
                ),
                "macos" => format!(
                    "curl -L https://github.com/volumeee/zenclaw/releases/download/v{}/zenclaw-macos-$(uname -m).tar.gz | tar -xz && sudo mv zenclaw /usr/local/bin/zenclaw",
                    info.latest
                ),
                _ => format!(
                    "wget -qO- https://github.com/volumeee/zenclaw/releases/download/v{}/zenclaw-linux-$(uname -m).tar.gz | tar -xz && sudo mv zenclaw /usr/local/bin/zenclaw",
                    info.latest
                ),
            };

            out.push_str(&format!("\n  To update, run:\n  {}\n", install_cmd));
            tui_menu::run_tui_text_viewer("🔄 Update Available", &out).ok();
        }
        Ok(None) => {
            let msg = format!("✅ You're on the latest version! (v{})", env!("CARGO_PKG_VERSION"));
            tui_menu::run_tui_text_viewer("🔄 Update Check", &msg).ok();
        }
        Err(e) => {
            tui_menu::run_tui_error("Update Check Failed", &format!("Unable to check for updates:\n{}", e)).ok();
        }
    }
    Ok(())
}

pub async fn run_logs(initial_lines: usize) -> anyhow::Result<()> {
    use tokio::io::{AsyncBufReadExt, AsyncSeekExt, BufReader};
    use tokio::fs::File;
    use std::sync::mpsc;
    use std::time::Duration;

    let log_dir = setup::data_dir().join("logs");

    let log_file = std::fs::read_dir(&log_dir)
        .ok()
        .and_then(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.file_name().to_string_lossy().starts_with("zenclaw.log.")
                })
                .max_by_key(|e| e.metadata().ok().and_then(|m| m.modified().ok()))
                .map(|e| e.path())
        });

    let log_file = match log_file {
        Some(f) => f,
        None => {
            tui_menu::run_tui_error(
                "Log File Not Found",
                &format!("No log files in:\n{}\n\nRun the app first to generate logs.", log_dir.display()),
            ).ok();
            return Ok(());
        }
    };

    let (tx, mut rx) = mpsc::channel::<String>();

    let mut initial_logs: Vec<String> = Vec::new();
    if let Ok(content) = std::fs::read_to_string(&log_file) {
        let lines: Vec<&str> = content.lines().collect();
        let start = lines.len().saturating_sub(initial_lines);
        for line in lines.into_iter().skip(start) {
            initial_logs.push(line.to_string());
        }
    }

    let log_file_clone = log_file.clone();
    let tail_handle = tokio::spawn(async move {
        if let Ok(file) = File::open(&log_file_clone).await 
            && let Ok(metadata) = file.metadata().await 
        {
            let mut reader = BufReader::new(file);
            let _ = reader.seek(std::io::SeekFrom::Start(metadata.len())).await;
            let mut buf = String::new();
            loop {
                buf.clear();
                match reader.read_line(&mut buf).await {
                    Ok(0) => tokio::time::sleep(Duration::from_millis(200)).await,
                    Ok(_) => {
                        let line = buf.trim_end().to_string();
                        if !line.is_empty() && tx.send(line).is_err() {
                            break;
                        }
                    }
                    Err(_) => tokio::time::sleep(Duration::from_millis(200)).await,
                }
            }
        }
    });

    let file_label = log_file
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("zenclaw.log");

    tui_menu::run_tui_log_viewer(initial_logs, &mut rx, file_label).ok();

    tail_handle.abort();
    Ok(())
}

pub async fn run_maintenance(retention_days: u32) -> anyhow::Result<()> {
    println!("{}", "🧹 Running maintenance...".cyan());
    
    let data = setup::data_dir();
    let db_path = data.join("memory.db");
    
    if !db_path.exists() {
        println!("  No memory database found at {}. Skipping history pruning.", db_path.display());
        return Ok(());
    }

    match zenclaw_hub::memory::SqliteMemory::open(&db_path) {
        Ok(mem) => {
            match mem.prune_history(retention_days).await {
                Ok(cleaned) => {
                    println!("  ✅ Pruned {} messages older than {} days.", cleaned, retention_days);
                }
                Err(e) => {
                    println!("  ❌ Failed to prune history: {}", e);
                }
            }
        }
        Err(e) => {
            println!("  ❌ Could not open memory database: {}", e);
        }
    }
    
    println!("{}", "✨ Maintenance complete!".green());
    Ok(())
}

