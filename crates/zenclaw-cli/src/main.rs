//! ZenClaw CLI — Build AI the simple way.
//!
//! Beautiful, interactive terminal interface for the ZenClaw AI agent.
//! Run `zenclaw setup` to get started!

mod setup;

use std::io::{self, Write};
use std::sync::Arc;

use clap::{Parser, Subcommand};
use colored::*;
use tracing_subscriber::EnvFilter;

use zenclaw_core::agent::{Agent, AgentConfig};
use zenclaw_core::bus::EventBus;
use zenclaw_core::config::ZenClawConfig;
use zenclaw_core::memory::MemoryStore;
use zenclaw_core::provider::ProviderConfig;
use zenclaw_hub::channels::{DiscordConfig, TelegramConfig};
use zenclaw_hub::memory::SqliteMemory;
use zenclaw_hub::providers::OpenAiProvider;
use zenclaw_hub::skills::SkillManager;
use zenclaw_hub::plugins::PluginManager;
use zenclaw_hub::tools::{
    CronTool, EditFileTool, EnvTool, HealthTool, HistoryTool, ListDirTool, ProcessTool,
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

fn print_banner() {
    let version = env!("CARGO_PKG_VERSION");
    let title = format!("    ║        ⚡ ZenClaw v{} ⚡         ║", version);
    println!();
    println!(
        "{}",
        "    ╔══════════════════════════════════════╗".cyan()
    );
    println!("{}", title.cyan());
    println!(
        "{}",
        "    ║   Build AI the simple way 🦀        ║".cyan()
    );
    println!(
        "{}",
        "    ╚══════════════════════════════════════╝\n".cyan()
    );
}

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
    let system_prompt = match skill_prompt {
        Some(p) => format!(
            "You are ZenClaw, a helpful AI assistant. You have access to tools to help the user.\n\
             Use tools when needed to accomplish tasks.\n\
             Always be helpful, concise, and accurate.\n\n\
             {}", p
        ),
        None => zenclaw_core::agent::DEFAULT_SYSTEM_PROMPT.to_string(),
    };

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

                print_banner();

                let has_config = ZenClawConfig::default_path().exists();
                let mut options = vec![
                    "1. 💬 Chat (Interactive)",
                    "2. 🔄 Switch AI Model",
                    "3. 🤖 Start Telegram Bot",
                    "4. 🎮 Start Discord Bot",
                    "5. 📱 Start WhatsApp Bot",
                    "6. 🌐 Start REST API Server",
                    "7. 📚 Manage Skills",
                    "8. ⚙️  Settings",
                    "9. 🔄 Check for Updates",
                    "10. 🐛 View Live Logs",
                    "11. ❌ Exit",
                ];

                if !has_config {
                    options.insert(0, "0. ⚡ Setup Wizard (Start Here)");
                }

                let selection = dialoguer::Select::with_theme(&dialoguer::theme::ColorfulTheme::default())
                    .with_prompt("What would you like to do? (Use arrow keys or type number)")
                    .default(0)
                    .items(&options)
                    .interact()?;

                let choice = options[selection];
                let mut should_exit = false;
                
                let result = if choice.contains("Setup Wizard") {
                    setup::run_setup()
                } else if choice.contains("💬 Chat") {
                    run_chat(None, None, None, None, vec![]).await
                } else if choice.contains("Switch AI Model") {
                    let _ = setup::run_model_switcher();
                    Ok(())
                } else if choice.contains("Telegram") {
                    run_telegram(None, None, None, None, None).await
                } else if choice.contains("Discord") {
                    run_discord(None, None, None, None).await
                } else if choice.contains("WhatsApp") {
                    run_whatsapp("http://localhost:3001", None, None, None, None).await
                } else if choice.contains("REST API") {
                    run_serve("127.0.0.1", 3000, None, None, None).await
                } else if choice.contains("Manage Skills") {
                    run_skills(None).await
                } else if choice.contains("Settings") {
                    let config_options = vec!["1. Show Configuration", "2. Show Config Path", "3. Run Setup Wizard", "4. Back"];
                    let config_sel = dialoguer::Select::with_theme(&dialoguer::theme::ColorfulTheme::default())
                        .with_prompt("⚙️ Settings:")
                        .default(0)
                        .items(&config_options)
                        .interact()?;
                        
                    match config_sel {
                        0 => setup::run_config_show(),
                        1 => {
                            println!("{}", ZenClawConfig::default_path().display());
                            Ok(())
                        },
                        2 => setup::run_setup(),
                        _ => Ok(()),
                    }
                } else if choice.contains("Updates") {
                    run_update_check().await
                } else if choice.contains("Live Logs") {
                    run_logs(50).await
                } else if choice.contains("Exit") {
                    println!("Goodbye! 🦀");
                    should_exit = true;
                    Ok(())
                } else {
                    Ok(())
                };

                // Handle errors gracefully without crashing the loop
                if let Err(e) = result {
                    println!("\n{}", format!("❌ Error: {}", e).red().bold());
                }

                if should_exit {
                    break;
                } else {
                    println!("\n{}", "Press Enter to return to main menu...".dimmed());
                    let mut input = String::new();
                    std::io::stdin().read_line(&mut input).ok();
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

    let (mut agent, mut provider, memory, mut provider_name, mut model) = setup_bot_env(
        provider_name,
        model,
        api_key,
        api_base,
        skill_prompt.as_deref()
    ).await?;

    print_banner();
    println!(
        "  {} {} {} {} {}",
        "Provider:".dimmed(),
        provider_name.green(),
        "│".dimmed(),
        "Model:".dimmed(),
        model.green()
    );
    println!(
        "  {} {} {} {} {}",
        "Tools:".dimmed(),
        agent.tools.len().to_string().cyan(),
        "│".dimmed(),
        "Memory:".dimmed(),
        "SQLite".green()
    );
    if !active_skills.is_empty() {
        println!(
            "  {} {}",
            "Skills:".dimmed(),
            active_skills.join(", ").yellow()
        );
    }
    println!();
    println!(
        "  {} {}",
        "Commands:".dimmed(),
        "/quit /clear /tools /model /skills /help".dimmed()
    );
    println!();

    let session_key = "cli:default";

    loop {
        print!("{} ", "You ›".green().bold());
        io::stdout().flush()?;

        let mut input = String::new();
        if io::stdin().read_line(&mut input)? == 0 {
            break;
        }
        let input = input.trim();

        if input.is_empty() {
            continue;
        }

        match input {
            "/quit" | "/exit" | "/q" => {
                println!("{}", "👋 Goodbye!".cyan());
                break;
            }
            "/clear" => {
                memory.clear_history(session_key).await?;
                println!("{}", "🗑️  History cleared.".yellow());
                continue;
            }
            "/tools" => {
                println!("\n{}", "🔧 Registered Tools:".bold());
                for name in agent.tools.names() {
                    println!("   • {}", name.cyan());
                }
                println!();
                continue;
            }
            "/model" => {
                println!("  {} {}", "Provider:".dimmed(), provider_name.green());
                println!("  {} {}", "Model:".dimmed(), model.green());
                
                if let Ok(Some((p, m, k, b))) = setup::run_model_switcher() {
                    provider_name = p.clone();
                    model = m.clone();
                    agent.config.model = Some(m.clone());
                    provider = create_provider(&p, k.as_deref().unwrap_or(""), &m, b.as_deref());
                }
                
                continue;
            }
            "/skills" => {
                let data = setup::data_dir();
                let mut skill_mgr = SkillManager::new(&data.join("skills"));
                let _ = skill_mgr.load_all().await;
                println!("\n{}", "📚 Available Skills:".bold());
                for skill in skill_mgr.list() {
                    let active = if active_skills.contains(&skill.name) {
                        "✅".to_string()
                    } else {
                        "  ".to_string()
                    };
                    println!(
                        "  {} {} — {}",
                        active,
                        skill.name.cyan(),
                        skill.description.dimmed()
                    );
                }
                println!(
                    "\n  {} {}",
                    "Tip:".dimmed(),
                    "Use --skill <name> to activate".dimmed()
                );
                println!();
                continue;
            }
            "/help" => {
                println!("\n{}", "Commands:".bold());
                println!("  /quit    — Exit");
                println!("  /clear   — Clear conversation history");
                println!("  /tools   — List registered tools");
                println!("  /model   — Show current model");
                println!("  /skills  — List available skills");
                println!("  /help    — Show this help");
                println!();
                continue;
            }
            _ => {}
        }

        // Live transparent status — every agent step prints a new line
        // so the user sees exactly what ZenClaw is doing in real-time.
        let spinner = indicatif::ProgressBar::new_spinner();
        spinner.set_style(
            indicatif::ProgressStyle::with_template("{spinner:.cyan} {msg}")
                .unwrap()
                .tick_strings(&["⠋","⠙","⠹","⠸","⠼","⠴","⠦","⠧","⠇","⠏"]),
        );
        spinner.set_message("🧠 Analyzing your question...");
        spinner.enable_steady_tick(std::time::Duration::from_millis(80));

        let bus = EventBus::new(32);
        let mut rx = bus.subscribe_system();
        let sp_clone = spinner.clone();

        let _bg_task = tokio::spawn(async move {
            while let Ok(event) = rx.recv().await {
                if let Some(msg) = event.format_status() {
                    // Only update the spinner in-place to avoid spamming the terminal with double lines
                    sp_clone.set_message(msg);
                }
            }
        });

        match agent.process(&provider, &memory, input, session_key, Some(&bus)).await {
            Ok(response) => {
                spinner.finish_and_clear();
                
                // Print stylish Markdown output using termimad
                print!("\n{} ", "AI ›".cyan().bold());
                std::io::stdout().flush().unwrap_or(());
                
                // Create a clean skin
                let mut skin = termimad::MadSkin::default();
                skin.set_headers_fg(termimad::crossterm::style::Color::Cyan);
                skin.bold.set_fg(termimad::crossterm::style::Color::Yellow);
                skin.italic.set_fg(termimad::crossterm::style::Color::Green);
                skin.quote_mark.set_fg(termimad::crossterm::style::Color::DarkGrey);
                
                print!("{}", skin.term_text(response.trim()));
                println!();
            }
            Err(e) => {
                spinner.finish_and_clear();
                eprintln!("{} {}\n", "Error:".red().bold(), e);
            }
        }

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
    print_banner();

    let has_config = ZenClawConfig::default_path().exists();
    let config = setup::load_saved_config();

    println!("  {} ZenClaw v{}", "Version:".dimmed(), env!("CARGO_PKG_VERSION"));
    println!("  {} {:?}", "Data dir:".dimmed(), setup::data_dir());
    println!(
        "  {} {} {}",
        "Config:".dimmed(),
        ZenClawConfig::default_path().display(),
        if has_config { "✅".green() } else { "❌ (run `zenclaw setup`)".red() }
    );

    if let Some(ref cfg) = config {
        println!();
        println!("  {}", "Current Settings:".bold());
        println!("    {} {}", "Provider:".dimmed(), cfg.provider.provider.green());
        println!("    {} {}", "Model:".dimmed(), cfg.provider.model.green());
        println!(
            "    {} {}",
            "API Key:".dimmed(),
            if cfg.provider.api_key.is_some() {
                "✅ configured".green()
            } else {
                "❌ not set".red()
            }
        );
    }

    println!();
    println!("  {}", "Environment Variables:".bold());
    let providers = [
        "OPENAI_API_KEY",
        "GEMINI_API_KEY",
        "OPENROUTER_API_KEY",
        "ANTHROPIC_API_KEY",
    ];
    for p in &providers {
        let status = if std::env::var(p).is_ok() {
            "✅".green()
        } else {
            "·".dimmed()
        };
        println!("    {} {}", status, p);
    }
    println!("    {} Ollama (localhost:11434)", "🟡".yellow());

    // Load and show skills
    let data = setup::data_dir();
    let mut skill_mgr = SkillManager::new(&data.join("skills"));
    let skill_count = skill_mgr.load_all().await.unwrap_or(0);
    println!();
    println!("  {}", "Skills:".bold());
    println!("    {} skills loaded from {}", skill_count.to_string().cyan(), data.join("skills").display().to_string().dimmed());

    println!();

    Ok(())
}

async fn run_telegram(
    bot_token: Option<&str>,
    provider_name: Option<&str>,
    model: Option<&str>,
    api_key: Option<&str>,
    allowed_users: Option<&str>,
) -> anyhow::Result<()> {
    let (provider_name, model, api_key, _) =
        resolve_config(provider_name, model, api_key, None)?;

    // Resolve bot token: CLI arg → config → env → error
    let saved = setup::load_saved_config();
    let bot_token = bot_token
        .map(|s| s.to_string())
        .or_else(|| {
            saved
                .as_ref()
                .and_then(|c| c.channels.telegram.as_ref())
                .map(|t| t.bot_token.clone())
                .filter(|t| !t.is_empty())
        })
        .or_else(|| {
            // Interactively prompt for the token if missing!
            println!("\n  {}", "🤖 No Telegram Bot Token found!".yellow());
            println!("  Get one from @BotFather on Telegram.");
            let token: String = dialoguer::Password::with_theme(&dialoguer::theme::ColorfulTheme::default())
                .with_prompt("Enter your Telegram Bot Token")
                .interact()
                .unwrap_or_default();
            
            if !token.is_empty() {
                // Save it to config automatically
                if let Err(e) = setup::run_config_set("telegram_token", &token) {
                    println!("Failed to save token to config: {}", e);
                }
                Some(token)
            } else {
                None
            }
        })
        .ok_or_else(|| anyhow::anyhow!("No Telegram bot token provided. Aborting."))?;

    let allowed: Vec<i64> = allowed_users
        .unwrap_or("")
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    let (agent, provider, memory, provider_name, model) = setup_bot_env(
        Some(&provider_name),
        Some(&model),
        Some(&api_key),
        None,
        None
    ).await?;
    
    let provider = Arc::new(provider);
    let agent = Arc::new(agent);
    let memory = Arc::new(memory);

    print_banner();
    println!("  {} {}", "Mode:".dimmed(), "🤖 Telegram Bot".green().bold());
    println!("  {} {} │ {} {}", "Provider:".dimmed(), provider_name.green(), "Model:".dimmed(), model.green());
    println!(
        "  {} {} │ {} {}",
        "Tools:".dimmed(),
        agent.tools.len().to_string().cyan(),
        "Memory:".dimmed(),
        "SQLite".green()
    );
    if allowed.is_empty() {
        println!("  {} {}", "Access:".dimmed(), "Everyone".yellow());
    } else {
        println!("  {} {:?}", "Allowed:".dimmed(), allowed);
    }
    println!("\n  {}", "Press Ctrl+C to stop".dimmed());

    let config = TelegramConfig {
        bot_token,
        allowed_users: allowed,
        poll_timeout: 30,
    };

    let mut telegram = zenclaw_hub::channels::TelegramChannel::new(config);
    telegram.start(agent, provider, memory).await?;

    tokio::signal::ctrl_c().await?;
    println!("\n{}", "🛑 Shutting down...".yellow());
    telegram.stop().await;
    println!("{}", "👋 Goodbye!".cyan());

    Ok(())
}

async fn run_discord(
    bot_token: Option<&str>,
    provider_name: Option<&str>,
    model: Option<&str>,
    api_key: Option<&str>,
) -> anyhow::Result<()> {
    let (provider_name, model, api_key, _) =
        resolve_config(provider_name, model, api_key, None)?;

    let saved = setup::load_saved_config();
    let bot_token = bot_token
        .map(|s| s.to_string())
        .or_else(|| {
            saved
                .as_ref()
                .and_then(|c| c.channels.discord.as_ref())
                .map(|d| d.bot_token.clone())
                .filter(|t| !t.is_empty())
        })
        .or_else(|| {
            // Interactively prompt for the token if missing!
            println!("\n  {}", "🎮 No Discord Bot Token found!".yellow());
            println!("  Get one from https://discord.com/developers/applications");
            let token: String = dialoguer::Password::with_theme(&dialoguer::theme::ColorfulTheme::default())
                .with_prompt("Enter your Discord Bot Token")
                .interact()
                .unwrap_or_default();
            
            if !token.is_empty() {
                // Save it to config automatically
                if let Err(e) = setup::run_config_set("discord_token", &token) {
                    println!("Failed to save token to config: {}", e);
                }
                Some(token)
            } else {
                None
            }
        })
        .ok_or_else(|| anyhow::anyhow!("No Discord bot token provided. Aborting."))?;

    let (agent, provider, memory, provider_name, model) = setup_bot_env(
        Some(&provider_name),
        Some(&model),
        Some(&api_key),
        None,
        None
    ).await?;
    
    let provider = Arc::new(provider);
    let agent = Arc::new(agent);
    let memory = Arc::new(memory);

    print_banner();
    println!("  {} {}", "Mode:".dimmed(), "🎮 Discord Bot".green().bold());
    println!("  {} {} │ {} {}", "Provider:".dimmed(), provider_name.green(), "Model:".dimmed(), model.green());
    println!(
        "  {} {} │ {} {}",
        "Tools:".dimmed(),
        agent.tools.len().to_string().cyan(),
        "Memory:".dimmed(),
        "SQLite".green()
    );
    println!("\n  {}", "Press Ctrl+C to stop".dimmed());

    let config = DiscordConfig {
        bot_token,
        allowed_users: vec![],
    };

    let mut discord = zenclaw_hub::channels::DiscordChannel::new(config);
    discord.start(agent, provider, memory).await?;

    tokio::signal::ctrl_c().await?;
    println!("\n{}", "🛑 Shutting down...".yellow());
    discord.stop().await;
    println!("{}", "👋 Goodbye!".cyan());

    Ok(())
}

async fn run_skills(action: Option<SkillAction>) -> anyhow::Result<()> {
    let data = setup::data_dir();
    let mut skill_mgr = SkillManager::new(&data.join("skills"));
    skill_mgr.load_all().await?;

    match action {
        Some(SkillAction::Show { name }) => {
            if let Some(skill) = skill_mgr.get(&name) {
                println!();
                println!("  {} {}", "Skill:".bold(), skill.title.cyan());
                println!("  {} {}", "Description:".dimmed(), skill.description);
                println!("  {} {}", "File:".dimmed(), skill.path.display().to_string().dimmed());
                println!();
                println!("{}", skill.content);
            } else {
                println!("{} Skill '{}' not found.", "Error:".red(), name);
                println!("Available: {}", skill_mgr.list().iter().map(|s| s.name.as_str()).collect::<Vec<_>>().join(", "));
            }
        }
        _ => {
            // List skills (default)
            println!();
            println!("  {}", "📚 Available Skills:".bold());
            println!("  {} {}\n", "Directory:".dimmed(), skill_mgr.dir().display().to_string().dimmed());

            if skill_mgr.list().is_empty() {
                println!("  {}", "No skills found. Skills will be created on first use.".dimmed());
            } else {
                for skill in skill_mgr.list() {
                    println!(
                        "  {} {} — {}",
                        "•".cyan(),
                        skill.name.cyan().bold(),
                        skill.description.dimmed()
                    );
                }
            }

            println!();
            println!("  {}", "Usage:".bold());
            println!(
                "    {} — Activate during chat",
                "zenclaw chat --skill coding".cyan()
            );
            println!(
                "    {} — View skill content",
                "zenclaw skills show coding".cyan()
            );
            println!(
                "    {} — Add custom skill",
                format!("Create a .md file in {}", skill_mgr.dir().display()).dimmed()
            );
            println!();
        }
    }

    Ok(())
}

// ─── Serve (REST API) ──────────────────────────────────────

async fn run_serve(
    host: &str,
    port: u16,
    provider_name: Option<&str>,
    model: Option<&str>,
    api_key: Option<&str>,
) -> anyhow::Result<()> {
    let (agent, provider, memory, provider_name, model) = setup_bot_env(
        provider_name,
        model,
        api_key,
        None,
        None
    ).await?;
    let data = setup::data_dir();
    let rag_path = data.join("rag.db");
    let rag = zenclaw_hub::memory::RagStore::open(&rag_path).ok();

    print_banner();
    println!("  {} {}", "Mode:".dimmed(), "🌐 REST API Server".green().bold());
    println!("  {} {}", "Provider:".dimmed(), provider_name.cyan());
    println!("  {} {}", "Model:".dimmed(), model.cyan());
    println!(
        "  {} {}",
        "Endpoint:".dimmed(),
        format!("http://{}:{}", host, port).green().bold()
    );
    println!();
    println!("  {}", "Endpoints:".bold());
    println!("    {} — Health check", "GET  /v1/health".cyan());
    println!("    {} — System status", "GET  /v1/status".cyan());
    println!("    {} — Chat with agent", "POST /v1/chat".cyan());
    println!("    {} — Index document", "POST /v1/rag/index".cyan());
    println!("    {} — Search documents", "POST /v1/rag/search".cyan());
    println!();
    println!("  {}", "Example:".bold());
    println!(
        "    {}",
        format!(
            "curl -X POST http://{}:{}/v1/chat -H 'Content-Type: application/json' -d '{{\"message\": \"hello\"}}'",
            host, port
        )
        .dimmed()
    );
    println!();

    let state = zenclaw_hub::api::ApiState {
        agent,
        provider: Box::new(provider),
        memory: Box::new(memory),
        rag,
    };

    zenclaw_hub::api::start_server(state, host, port).await?;

    Ok(())
}

// ─── WhatsApp ──────────────────────────────────────────────

async fn run_whatsapp(
    bridge_url: &str,
    provider_name: Option<&str>,
    model: Option<&str>,
    api_key: Option<&str>,
    allowed_numbers: Option<&str>,
) -> anyhow::Result<()> {
    let (agent, provider, memory, provider_name, model) = setup_bot_env(
        provider_name,
        model,
        api_key,
        None,
        None
    ).await?;

    print_banner();
    println!("  {} {}", "Mode:".dimmed(), "📱 WhatsApp Bot".green().bold());
    println!("  {} {}", "Bridge:".dimmed(), bridge_url.cyan());
    println!("  {} {}", "Provider:".dimmed(), provider_name.cyan());
    println!("  {} {}", "Model:".dimmed(), model.cyan());
    println!();

    let mut wa = zenclaw_hub::channels::WhatsAppChannel::new(bridge_url);

    if let Some(numbers) = allowed_numbers {
        let nums: Vec<String> = numbers.split(',').map(|s| s.trim().to_string()).collect();
        wa = wa.with_allowed_numbers(nums);
    }

    wa.start(&agent, &provider, &memory).await?;

    Ok(())
}

// ─── Update Check ──────────────────────────────────────────

async fn run_update_check() -> anyhow::Result<()> {
    print_banner();
    println!("  🔄 Checking for updates...\n");

    match zenclaw_hub::updater::check_for_updates().await {
        Ok(Some(info)) => {
            println!("  🆕 New version available!");
            println!("     Current: v{}", info.current);
            println!("     Latest:  v{}", info.latest.green().bold());
            println!("     URL:     {}", info.url.cyan());

            if !info.changelog.is_empty() {
                let preview = if info.changelog.len() > 300 {
                    format!("{}...", &info.changelog[..300])
                } else {
                    info.changelog.clone()
                };
                println!("\n  {}:", "Changelog".bold());
                for line in preview.lines().take(10) {
                    println!("    {}", line.dimmed());
                }
            }

            println!(
                "\n  To update run this command in your terminal:\n  {}",
                "wget -qO- https://github.com/volumeee/zenclaw/releases/download/v0.1.4/zenclaw-linux-$(uname -m).tar.gz | tar -xz && sudo mv zenclaw-linux-$(uname -m) /usr/local/bin/zenclaw".cyan()
            );
        }
        Ok(None) => {
            println!(
                "  ✅ You're on the latest version! (v{})",
                env!("CARGO_PKG_VERSION")
            );
        }
        Err(e) => {
            println!("  ⚠️ Unable to check for updates: {}", e.to_string().dimmed());
        }
    }

    println!();
    Ok(())
}

async fn run_logs(initial_lines: usize) -> anyhow::Result<()> {
    use tokio::io::{AsyncBufReadExt, AsyncSeekExt, BufReader};
    use tokio::fs::File;
    use chrono;
    
    let log_dir = setup::data_dir().join("logs");
    let log_file = log_dir.join(format!("zenclaw.log.{}", chrono::Local::now().format("%Y-%m-%d")));
    
    if !log_file.exists() {
        println!("{} Log file doesn't exist yet at: {}", "Info:".yellow(), log_file.display());
        return Ok(());
    }

    println!("{}", format!("👀 Tailing ZenClaw logs from {}...", log_file.display()).cyan());
    println!("{}", "Press Ctrl+C to exit.".dimmed());
    println!();
    
    // Quick ugly way to get last N lines synchronously
    if let Ok(content) = std::fs::read_to_string(&log_file) {
        let lines: Vec<&str> = content.lines().collect();
        let start = lines.len().saturating_sub(initial_lines);
        for line in lines.into_iter().skip(start) {
            println!("{}", colorize_log(line));
        }
    }

    // Tail future lines asynchronously
    let file = File::open(&log_file).await?;
    let metadata = file.metadata().await?;
    let mut reader = BufReader::new(file);
    // Seek to end
    reader.seek(std::io::SeekFrom::Start(metadata.len())).await?;
    
    let mut line_buf = String::new();
    loop {
        line_buf.clear();
        let bytes = reader.read_line(&mut line_buf).await?;
        if bytes == 0 {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            continue;
        }
        
        let trimmed = line_buf.trim_end();
        if !trimmed.is_empty() {
            println!("{}", colorize_log(trimmed));
        }
    }
}

fn colorize_log(line: &str) -> String {
    if line.contains(" ERROR ") {
        line.red().to_string()
    } else if line.contains(" WARN ") {
        line.yellow().to_string()
    } else if line.contains(" INFO ") {
        line.green().to_string()
    } else if line.contains(" DEBUG ") {
        line.blue().to_string()
    } else {
        line.dimmed().to_string()
    }
}
