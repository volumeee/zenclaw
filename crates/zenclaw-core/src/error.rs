//! Error types for ZenClaw.

use thiserror::Error;

/// Core error type for all ZenClaw operations.
#[derive(Error, Debug)]
pub enum ZenClawError {
    #[error("Provider error: {0}")]
    Provider(String),

    #[error("Tool execution error: {tool} — {message}")]
    ToolExecution { tool: String, message: String },

    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Memory error: {0}")]
    Memory(String),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Max iterations reached ({0})")]
    MaxIterations(usize),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, ZenClawError>;
