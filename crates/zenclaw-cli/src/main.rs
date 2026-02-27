//! ZenClaw CLI — Build AI the simple way.
//!
//! Beautiful, interactive terminal interface for the ZenClaw AI agent.
//! Run `zenclaw setup` to get started!

mod setup;
mod ui;
pub mod tui_app;
pub mod tui_menu;
pub mod theme;
pub mod tui_guard;
pub mod markdown;

use std::io::{self, Write};
use std::sync::Arc;

use clap::{Parser, Subcommand};
use colored::*;
use tracing_subscriber::EnvFilter;

use zenclaw_core::agent::{Agent, AgentConfig};
use zenclaw_core::bus::EventBus;
use zenclaw_core::config::ZenClawConfig;
use zenclaw_core::provider::ProviderConfig;
use zenclaw_hub::channels::{DiscordConfig, TelegramConfig};
use zenclaw_hub::memory::SqliteMemory;
use zenclaw_hub::providers::OpenAiProvider;
use zenclaw_hub::skills::SkillManager;
use zenclaw_hub::plugins::PluginManager;
use zenclaw_hub::tools::{
    CodebaseSearchTool, CronTool, EditFileTool, EnvTool, HealthTool, HistoryTool, ListDirTool, ProcessTool,
    ReadFileTool, ShellTool, SubAgentTool, SystemInfoTool, WebFetchTool, WebScrapeTool, WebSearchTool, WriteFileTool,
};

// ─── CLI Definition ────────────────────────────────────────

/// ZenClaw — Build AI the simple way 🦀⚡
#[derive(Parser)]
#[command(name = "zenclaw", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// ⚡ Interactive setup wizard — configure provider, API key, model
    Setup,

    /// 💬 Start interactive chat with the agent
    Chat {
        /// Model to use (overrides config)
        #[arg(short, long)]
        model: Option<String>,

        /// Provider (overrides config)
        #[arg(short, long)]
        provider: Option<String>,

        /// API key (overrides config)
        #[arg(short = 'k', long)]
        api_key: Option<String>,

        /// API base URL override
        #[arg(long)]
        api_base: Option<String>,

        /// Activate a skill (e.g. --skill coding)
        #[arg(short, long)]
        skill: Option<Vec<String>>,
    },

    /// ❓ Send a single message and get a response
    Ask {
        /// The message to send
        message: String,

        /// Model to use
        #[arg(short, long)]
        model: Option<String>,

        /// Provider
        #[arg(short, long)]
        provider: Option<String>,

        /// API key
        #[arg(short = 'k', long)]
        api_key: Option<String>,
    },

    /// ⚙️  Manage configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// 📊 Show system info and status
    Status,

    /// 🤖 Start Telegram bot
    Telegram {
        /// Telegram bot token (or use config)
        #[arg(short, long, env = "TELEGRAM_BOT_TOKEN")]
        token: Option<String>,

        /// Model to use
        #[arg(short, long)]
        model: Option<String>,

        /// Provider
        #[arg(short, long)]
        provider: Option<String>,

        /// API key
        #[arg(short = 'k', long)]
        api_key: Option<String>,

        /// Allowed Telegram user IDs (comma-separated)
        #[arg(long)]
        allowed_users: Option<String>,
    },

    /// 🎮 Start Discord bot
    Discord {
        /// Discord bot token (or use config)
        #[arg(short, long, env = "DISCORD_BOT_TOKEN")]
        token: Option<String>,

        /// Model to use
        #[arg(short, long)]
        model: Option<String>,

        /// Provider
        #[arg(short, long)]
        provider: Option<String>,

        /// API key
        #[arg(short = 'k', long)]
        api_key: Option<String>,
    },

    /// 👔 Start Slack bot
    Slack {
        /// Slack bot token (xoxb-...)
        #[arg(short, long, env = "SLACK_BOT_TOKEN")]
        token: Option<String>,

        /// Model to use
        #[arg(short, long)]
        model: Option<String>,

        /// Provider
        #[arg(short, long)]
        provider: Option<String>,

        /// API key
        #[arg(short = 'k', long)]
        api_key: Option<String>,

        /// Allowed Slack channel IDs (comma-separated)
        #[arg(long)]
        allowed_channels: Option<String>,
    },

    /// 📚 List and manage skills
    Skills {
        #[command(subcommand)]
        action: Option<SkillAction>,
    },

    /// 🌐 Start REST API server
    Serve {
        /// Host to bind to
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        /// Port to listen on
        #[arg(long, default_value_t = 3000)]
        port: u16,

        /// Provider
        #[arg(short, long)]
        provider: Option<String>,

        /// Model to use
        #[arg(short, long)]
        model: Option<String>,

        /// API key
        #[arg(short = 'k', long)]
        api_key: Option<String>,
    },

    /// 📱 Start WhatsApp bot (via HTTP bridge)
    Whatsapp {
        /// Bridge URL (e.g. http://localhost:3001)
        #[arg(short, long, default_value = "http://localhost:3001")]
        bridge: String,

        /// Provider
        #[arg(short, long)]
        provider: Option<String>,

        /// Model to use
        #[arg(short, long)]
        model: Option<String>,

        /// API key
        #[arg(short = 'k', long)]
        api_key: Option<String>,

        /// Allowed phone numbers (comma-separated)
        #[arg(long)]
        allowed_numbers: Option<String>,
    },

    /// 🔄 Check for updates
    Update,

    /// 🐛 Monitor ZenClaw internal diagnostic logs
    Logs {
        /// Number of tail lines to show initially
        #[arg(short, long, default_value_t = 50)]
        lines: usize,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Show current configuration
    Show,
    /// Set a configuration value
    Set {
        /// Key to set (e.g. provider, model, api_key, telegram_token)
        key: String,
        /// Value to set
        value: String,
    },
    /// Open config file location
    Path,
}

#[derive(Subcommand)]
enum SkillAction {
    /// List available skills
    List,
    /// Show a skill's content
    Show { name: String },
}

// ─── Helpers ───────────────────────────────────────────────



fn resolve_api_key(provided: Option<&str>, provider: &str) -> Option<String> {
    if let Some(key) = provided {
        return Some(key.to_string());
    }

    let env_vars = match provider {
        "openai" => vec!["OPENAI_API_KEY"],
        "openrouter" => vec!["OPENROUTER_API_KEY"],
        "groq" => vec!["GROQ_API_KEY"],
        "gemini" => vec!["GEMINI_API_KEY", "GOOGLE_API_KEY"],

        "anthropic" => vec!["ANTHROPIC_API_KEY"],
        "ollama" | "lmstudio" => return Some("local".to_string()),
        _ => vec![],
    };

    for var in env_vars {
        if let Ok(val) = std::env::var(var)
            && !val.is_empty()
        {
            return Some(val);
        }
    }

    None
}

fn default_model(provider: &str) -> &str {
    match provider {
        "openai" => "gpt-4o-mini",
        "openrouter" => "openai/gpt-4o-mini",
        "groq" => "llama-3.3-70b-versatile",
        "gemini" => "gemini-2.5-flash",

        "anthropic" => "claude-3-5-sonnet-20241022",
        "ollama" => "llama3.2",
        "lmstudio" => "local-model",
        _ => "gpt-4o-mini",
    }
}

fn create_provider(
    provider_name: &str,
    api_key: &str,
    model: &str,
    api_base: Option<&str>,
) -> OpenAiProvider {
    match provider_name {
        "ollama" => OpenAiProvider::ollama(model),
        "openrouter" => OpenAiProvider::openrouter(api_key, model),
        "gemini" => OpenAiProvider::gemini(api_key, model),
        "groq" => OpenAiProvider::groq(api_key, model),
        _ => {

            if let Some(base) = api_base {
                OpenAiProvider::new(ProviderConfig {
                    provider: provider_name.to_string(),
                    model: model.to_string(),
                    api_key: Some(api_key.to_string()),
                    api_base: Some(base.to_string()),
                    ..Default::default()
                })
            } else {
                OpenAiProvider::openai(api_key, model)
            }
        }
    }
}

async fn setup_bot_env(
    provider_name: Option<&str>,
    model: Option<&str>,
    api_key: Option<&str>,
    api_base: Option<&str>,
    skill_prompt: Option<&str>,
) -> anyhow::Result<(Agent, OpenAiProvider, SqliteMemory, String, String)> {
    let (resolved_provider_name, resolved_model, resolved_api_key, resolved_api_base) =
        resolve_config(provider_name, model, api_key, api_base)?;

    let provider = create_provider(
        &resolved_provider_name,
        &resolved_api_key,
        &resolved_model,
        resolved_api_base.as_deref(),
    );

    let data = setup::data_dir();
    std::fs::create_dir_all(&data)?;

    let db_path = data.join("memory.db");
    let memory = SqliteMemory::open(&db_path)?;

    let agent = build_agent(&resolved_model, skill_prompt).await;

    Ok((
        agent,
        provider,
        memory,
        resolved_provider_name,
        resolved_model,
    ))
}

/// Resolve provider config: CLI args → saved config → env vars → error
fn resolve_config(
    cli_provider: Option<&str>,
    cli_model: Option<&str>,
    cli_api_key: Option<&str>,
    cli_api_base: Option<&str>,
) -> anyhow::Result<(String, String, String, Option<String>)> {
    let saved = setup::load_saved_config();

    let provider_name = cli_provider
        .map(|s| s.to_string())
        .or_else(|| saved.as_ref().map(|c| c.provider.provider.clone()))
        .unwrap_or_else(|| "openai".to_string());

    let model = cli_model
        .map(|s| s.to_string())
        .or_else(|| saved.as_ref().map(|c| c.provider.model.clone()))
        .unwrap_or_else(|| default_model(&provider_name).to_string());

    let api_key = cli_api_key
        .map(|s| s.to_string())
        .or_else(|| {
            saved
                .as_ref()
                .and_then(|c| c.provider.api_key.clone())
        })
        .or_else(|| resolve_api_key(None, &provider_name))
        .or_else(|| {
            // Interactive fallback prompt if missing
            let token = crate::tui_menu::run_tui_input(
                &format!("🔧 Missing {} API Key", provider_name),
                "Enter API Key to continue:",
                "",
                true
            ).ok().flatten().unwrap_or_default();
            
            if !token.is_empty() {
                // Try and save it to config automatically
                let _ = setup::run_config_set("api_key", &token);
                // Even set provider just in case if doing it strictly
                let _ = setup::run_config_set("provider", &provider_name);
                Some(token)
            } else {
                None
            }
        })
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No API key found!\n\n\
                 Run {} to set up, or:\n\
                 • {} to set key directly\n\
                 • Set {} environment variable\n\
                 • Pass {}",
                "zenclaw setup".cyan(),
                "zenclaw config set api_key <KEY>".cyan(),
                match provider_name.as_str() {
                    "openai" => "OPENAI_API_KEY",
                    "gemini" => "GEMINI_API_KEY",
                    "openrouter" => "OPENROUTER_API_KEY",
                    "groq" => "GROQ_API_KEY",
                    _ => "<PROVIDER>_API_KEY",
                },

                "--api-key <KEY>".cyan()
            )
        })?;

    let api_base = cli_api_base
        .map(|s| s.to_string())
        .or_else(|| saved.as_ref().and_then(|c| c.provider.api_base.clone()));

    Ok((provider_name, model, api_key, api_base))
}

/// Build agent with all tools + plugins.
async fn build_agent(model: &str, skill_prompt: Option<&str>) -> Agent {
    let mut system_prompt = match skill_prompt {
        Some(p) => format!(
            "You are ZenClaw, a helpful AI assistant. You have access to tools to help the user.\n\
             Use tools when needed to accomplish tasks.\n\
             Always be helpful, concise, and accurate.\n\n\
             {}", p
        ),
        None => zenclaw_core::agent::DEFAULT_SYSTEM_PROMPT.to_string(),
    };

    // Load project context file (.zenclaw.md or ZENCLAW.md) if present
    for name in &[".zenclaw.md", "ZENCLAW.md", "zenclaw.md"] {
        if let Ok(ctx) = std::fs::read_to_string(name) {
            system_prompt.push_str("\n\n## Project Context\n\n");
            system_prompt.push_str(&ctx);
            tracing::info!("Loaded project context from {}", name);
            break;
        }
    }

    let mut agent = Agent::with_config(AgentConfig {
        model: Some(model.to_string()),
        system_prompt,
        ..Default::default()
    });

    agent.tools.register(ShellTool::new());
    agent.tools.register(ProcessTool::new());
    agent.tools.register(SubAgentTool::new());
    agent.tools.register(ReadFileTool::new());
    agent.tools.register(WriteFileTool::new());
    agent.tools.register(EditFileTool::new());
    agent.tools.register(ListDirTool::new());
    agent.tools.register(CodebaseSearchTool::new());
    agent.tools.register(WebFetchTool::new());
    agent.tools.register(WebScrapeTool::new());
    agent.tools.register(WebSearchTool::new());
    agent.tools.register(SystemInfoTool::new());
    agent.tools.register(CronTool::new());
    agent.tools.register(HealthTool::new());
    agent.tools.register(HistoryTool::new());
    agent.tools.register(EnvTool::new());

    // Load plugins
    let data = setup::data_dir();
    let plugin_mgr = PluginManager::new(&data.join("plugins"));
    let plugins = plugin_mgr.load_all().await;
    for plugin in plugins {
        agent.tools.register(plugin);
    }

    agent
}

// ─── Main ──────────────────────────────────────────────────

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Setup filesystem log trailing instead of dumping tracing to stdout
    let log_dir = setup::data_dir().join("logs");
    std::fs::create_dir_all(&log_dir).ok();
    let file_appender = tracing_appender::rolling::daily(&log_dir, "zenclaw.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,zenclaw_core=debug,zenclaw_hub=debug")),
        )
        .with_writer(non_blocking)
        .with_ansi(false) // logs in file should probably not have color
        .init();

    let cli = Cli::parse();

    match cli.command {
        // ─── Setup Wizard ──────────────────────────────
        Some(Commands::Setup) => {
            setup::run_setup()?;
        }

        // ─── Interactive Chat ──────────────────────────
        Some(Commands::Chat {
            model,
            provider,
            api_key,
            api_base,
            skill,
        }) => {
            run_chat(
                provider.as_deref(),
                model.as_deref(),
                api_key.as_deref(),
                api_base.as_deref(),
                skill.unwrap_or_default(),
            )
            .await?;
        }

        // ─── One-shot Ask ──────────────────────────────
        Some(Commands::Ask {
            message,
            model,
            provider,
            api_key,
        }) => {
            run_ask(
                provider.as_deref(),
                model.as_deref(),
                api_key.as_deref(),
                &message,
            )
            .await?;
        }

        // ─── Config Management ─────────────────────────
        Some(Commands::Config { action }) => match action {
            ConfigAction::Show => setup::run_config_show()?,
            ConfigAction::Set { key, value } => setup::run_config_set(&key, &value)?,
            ConfigAction::Path => {
                println!("{}", ZenClawConfig::default_path().display());
            }
        },

        // ─── Status ────────────────────────────────────
        Some(Commands::Status) => {
            run_status().await?;
        }

        // ─── Telegram Bot ──────────────────────────────
        Some(Commands::Telegram {
            token,
            model,
            provider,
            api_key,
            allowed_users,
        }) => {
            run_telegram(
                token.as_deref(),
                provider.as_deref(),
                model.as_deref(),
                api_key.as_deref(),
                allowed_users.as_deref(),
            )
            .await?;
        }

        // ─── Discord Bot ──────────────────────────────
        Some(Commands::Discord {
            token,
            model,
            provider,
            api_key,
        }) => {
            run_discord(
                token.as_deref(),
                provider.as_deref(),
                model.as_deref(),
                api_key.as_deref(),
            )
            .await?;
        }

        // ─── Slack Bot ────────────────────────────────
        Some(Commands::Slack {
            token,
            model,
            provider,
            api_key,
            allowed_channels,
        }) => {
            let channels = allowed_channels
                .map(|s| s.split(',').map(|s| s.trim().to_string()).collect())
                .unwrap_or_default();
            run_slack(
                token.as_deref(),
                provider.as_deref(),
                model.as_deref(),
                api_key.as_deref(),
                channels,
            )
            .await?;
        }

        // ─── Skills ───────────────────────────────────
        Some(Commands::Skills { action }) => {
            run_skills(action).await?;
        }

        Some(Commands::Serve {
            host,
            port,
            provider,
            model,
            api_key,
        }) => {
            run_serve(
                &host,
                port,
                provider.as_deref(),
                model.as_deref(),
                api_key.as_deref(),
            )
            .await?;
        }

        Some(Commands::Whatsapp {
            bridge,
            provider,
            model,
            api_key,
            allowed_numbers,
        }) => {
            run_whatsapp(
                &bridge,
                provider.as_deref(),
                model.as_deref(),
                api_key.as_deref(),
                allowed_numbers.as_deref(),
            )
            .await?;
        }

        Some(Commands::Update) => {
            run_update_check().await?;
        }

        // ─── Logs Monitoring ───────────────────────────
        Some(Commands::Logs { lines }) => {
            run_logs(lines).await?;
        }

        // ─── Default: show interactive menu loop ─────────────
        None => {
            loop {
                // Clear the screen for a cleaner UI loop experience
                print!("\x1B[2J\x1B[1;1H");
                io::stdout().flush().ok();
                
                let has_config = ZenClawConfig::default_path().exists();

                let selected_action = tui_menu::run_main_menu(has_config)?;

                if selected_action.is_none() {
                    break;
                }

                let choice = selected_action.unwrap();
                let mut should_exit = false;
                
                let result = match choice.as_str() {
                    "setup" => setup::run_setup(),
                    "chat" => run_chat(None, None, None, None, vec![]).await,
                    "switch" => {
                        let _ = setup::run_model_switcher();
                        Ok(())
                    }
                    "telegram" => run_telegram(None, None, None, None, None).await,
                    "discord" => run_discord(None, None, None, None).await,
                    "whatsapp" => run_whatsapp("http://localhost:3001", None, None, None, None).await,
                    "api" => run_serve("127.0.0.1", 3000, None, None, None).await,
                    "skills" => run_skills(None).await,
                    "settings" => {
                        loop {
                            let config_options = vec![
                                tui_menu::MenuItem { label: "1. Show Configuration".into(), description: "Display current config values.".into(), action_key: "0".into() },
                                tui_menu::MenuItem { label: "2. Show Config Path".into(), description: "Show the absolute path to your config file.".into(), action_key: "1".into() },
                                tui_menu::MenuItem { label: "3. Run Setup Wizard".into(), description: "Re-run the first-time setup wizard to generate a new config.".into(), action_key: "2".into() },
                                tui_menu::MenuItem { label: "4. Back".into(), description: "Return to main menu.".into(), action_key: "3".into() },
                            ];
                            let config_sel = tui_menu::run_tui_menu("⚙️ Settings", &config_options, 0)?;
                            
                            match config_sel.as_deref() {
                                Some("0") => {
                                    let mut out = Vec::new();
                                    {
                                        let mut w = std::io::BufWriter::new(&mut out);
                                        // Read the file directly instead of calling `run_config_show` which prints to stdout
                                        if let Ok(c) = std::fs::read_to_string(ZenClawConfig::default_path()) {
                                            use std::io::Write;
                                            writeln!(w, "Current configuration file contents:\n{}", c).unwrap();
                                        } else {
                                            use std::io::Write;
                                            writeln!(w, "No configuration found at {:?}", ZenClawConfig::default_path()).unwrap();
                                        }
                                    }
                                    let content = String::from_utf8_lossy(&out).to_string();
                                    tui_menu::run_tui_text_viewer("Configuration", &content).ok();
                                },
                                Some("1") => {
                                    let path = ZenClawConfig::default_path().display().to_string();
                                    tui_menu::run_tui_text_viewer("Config Path", &path).ok();
                                },
                                Some("2") => {
                                    setup::run_setup().ok();
                                },
                                Some("3") | None => {
                                    break Ok(());
                                }
                                _ => break Ok(())
                            }
                        }
                    }
                    "updates" => run_update_check().await,
                    "logs" => run_logs(50).await,
                    "exit" => {
                        should_exit = true;
                        Ok(())
                    }
                    _ => Ok(())
                };

                // Handle errors gracefully without crashing the loop
                if let Err(e) = result {
                    let _ = tui_menu::run_tui_error("Execution Error", &e.to_string());
                } else if !should_exit {
                    // Only pause if this was NOT a TUI command that already took over the screen
                    let tui_commands = ["setup", "switch", "telegram", "discord", "whatsapp", "api", "skills", "settings", "logs", "chat"];
                    if !tui_commands.contains(&choice.as_str()) {
                        println!("\n{}", "Press Enter to return to main menu...".dimmed());
                        let mut input = String::new();
                        std::io::stdin().read_line(&mut input).ok();
                    }
                }

                if should_exit {
                    break;
                }
            }
        }
    }

    Ok(())
}

// ─── Command Handlers ──────────────────────────────────────

async fn run_chat(
    provider_name: Option<&str>,
    model: Option<&str>,
    api_key: Option<&str>,
    api_base: Option<&str>,
    active_skills: Vec<String>,
) -> anyhow::Result<()> {
    let skill_prompt = if active_skills.is_empty() {
        None
    } else {
        let data = setup::data_dir();
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

    ui::print_session_info(&provider_name, &model, agent.tools.len(), &active_skills);

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
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let terminal = ratatui::Terminal::new(backend)?;

    let bus = std::sync::Arc::new(EventBus::new(32));

    // RUN THE TUI
    let res = tui_app::run_tui(
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

async fn run_ask(
    provider_name: Option<&str>,
    model: Option<&str>,
    api_key: Option<&str>,
    message: &str,
) -> anyhow::Result<()> {
    let (provider_name, model, api_key, _api_base) =
        resolve_config(provider_name, model, api_key, None)?;

    let provider = create_provider(&provider_name, &api_key, &model, None);
    let memory = zenclaw_core::memory::InMemoryStore::new();
    let agent = build_agent(&model, None).await;

    match agent.process(&provider, &memory, message, "oneshot", None).await {
        Ok(response) => println!("{}", response),
        Err(e) => eprintln!("{}: {}", "Error".red(), e),
    }

    Ok(())
}

async fn run_status() -> anyhow::Result<()> {
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

    crate::tui_menu::run_tui_text_viewer("📊 System Status", &out).ok();
    Ok(())
}


async fn run_telegram(
    cli_bot_token: Option<&str>,
    cli_provider: Option<&str>,
    cli_model: Option<&str>,
    cli_api_key: Option<&str>,
    allowed_users: Option<&str>,
) -> anyhow::Result<()> {
    // 1. Resolve agent environment first
    let (agent, provider, memory, resolved_provider, resolved_model) = setup_bot_env(
        cli_provider,
        cli_model,
        cli_api_key,
        None,
        None
    ).await?;

    let agent = Arc::new(agent);
    let provider = Arc::new(provider);
    let memory = Arc::new(memory);

    // 2. Resolve bot token: CLI arg → config → env → TUI prompt
    let saved = setup::load_saved_config();
    let mut current_token = cli_bot_token
        .map(|s| s.to_string())
        .or_else(|| {
            saved
                .as_ref()
                .and_then(|c| c.channels.telegram.as_ref())
                .map(|t| t.bot_token.clone())
                .filter(|t| !t.is_empty())
        });

    let allowed: Vec<i64> = allowed_users
        .unwrap_or("")
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    loop {
        let token = match current_token {
            Some(ref t) => t.clone(),
            None => {
                let t = crate::tui_menu::run_tui_input(
                    "Telegram Bot Token Required",
                    "Enter your Telegram Bot Token:",
                    "",
                    true
                ).ok().flatten().unwrap_or_default();
                
                if t.is_empty() {
                    return Ok(()); // Cancelled
                }
                t
            }
        };

        let config = TelegramConfig {
            bot_token: token.clone(),
            allowed_users: allowed.clone(),
            poll_timeout: 30,
        };

        let mut telegram = zenclaw_hub::channels::TelegramChannel::new(config);
        
        // Start bot (verifies token via getMe)
        match telegram.start(agent.clone(), provider.clone(), memory.clone()).await {
            Ok(_) => {
                let _ = setup::run_config_set("telegram_token", &token);
                // Interactively monitor bot status via TUI
                let details = [
                    ("Channel", "Telegram"),
                    ("Allowed Users", if allowed.is_empty() { "Public" } else { "Restricted" }),
                    ("Poll Timeout", "30s"),
                ];
                let _ = crate::tui_menu::run_bot_dashboard("Telegram", &resolved_provider, &resolved_model, &details, None);
                telegram.stop().await;
                break Ok(());
            }
            Err(e) => {
                let error_msg = e.to_string();
                if error_msg.contains("token is invalid") || error_msg.contains("Unauthorized") {
                    let _ = crate::tui_menu::run_tui_error("Telegram Connection Failed", &format!("{}\n\nPlease check your token and try again.", error_msg));
                    current_token = None; // Force re-prompt
                } else {
                    return Err(e.into());
                }
            }
        }
    }
}

async fn run_discord(
    cli_bot_token: Option<&str>,
    cli_provider: Option<&str>,
    cli_model: Option<&str>,
    cli_api_key: Option<&str>,
) -> anyhow::Result<()> {
    let (agent, provider, memory, resolved_provider, resolved_model) = setup_bot_env(
        cli_provider,
        cli_model,
        cli_api_key,
        None,
        None
    ).await?;

    let agent = Arc::new(agent);
    let provider = Arc::new(provider);
    let memory = Arc::new(memory);

    let saved = setup::load_saved_config();
    let mut current_token = cli_bot_token
        .map(|s| s.to_string())
        .or_else(|| {
            saved
                .as_ref()
                .and_then(|c| c.channels.discord.as_ref())
                .map(|d| d.bot_token.clone())
                .filter(|d| !d.is_empty())
        });

    loop {
        let token = match current_token {
            Some(ref t) => t.clone(),
            None => {
                let t = crate::tui_menu::run_tui_input(
                    "Discord Bot Token Required",
                    "Enter your Discord Bot Token:",
                    "",
                    true
                ).ok().flatten().unwrap_or_default();
                
                if t.is_empty() {
                    return Ok(());
                }
                t
            }
        };

        let config = DiscordConfig {
            bot_token: token.clone(),
            allowed_users: vec![],
        };

        let mut discord = zenclaw_hub::channels::DiscordChannel::new(config);
        
        match discord.start(agent.clone(), provider.clone(), memory.clone()).await {
            Ok(_) => {
                let _ = setup::run_config_set("discord_token", &token);
                let details = [
                    ("Channel", "Discord"),
                    ("Connection", "Gateway/Secure"),
                    ("Allowed Guilds", "All"),
                ];
                let _ = crate::tui_menu::run_bot_dashboard("Discord", &resolved_provider, &resolved_model, &details, None);
                discord.stop().await;
                break Ok(());
            }
            Err(e) => {
                let error_msg = e.to_string();
                if error_msg.contains("token") || error_msg.contains("Unauthorized") || error_msg.contains("401") {
                    let _ = crate::tui_menu::run_tui_error("Discord Connection Failed", &format!("{}\n\nPlease check your discord token.", error_msg));
                    current_token = None;
                } else {
                    return Err(e.into());
                }
            }
        }
    }
}

async fn run_slack(
    cli_bot_token: Option<&str>,
    cli_provider: Option<&str>,
    cli_model: Option<&str>,
    cli_api_key: Option<&str>,
    allowed_channels: Vec<String>,
) -> anyhow::Result<()> {
    let (agent, provider, memory, resolved_provider, resolved_model) = setup_bot_env(
        cli_provider,
        cli_model,
        cli_api_key,
        None,
        None
    ).await?;

    let agent = Arc::new(agent);
    let provider = Arc::new(provider);
    let memory = Arc::new(memory);

    let saved = setup::load_saved_config();
    let mut current_token = cli_bot_token
        .map(|s| s.to_string())
        .or_else(|| {
            saved
                .as_ref()
                .and_then(|c| c.channels.slack.as_ref())
                .map(|s| s.bot_token.clone())
                .filter(|s| !s.is_empty())
        });

    loop {
        let token = match current_token {
            Some(ref t) => t.clone(),
            None => {
                let t = crate::tui_menu::run_tui_input(
                    "Slack Bot Token Required",
                    "Enter your Slack Bot Token:",
                    "",
                    true
                ).ok().flatten().unwrap_or_default();
                
                if t.is_empty() {
                    return Ok(());
                }
                t
            }
        };

        let config = zenclaw_hub::channels::SlackConfig {
            bot_token: token.clone(),
            allowed_channels: allowed_channels.clone(),
        };

        let mut slack = zenclaw_hub::channels::SlackChannel::new(config);
        
        match slack.start(agent.clone(), provider.clone(), memory.clone()).await {
            Ok(_) => {
                let _ = setup::run_config_set("slack_token", &token);
                let details = [
                    ("Channel", "Slack"),
                    ("Allowed Chans", if allowed_channels.is_empty() { "All" } else { "Restricted" }),
                ];
                let _ = crate::tui_menu::run_bot_dashboard("Slack", &resolved_provider, &resolved_model, &details, None);
                slack.stop().await;
                break Ok(());
            }
            Err(e) => {
                let error_msg = e.to_string();
                if error_msg.contains("token") || error_msg.contains("Unauthorized") {
                    let _ = crate::tui_menu::run_tui_error("Slack Connection Failed", &format!("{}\n\nPlease check your slack token.", error_msg));
                    current_token = None;
                } else {
                    return Err(e.into());
                }
            }
        }
    }
}

async fn run_skills(action: Option<SkillAction>) -> anyhow::Result<()> {
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
                crate::tui_menu::run_tui_text_viewer(&skill.title, &content).ok();
            } else {
                let available = skill_mgr.list().iter().map(|s| s.name.as_str()).collect::<Vec<_>>().join(", ");
                crate::tui_menu::run_tui_error("Skill Not Found", &format!("Skill '{}' not found.\n\nAvailable: {}", name, available)).ok();
            }
        }
        _ => {
            loop {
                let mut items = vec![];
                items.push(crate::tui_menu::MenuItem {
                    label: "➕ Create New Skill".to_string(),
                    description: "Create a new specialized AI behavior from scratch.".to_string(),
                    action_key: "create_new".to_string(),
                });

                for skill in skill_mgr.list() {
                    items.push(crate::tui_menu::MenuItem {
                        label: format!("{} {}", "•".cyan(), skill.name),
                        description: format!("Skill: {}\n\n{}\n\nFile: {}", skill.title, skill.description, skill.path.display()),
                        action_key: skill.name.clone(),
                    });
                }
                items.push(crate::tui_menu::MenuItem {
                    label: "❌ Back".to_string(),
                    description: "Return to previous menu.".to_string(),
                    action_key: "back".to_string(),
                });

                if let Ok(Some(action_key)) = crate::tui_menu::run_tui_menu("📚 Manage Skills", &items, 0) {
                    if action_key == "back" {
                        break;
                    }

                    if action_key == "create_new" {
                        let name_input = crate::tui_menu::run_tui_input("New Skill", "Enter internal name (id):", "", false)?;
                        if let Some(name) = name_input {
                            if !name.trim().is_empty() {
                                if let Ok(Some((t, d, c))) = crate::tui_menu::run_tui_skill_editor(&name, &name, "", "") {
                                    skill_mgr.save_skill(&name, &t, &d, &c).await?;
                                }
                            }
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
                                crate::tui_menu::MenuItem { label: "📄 View Content".into(), description: "Read the skill markdown content.".into(), action_key: "view".into() },
                                crate::tui_menu::MenuItem { label: "📝 Edit Skill".into(), description: "Modify title, description, or content.".into(), action_key: "edit".into() },
                                crate::tui_menu::MenuItem { label: "🗑️  Delete Skill".into(), description: "Permanently remove this skill from disk.".into(), action_key: "delete".into() },
                                crate::tui_menu::MenuItem { label: "⬅️  Back".into(), description: "Return to skills list.".into(), action_key: "back".into() },
                            ];
                            let skill_sel = crate::tui_menu::run_tui_menu(&format!("Manage: {}", s_name), &skill_options, 0)?;
                            
                            match skill_sel.as_deref() {
                                Some("view") => {
                                    let content = format!("Skill: {}\nDescription: {}\nFile: {}\n\n{}\n", s_title, s_desc, s_path, s_content);
                                    crate::tui_menu::run_tui_text_viewer(&s_title, &content).ok();
                                },
                                Some("edit") => {
                                    if let Ok(Some((t, d, c))) = crate::tui_menu::run_tui_skill_editor(&s_name, &s_title, &s_desc, &s_content) {
                                        skill_mgr.save_skill(&s_name, &t, &d, &c).await?;
                                        break;
                                    }
                                },
                                Some("delete") => {
                                    let confirm = crate::tui_menu::run_tui_input("Confirm Delete", &format!("Delete '{}'? Type 'yes' to confirm:", s_name), "", false)?;
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

// ─── Serve (REST API) ──────────────────────────────────────

async fn run_serve(
    cli_host: &str,
    cli_port: u16,
    cli_provider: Option<&str>,
    cli_model: Option<&str>,
    cli_api_key: Option<&str>,
) -> anyhow::Result<()> {
    let (agent, provider, memory, resolved_provider, resolved_model) = setup_bot_env(
        cli_provider,
        cli_model,
        cli_api_key,
        None,
        None
    ).await?;

    let data = setup::data_dir();
    let rag_path = data.join("rag.db");
    let rag = zenclaw_hub::memory::RagStore::open(&rag_path).ok().map(Arc::new);

    let host = cli_host.to_string();
    let mut port = cli_port;

    let agent = Arc::new(agent);
    let provider = Arc::new(provider);
    let memory = Arc::new(memory);

    loop {
        let state = zenclaw_hub::api::ApiState {
            agent: agent.clone(),
            provider: provider.clone(),
            memory: memory.clone(),
            rag: rag.clone(),
        };

        // Fail-fast test to see if we can bind to the port
        let addr_str = format!("{}:{}", host, port);
        match tokio::net::TcpListener::bind(&addr_str).await {
            Ok(listener) => {
                drop(listener); // Close it so AXUM can take it

                // Run server in background
                let bg_host = host.clone();
                let bg_port = port;
                let bg_state = state; 
                tokio::spawn(async move {
                    let _ = zenclaw_hub::api::start_server_from_state(bg_state, &bg_host, bg_port).await;
                });

                // Interactively monitor via TUI
                let endpoint = format!("http://{}:{}", host, port);
                let details = [
                    ("Host", host.as_str()),
                    ("Port", &port.to_string()),
                    ("Status", "Listening"),
                    ("Endpoint", endpoint.as_str()),
                ];
                let _ = crate::tui_menu::run_bot_dashboard("REST API", &resolved_provider, &resolved_model, &details, None);
                break Ok(());
            }
            Err(e) => {
                let _ = crate::tui_menu::run_tui_error("Server Startup Failed", &format!("Address {} error: {}\n\nPlease try a different port.", addr_str, e));
                let input = crate::tui_menu::run_tui_input("Assign New Port", "Enter Port Number:", &port.to_string(), false)?;
                if let Some(p_str) = input {
                    if let Ok(p) = p_str.parse() {
                        port = p;
                        continue;
                    }
                }
                break Err(anyhow::anyhow!("Port binding failed. Aborting."));
            }
        }
    }
}

// ─── WhatsApp ──────────────────────────────────────────────

async fn run_whatsapp(
    cli_bridge_url: &str,
    cli_provider: Option<&str>,
    cli_model: Option<&str>,
    cli_api_key: Option<&str>,
    allowed_numbers: Option<&str>,
) -> anyhow::Result<()> {
    let (agent, provider, memory, resolved_provider, resolved_model) = setup_bot_env(
        cli_provider,
        cli_model,
        cli_api_key,
        None,
        None
    ).await?;

    let agent = Arc::new(agent);
    let provider = Arc::new(provider);
    let memory = Arc::new(memory);

    let mut current_bridge_url = if cli_bridge_url.is_empty() {
        "http://localhost:3001".to_string()
    } else {
        cli_bridge_url.to_string()
    };

    let (log_tx, log_rx) = tokio::sync::mpsc::channel(100);

    loop {
        let mut wa = zenclaw_hub::channels::WhatsAppChannel::new(&current_bridge_url);

        if let Some(numbers) = allowed_numbers {
            let nums: Vec<String> = numbers.split(',').map(|s| s.trim().to_string()).collect();
            wa = wa.with_allowed_numbers(nums);
        }

        match wa.start(agent.clone(), provider.clone(), memory.clone(), Some(log_tx.clone())).await {
            Ok(_) => {
                let details = [
                    ("Bridge URL", current_bridge_url.as_str()),
                    ("Poll Interval", "2000ms"),
                    ("Auth", "Bridge-based"),
                ];
                let _ = crate::tui_menu::run_bot_dashboard("WhatsApp", &resolved_provider, &resolved_model, &details, Some(log_rx));
                break Ok(());
            }
            Err(e) => {
                let _ = crate::tui_menu::run_tui_error("WhatsApp Connection Failed", &format!("{}\n\nMake sure your bridge is running or check the URL.", e));
                let input = crate::tui_menu::run_tui_input(
                    "WhatsApp Bridge URL", 
                    "Enter Bridge URL:", 
                    &current_bridge_url, 
                    false
                )?;
                
                if let Some(new_url) = input {
                    current_bridge_url = new_url;
                } else {
                    return Ok(()); // Cancelled
                }
            }
        }
    }
}

// ─── Update Check ──────────────────────────────────────────

async fn run_update_check() -> anyhow::Result<()> {
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
            crate::tui_menu::run_tui_text_viewer("🔄 Update Available", &out).ok();
        }
        Ok(None) => {
            let msg = format!("✅ You're on the latest version! (v{})", env!("CARGO_PKG_VERSION"));
            crate::tui_menu::run_tui_text_viewer("🔄 Update Check", &msg).ok();
        }
        Err(e) => {
            crate::tui_menu::run_tui_error("Update Check Failed", &format!("Unable to check for updates:\n{}", e)).ok();
        }
    }
    Ok(())
}


async fn run_logs(initial_lines: usize) -> anyhow::Result<()> {
    use tokio::io::{AsyncBufReadExt, AsyncSeekExt, BufReader};
    use tokio::fs::File;
    use std::sync::mpsc;
    use std::time::Duration;

    let log_dir = setup::data_dir().join("logs");

    // Find the most recent log file — tracing_appender uses UTC dates,
    // which may differ from local time, so we pick the newest file instead.
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
            crate::tui_menu::run_tui_error(
                "Log File Not Found",
                &format!("No log files in:\n{}\n\nRun the app first to generate logs.", log_dir.display()),
            ).ok();
            return Ok(());
        }
    };

    // Use std::sync::mpsc so the sync TUI can call try_recv directly
    let (tx, mut rx) = mpsc::channel::<String>();

    // Load initial tail
    let mut initial_logs: Vec<String> = Vec::new();
    if let Ok(content) = std::fs::read_to_string(&log_file) {
        let lines: Vec<&str> = content.lines().collect();
        let start = lines.len().saturating_sub(initial_lines);
        for line in lines.into_iter().skip(start) {
            initial_logs.push(line.to_string());
        }
    }

    // Spawn async tailing task — sends new lines through the sync channel
    let log_file_clone = log_file.clone();
    let tail_handle = tokio::spawn(async move {
        if let Ok(file) = File::open(&log_file_clone).await {
            if let Ok(metadata) = file.metadata().await {
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
        }
    });

    let file_label = log_file
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("zenclaw.log");

    // Delegate all TUI rendering to tui_menu (DRY)
    crate::tui_menu::run_tui_log_viewer(initial_logs, &mut rx, file_label).ok();

    tail_handle.abort();
    Ok(())
}

