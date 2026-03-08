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
pub mod commands;

use std::io::{self, Write};

use clap::{Parser, Subcommand};
use colored::*;
use tracing_subscriber::EnvFilter;

use zenclaw_core::agent::{Agent, AgentConfig};
use zenclaw_core::config::ZenClawConfig;
use zenclaw_core::provider::ProviderConfig;
use zenclaw_hub::memory::SqliteMemory;
use zenclaw_hub::providers::OpenAiProvider;
use zenclaw_hub::plugins::PluginManager;
use zenclaw_hub::tools::{
    CodebaseSearchTool, CronTool, EditFileTool, EnvTool, HealthTool, HistoryTool, ListDirTool, ProcessTool,
    ReadFileTool, ShellTool, SubAgentTool, SystemInfoTool, WebFetchTool, WebScrapeTool,
    WebSearchTool, WriteFileTool, SkillsMpTool,
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

    /// 🧹 Run maintenance tasks (e.g. prune history)
    Maintenance {
        /// Number of days of history to retain (default 30)
        #[arg(long, default_value_t = 30)]
        retention_days: u32,
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
pub enum SkillAction {
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
        "deepseek" => OpenAiProvider::deepseek(api_key, model),
        "openclaw" => OpenAiProvider::openclaw(api_key, model),
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

/// Resolve provider config: CLI args → env vars (ZENCLAW_*) → saved config → defaults
fn resolve_config(
    cli_provider: Option<&str>,
    cli_model: Option<&str>,
    cli_api_key: Option<&str>,
    cli_api_base: Option<&str>,
) -> anyhow::Result<(String, String, String, Option<String>)> {
    let saved = setup::load_saved_config();

    // Priority: CLI flag → ZENCLAW_* env var → saved config → default
    let provider_name = cli_provider
        .map(|s| s.to_string())
        .or_else(|| std::env::var("ZENCLAW_PROVIDER").ok().filter(|v| !v.is_empty()))
        .or_else(|| saved.as_ref().map(|c| c.provider.provider.clone()))
        .unwrap_or_else(|| "openai".to_string());

    let model = cli_model
        .map(|s| s.to_string())
        .or_else(|| std::env::var("ZENCLAW_MODEL").ok().filter(|v| !v.is_empty()))
        .or_else(|| saved.as_ref().map(|c| c.provider.model.clone()))
        .unwrap_or_else(|| default_model(&provider_name).to_string());

    let api_key = cli_api_key
        .map(|s| s.to_string())
        .or_else(|| std::env::var("ZENCLAW_API_KEY").ok().filter(|v| !v.is_empty()))
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
                 • Set {} for universal override\n\
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
                "ZENCLAW_API_KEY".cyan(),
                "--api-key <KEY>".cyan()
            )
        })?;

    let api_base = cli_api_base
        .map(|s| s.to_string())
        .or_else(|| std::env::var("ZENCLAW_API_BASE").ok().filter(|v| !v.is_empty()))
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

    let config = zenclaw_core::config::ZenClawConfig::load(&zenclaw_core::config::ZenClawConfig::default_path()).unwrap_or_default();
    let allow_exec = std::env::var("ZENCLAW_ALLOW_COMMAND_EXEC").unwrap_or_default() == "1" 
                     || std::env::var("ZENCLAW_ALLOW_COMMAND_EXEC").unwrap_or_default() == "true";

    agent.tools.register(ShellTool::new(allow_exec));
    agent.tools.register(ProcessTool::new(allow_exec));
    agent.tools.register(SubAgentTool::new());
    agent.tools.register(ReadFileTool::new());
    agent.tools.register(WriteFileTool::new());
    agent.tools.register(EditFileTool::new());
    agent.tools.register(ListDirTool::new());
    agent.tools.register(CodebaseSearchTool::new());
    agent.tools.register(WebFetchTool::new());
    agent.tools.register(WebScrapeTool::new(config.tools.jina_api_key.clone()));
    agent.tools.register(WebSearchTool::new(config.tools.jina_api_key.clone()));
    
    agent.tools.register(SkillsMpTool::new(Some(config.tools)));
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
    // Load .env automatically
    dotenvy::dotenv().ok();

    // Note: Config loading and tool API key injection are now safely scoped in tool initialization.

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
            commands::run_chat(
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
            commands::run_ask(
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
            commands::run_status().await?;
        }

        // ─── Telegram Bot ──────────────────────────────
        Some(Commands::Telegram {
            token,
            model,
            provider,
            api_key,
            allowed_users,
        }) => {
            commands::run_telegram(
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
            commands::run_discord(
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
            commands::run_slack(
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
            commands::run_skills(action).await?;
        }

        Some(Commands::Serve {
            host,
            port,
            provider,
            model,
            api_key,
        }) => {
            commands::run_serve(
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
            commands::run_whatsapp(
                &bridge,
                provider.as_deref(),
                model.as_deref(),
                api_key.as_deref(),
                allowed_numbers.as_deref(),
            )
            .await?;
        }

        Some(Commands::Update) => {
            commands::run_update_check().await?;
        }

        // ─── Logs Monitoring ───────────────────────────
        Some(Commands::Logs { lines }) => {
            commands::run_logs(lines).await?;
        }

        // ─── Maintenance ───────────────────────────────
        Some(Commands::Maintenance { retention_days }) => {
            commands::run_maintenance(retention_days).await?;
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
                    "chat" => commands::run_chat(None, None, None, None, vec![]).await,
                    "switch" => {
                        let _ = setup::run_model_switcher();
                        Ok(())
                    }
                    "telegram" => commands::run_telegram(None, None, None, None, None).await,
                    "discord" => commands::run_discord(None, None, None, None).await,
                    "whatsapp" => commands::run_whatsapp("http://localhost:3001", None, None, None, None).await,
                    "api" => commands::run_serve("127.0.0.1", 3000, None, None, None).await,
                    "skills" => commands::run_skills(None).await,
                    "settings" => {
                        loop {
                            let config_options = vec![
                                tui_menu::MenuItem { label: "1. Show Configuration".into(), description: "Display current config values.".into(), action_key: "0".into() },
                                tui_menu::MenuItem { label: "2. Show Config Path".into(), description: "Show the absolute path to your config file.".into(), action_key: "1".into() },
                                tui_menu::MenuItem { label: "3. Run Setup Wizard".into(), description: "Re-run the first-time setup wizard to generate a new config.".into(), action_key: "2".into() },
                                tui_menu::MenuItem { label: "4. Set JINA_API_KEY".into(), description: "Configure API Key for Web Search & Scrape plugin.".into(), action_key: "3".into() },
                                tui_menu::MenuItem { label: "5. Set OPENWEATHER_API_KEY".into(), description: "Configure API Key for Weather data.".into(), action_key: "4".into() },
                                tui_menu::MenuItem { label: "6. Set SERPER_API_KEY".into(), description: "Configure API Key for Serper.dev Search.".into(), action_key: "5".into() },
                                tui_menu::MenuItem { label: "7. Set SKILLSMP_API_KEY".into(), description: "Configure API Key for SkillsMP Integrations.".into(), action_key: "6".into() },
                                tui_menu::MenuItem { label: "8. Back".into(), description: "Return to main menu.".into(), action_key: "7".into() },
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
                                Some("3") => {
                                    if let Ok(Some(key)) = tui_menu::run_tui_input("Configure API Key", "Enter JINA_API_KEY (leave empty to clear):", "", false) {
                                        let mut config = setup::load_saved_config().unwrap_or_default();
                                        if key.trim().is_empty() {
                                            config.tools.jina_api_key = None;
                                        } else {
                                            config.tools.jina_api_key = Some(key.trim().to_string());
                                        }
                                        let _ = config.save(&ZenClawConfig::default_path());
                                    }
                                },
                                Some("4") => {
                                    if let Ok(Some(key)) = tui_menu::run_tui_input("Configure API Key", "Enter OPENWEATHER_API_KEY (leave empty to clear):", "", false) {
                                        let mut config = setup::load_saved_config().unwrap_or_default();
                                        if key.trim().is_empty() {
                                            config.tools.openweather_api_key = None;
                                        } else {
                                            config.tools.openweather_api_key = Some(key.trim().to_string());
                                        }
                                        let _ = config.save(&ZenClawConfig::default_path());
                                    }
                                },
                                Some("5") => {
                                    if let Ok(Some(key)) = tui_menu::run_tui_input("Configure API Key", "Enter SERPER_API_KEY (leave empty to clear):", "", false) {
                                        let mut config = setup::load_saved_config().unwrap_or_default();
                                        if key.trim().is_empty() {
                                            config.tools.serper_api_key = None;
                                        } else {
                                            config.tools.serper_api_key = Some(key.trim().to_string());
                                        }
                                        let _ = config.save(&ZenClawConfig::default_path());
                                    }
                                },
                                Some("6") => {
                                    if let Ok(Some(key)) = tui_menu::run_tui_input("Configure API Key", "Enter SKILLSMP_API_KEY (leave empty to clear):", "", false) {
                                        let mut config = setup::load_saved_config().unwrap_or_default();
                                        if key.trim().is_empty() {
                                            config.tools.skillsmp_api_key = None;
                                        } else {
                                            config.tools.skillsmp_api_key = Some(key.trim().to_string());
                                        }
                                        let _ = config.save(&ZenClawConfig::default_path());
                                    }
                                },
                                Some("7") | None => {
                                    break Ok(());
                                }
                                _ => break Ok(())
                            }
                        }
                    }
                    "updates" => commands::run_update_check().await,
                    "logs" => commands::run_logs(50).await,
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

