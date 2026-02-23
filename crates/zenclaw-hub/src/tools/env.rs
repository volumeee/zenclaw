//! Environment tool — inspect and manage environment variables.
//!
//! Useful for debugging provider configs, checking API key presence, etc.

use async_trait::async_trait;
use serde_json::{json, Value};

use zenclaw_core::error::Result;
use zenclaw_core::tool::Tool;

/// Environment inspection tool.
pub struct EnvTool;

impl EnvTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for EnvTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for EnvTool {
    fn name(&self) -> &str {
        "env"
    }

    fn description(&self) -> &str {
        "Inspect environment variables. Can check if API keys are set, view PATH, etc. Sensitive values are masked."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["get", "list", "check"],
                    "description": "Action: 'get' a variable, 'list' all vars, 'check' if key vars are set"
                },
                "name": {
                    "type": "string",
                    "description": "Variable name (for 'get' action)"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value) -> Result<String> {
        let action = args["action"].as_str().unwrap_or("check");

        match action {
            "get" => {
                let name = args["name"].as_str().unwrap_or("");
                if name.is_empty() {
                    return Ok("Variable name is required.".to_string());
                }

                match std::env::var(name) {
                    Ok(val) => {
                        // Mask sensitive values
                        let masked = if is_sensitive(name) {
                            mask_value(&val)
                        } else {
                            val.clone()
                        };
                        Ok(format!("{}={}", name, masked))
                    }
                    Err(_) => Ok(format!("{} is not set", name)),
                }
            }
            "list" => {
                let mut vars: Vec<String> = std::env::vars()
                    .filter(|(k, _)| !k.starts_with('_') && !k.starts_with("LS_"))
                    .map(|(k, v)| {
                        let masked = if is_sensitive(&k) {
                            mask_value(&v)
                        } else if v.len() > 80 {
                            format!("{}...", &v[..80])
                        } else {
                            v
                        };
                        format!("  {}={}", k, masked)
                    })
                    .collect();

                vars.sort();
                Ok(format!("Environment ({} vars):\n{}", vars.len(), vars.join("\n")))
            }
            "check" => {
                let keys = [
                    ("OPENAI_API_KEY", "OpenAI"),
                    ("GEMINI_API_KEY", "Google Gemini"),
                    ("OPENROUTER_API_KEY", "OpenRouter"),
                    ("ANTHROPIC_API_KEY", "Anthropic"),
                    ("TELEGRAM_BOT_TOKEN", "Telegram Bot"),
                    ("DISCORD_BOT_TOKEN", "Discord Bot"),
                    ("ZENCLAW_API_KEY", "ZenClaw API Auth"),
                ];

                let mut output = String::from("API Key Status:\n");
                for (key, label) in &keys {
                    let status = if std::env::var(key).is_ok() {
                        "✅ Set"
                    } else {
                        "❌ Not set"
                    };
                    output.push_str(&format!("  {} — {} ({})\n", status, label, key));
                }

                Ok(output)
            }
            _ => Ok(format!("Unknown action: {}. Use 'get', 'list', or 'check'.", action)),
        }
    }
}

/// Check if a variable name likely contains sensitive data.
fn is_sensitive(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("key")
        || lower.contains("secret")
        || lower.contains("token")
        || lower.contains("password")
        || lower.contains("auth")
        || lower.contains("credential")
}

/// Mask a sensitive value, showing only first/last 3 chars.
fn mask_value(val: &str) -> String {
    if val.len() <= 8 {
        "****".to_string()
    } else {
        format!("{}...{}", &val[..3], &val[val.len() - 3..])
    }
}
