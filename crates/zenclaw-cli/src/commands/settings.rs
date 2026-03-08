use zenclaw_core::config::ZenClawConfig;
use crate::{setup, tui_menu};

pub fn run_settings() -> anyhow::Result<()> {
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
