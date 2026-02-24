//! Interactive setup wizard — beautiful TUI for configuring ZenClaw.

use colored::*;
use dialoguer::{theme::ColorfulTheme, Password, Select};
use std::path::PathBuf;

use zenclaw_core::config::ZenClawConfig;
use zenclaw_core::provider::ProviderConfig;

/// Provider info for the selection menu.
#[allow(dead_code)]
struct ProviderInfo {
    name: &'static str,
    display: &'static str,
    models: &'static [&'static str],
    default_model: &'static str,
    env_var: &'static str,
    api_base: Option<&'static str>,
    needs_key: bool,
}

const PROVIDERS: &[ProviderInfo] = &[
    ProviderInfo {
        name: "openai",
        display: "🤖 OpenAI (GPT-4o, GPT-4o-mini)",
        models: &[
            "gpt-4o-mini",
            "gpt-4o",
            "o3-mini",
            "o1",
            "o4-mini",
            "gpt-4.1",
            "gpt-4.1-mini",
            "gpt-5",
            "gpt-5.1",
            "gpt-5.2",
        ],
        default_model: "gpt-4o-mini",

        env_var: "OPENAI_API_KEY",
        api_base: None,
        needs_key: true,
    },
    ProviderInfo {
        name: "gemini",
        display: "💎 Google Gemini (Free tier available!)",
        models: &[
            "gemini-2.5-flash",
            "gemini-2.5-pro",
            "gemini-2.5-flash-lite",
            "gemini-2.0-flash",
            "gemini-3-flash",
            "gemini-3.1-pro",
        ],
        default_model: "gemini-2.5-flash",

        env_var: "GEMINI_API_KEY",
        api_base: Some("https://generativelanguage.googleapis.com/v1beta/openai"),
        needs_key: true,
    },
    ProviderInfo {
        name: "groq",
        display: "⚡ Groq (Extremely fast, Free options)",
        models: &[
            "llama-3.3-70b-versatile",
            "llama-3.1-8b-instant",
            "mixtral-8x7b-32768",
            "gemma2-9b-it",
            "deepseek-r1-distill-llama-70b",
        ],
        default_model: "llama-3.3-70b-versatile",
        env_var: "GROQ_API_KEY",
        api_base: Some("https://api.groq.com/openai/v1"),
        needs_key: true,
    },

    ProviderInfo {
        name: "openrouter",
        display: "🌐 OpenRouter (100+ models, pay-per-use)",
        models: &[
            "openai/gpt-4o-mini",
            "google/gemini-2.0-flash-exp:free",
            "anthropic/claude-3.5-sonnet",
            "meta-llama/llama-3.3-70b-instruct",
            "deepseek/deepseek-chat",
        ],
        default_model: "openai/gpt-4o-mini",
        env_var: "OPENROUTER_API_KEY",
        api_base: Some("https://openrouter.ai/api/v1"),
        needs_key: true,
    },
    ProviderInfo {
        name: "ollama",
        display: "🦙 Ollama (Local, Free, Private)",
        models: &["llama3.2", "llama3.1", "mistral", "codellama", "phi3", "gemma2"],
        default_model: "llama3.2",
        env_var: "",
        api_base: Some("http://localhost:11434/v1"),
        needs_key: false,
    },
    ProviderInfo {
        name: "lmstudio",
        display: "🖥️  LM Studio (Local, GUI-based)",
        models: &["local-model"],
        default_model: "local-model",
        env_var: "",
        api_base: Some("http://localhost:1234/v1"),
        needs_key: false,
    },
];

/// Run the interactive setup wizard.
pub fn run_setup() -> anyhow::Result<()> {
    let theme = ColorfulTheme::default();

    println!();
    println!(
        "{}",
        "  ╔════════════════════════════════════════╗".cyan()
    );
    println!(
        "{}",
        "  ║    ⚡ ZenClaw Setup Wizard ⚡          ║".cyan()
    );
    println!(
        "{}",
        "  ║    Configure your AI in seconds        ║".cyan()
    );
    println!(
        "{}",
        "  ╚════════════════════════════════════════╝".cyan()
    );
    println!();

    // Step 1: Choose provider
    println!(
        "  {} {}",
        "Step 1/3".green().bold(),
        "Choose your AI provider:".bold()
    );
    println!();

    let provider_names: Vec<&str> = PROVIDERS.iter().map(|p| p.display).collect();
    let provider_idx = Select::with_theme(&theme)
        .items(&provider_names)
        .default(0)
        .interact()?;

    let provider = &PROVIDERS[provider_idx];
    println!();
    println!("  {} {}", "Selected:".dimmed(), provider.display.green());

    // Step 2: Enter API key (if needed)
    let api_key = if provider.needs_key {
        println!();
        println!(
            "  {} {}",
            "Step 2/3".green().bold(),
            format!("Enter your {} API key:", provider.name).bold()
        );
        println!(
            "  {}",
            format!(
                "Get one at: {}",
                match provider.name {
                    "openai" => "https://platform.openai.com/api-keys",
                    "gemini" => "https://aistudio.google.com/apikey",
                    "groq" => "https://console.groq.com/keys",
                    "openrouter" => "https://openrouter.ai/keys",

                    _ => "your provider's website",
                }
            )
            .dimmed()
        );
        println!();

        let key: String = Password::with_theme(&theme)
            .with_prompt("  API Key")
            .interact()?;

        if key.trim().is_empty() {
            println!(
                "  {}",
                "⚠️  No key entered. You can set it later with `zenclaw config set api_key <KEY>`"
                    .yellow()
            );
            None
        } else {
            Some(key.trim().to_string())
        }
    } else {
        println!();
        println!(
            "  {} {}",
            "Step 2/3".green().bold(),
            "No API key needed! (local provider)".bold()
        );
        None
    };

    // Step 3: Choose model
    println!();
    println!(
        "  {} {}",
        "Step 3/3".green().bold(),
        "Choose your default model:".bold()
    );
    println!();

    let model_idx = Select::with_theme(&theme)
        .items(provider.models)
        .default(0)
        .interact()?;

    let model = provider.models[model_idx];
    println!();
    println!("  {} {}", "Selected:".dimmed(), model.green());

    // Load existing config so we don't wipe out other settings (like telegram tokens, system prompt)
    let mut config = load_saved_config().unwrap_or_default();

    // If the user didn't enter a new key, but selected the same provider they already had,
    // we preserve their old API key. Otherwise, we overwrite it (or set to None).
    let final_api_key = if api_key.is_none() && config.provider.provider == provider.name {
        config.provider.api_key.clone()
    } else {
        api_key.clone()
    };

    // Update only the provider section
    config.provider = ProviderConfig {
        provider: provider.name.to_string(),
        model: model.to_string(),
        api_key: final_api_key,
        api_base: provider.api_base.map(|s| s.to_string()),
        ..Default::default()
    };


    let config_path = ZenClawConfig::default_path();
    config.save(&config_path)?;

    // Success!
    println!();
    println!(
        "{}",
        "  ╔════════════════════════════════════════╗".green()
    );
    println!(
        "{}",
        "  ║         ✅ Setup Complete!              ║".green()
    );
    println!(
        "{}",
        "  ╚════════════════════════════════════════╝".green()
    );
    println!();
    println!("  {} {}", "Config saved:".dimmed(), config_path.display());
    println!("  {} {}", "Provider:".dimmed(), provider.display.green());
    println!("  {} {}", "Model:".dimmed(), model.cyan());
    if api_key.is_some() {
    println!("  {} {}", "API Key:".dimmed(), "••••••••••••(saved)".green());
    }
    println!();
    println!("  {} Returning to Main Menu...", "🚀 Ready!".green().bold());
    println!();

    Ok(())
}

/// Interactive config management.
pub fn run_config_set(key: &str, value: &str) -> anyhow::Result<()> {
    let config_path = ZenClawConfig::default_path();
    let mut config = ZenClawConfig::load(&config_path).unwrap_or_default();

    match key {
        "provider" => config.provider.provider = value.to_string(),
        "model" => config.provider.model = value.to_string(),
        "api_key" => config.provider.api_key = Some(value.to_string()),
        "api_base" => config.provider.api_base = Some(value.to_string()),
        "max_iterations" => {
            if let Ok(v) = value.parse() {
                config.agent.max_iterations = v;
            }
        }
        "system_prompt" => config.agent.system_prompt = Some(value.to_string()),
        "telegram_token" => {
            let tg = config.channels.telegram.get_or_insert(
                zenclaw_core::config::TelegramConfig {
                    bot_token: String::new(),
                    allowed_users: vec![],
                },
            );
            tg.bot_token = value.to_string();
        }
        _ => {
            println!("{} Unknown key: {}", "Error:".red(), key);
            println!("\nAvailable keys:");
            for k in &[
                "provider",
                "model",
                "api_key",
                "api_base",
                "max_iterations",
                "system_prompt",
                "telegram_token",
            ] {
                println!("  • {}", k.cyan());
            }
            return Ok(());
        }
    }

    config.save(&config_path)?;
    println!(
        "  {} {} = {}",
        "✅ Set".green(),
        key.cyan(),
        if key.contains("key") || key.contains("token") {
            "••••••••(hidden)".to_string()
        } else {
            value.to_string()
        }
    );

    Ok(())
}

/// Show current config (hide sensitive values).
pub fn run_config_show() -> anyhow::Result<()> {
    let config_path = ZenClawConfig::default_path();

    println!();
    println!("  {} {}", "Config file:".dimmed(), config_path.display());
    println!();

    if !config_path.exists() {
        println!(
            "  {}",
            "No config yet! Run `zenclaw setup` to get started.".yellow()
        );
        return Ok(());
    }

    let config = ZenClawConfig::load(&config_path)?;

    println!("  {}", "┌─ Provider ─────────────────────".dimmed());
    println!(
        "  {} {} = {}",
        "│".dimmed(),
        "provider".cyan(),
        config.provider.provider.green()
    );
    println!(
        "  {} {} = {}",
        "│".dimmed(),
        "model".cyan(),
        config.provider.model.green()
    );
    println!(
        "  {} {} = {}",
        "│".dimmed(),
        "api_key".cyan(),
        if config.provider.api_key.is_some() {
            "••••••••(set)".green()
        } else {
            "(not set)".red()
        }
    );
    if let Some(ref base) = config.provider.api_base {
        println!(
            "  {} {} = {}",
            "│".dimmed(),
            "api_base".cyan(),
            base.dimmed()
        );
    }
    println!("  {}", "│".dimmed());
    println!("  {}", "├─ Agent ────────────────────────".dimmed());
    println!(
        "  {} {} = {}",
        "│".dimmed(),
        "max_iterations".cyan(),
        config.agent.max_iterations.to_string().yellow()
    );
    if let Some(ref prompt) = config.agent.system_prompt {
        println!(
            "  {} {} = {}...",
            "│".dimmed(),
            "system_prompt".cyan(),
            &prompt[..prompt.len().min(40)]
        );
    }
    println!("  {}", "│".dimmed());
    println!("  {}", "├─ Channels ────────────────────".dimmed());
    if let Some(ref tg) = config.channels.telegram {
        println!(
            "  {} {} = {}",
            "│".dimmed(),
            "telegram".cyan(),
            if tg.bot_token.is_empty() {
                "(not set)".red()
            } else {
                "••••••••(set)".green()
            }
        );
    } else {
        println!(
            "  {} {} = {}",
            "│".dimmed(),
            "telegram".cyan(),
            "(not configured)".dimmed()
        );
    }
    if let Some(ref dc) = config.channels.discord {
        println!(
            "  {} {} = {}",
            "│".dimmed(),
            "discord".cyan(),
            if dc.bot_token.is_empty() {
                "(not set)".red()
            } else {
                "••••••••(set)".green()
            }
        );
    }
    println!("  {}", "└───────────────────────────────".dimmed());
    println!();

    Ok(())
}

/// Load provider from saved config.
pub fn load_saved_config() -> Option<ZenClawConfig> {
    let path = ZenClawConfig::default_path();
    if path.exists() {
        ZenClawConfig::load(&path).ok()
    } else {
        None
    }
}

/// Get data directory.
pub fn data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("zenclaw")
}
