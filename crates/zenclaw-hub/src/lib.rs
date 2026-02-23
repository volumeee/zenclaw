//! # ZenClaw Hub
//!
//! Full AI agent implementation with providers, tools, persistent memory,
//! channel adapters (Telegram, Discord, WhatsApp), skills, plugins, RAG,
//! multi-agent router, REST API server, and auto-updater.

pub mod api;
pub mod channels;
pub mod memory;
pub mod plugins;
pub mod providers;
pub mod router;
pub mod skills;
pub mod tools;
pub mod updater;
