//! ZenClaw CLI — Terminal UI Components
//!
//! Single source of truth for every visual element.
//! Changing a colour, border width, or layout is always a one-file edit.

#![allow(dead_code)]

use colored::*;
use std::io::{self, Write};

// ─── Constants ──────────────────────────────────────────────
/// Standard card inner width (characters between the two border chars).
const CARD_WIDTH: usize = 50;

// ─── Low-level Box Drawing ──────────────────────────────────

/// Rounded card (`╭╮╰╯`) with optional title.
///
/// ```text
/// ╭─── Title ────────────────────────────╮
/// │  line1                               │
/// │  line2                               │
/// ╰──────────────────────────────────────╯
/// ```
pub fn print_card(title: &str, lines: &[&str], width: usize) {
    let inner = width.saturating_sub(2);

    // ── top ──────────────────────────────────────────
    let top = if title.is_empty() {
        format!("╭{}╮", "─".repeat(inner))
    } else {
        let label = format!(" {} ", title);
        let remaining = inner.saturating_sub(label.chars().count() + 1);
        format!("╭─{}{}╮", label, "─".repeat(remaining))
    };
    println!("{}", top.cyan());

    // ── body ─────────────────────────────────────────
    for line in lines {
        let visible = strip_ansi_len(line);
        let pad = inner.saturating_sub(visible + 2);
        println!("{}  {}{}{}", "│".cyan(), line, " ".repeat(pad), "│".cyan());
    }

    // ── bottom ───────────────────────────────────────
    println!("{}", format!("╰{}╯", "─".repeat(inner)).cyan());
}

/// Double-line card (`╔╗╚╝`) for hero banners.
fn print_hero(lines: &[&str], width: usize, color: fn(&str) -> ColoredString) {
    let inner = width.saturating_sub(2);
    println!("{}", color(&format!("╔{}╗", "═".repeat(inner))));
    for line in lines {
        let visible = strip_ansi_len(line);
        let pad = inner.saturating_sub(visible + 2);
        println!("{}  {}{}{}", color("║"), line, " ".repeat(pad), color("║"));
    }
    println!("{}", color(&format!("╚{}╝", "═".repeat(inner))));
}

/// Inline badge `[label]`.
pub fn badge(label: &str) -> ColoredString {
    format!("[{}]", label).cyan().bold()
}

// ─── Banner ─────────────────────────────────────────────────

/// Main app banner used everywhere: menu, chat, telegram, etc.
pub fn print_banner() {
    let v = env!("CARGO_PKG_VERSION");
    println!();
    print_hero(
        &[
            &format!("⚡ {} ⚡", format!("ZenClaw v{}", v).bold()),
            "Build AI the simple way  🦀",
            &"▓▓▒▒░░░░░░░░░░░░░░░░░░░░░░░░░▒▒▓▓".dimmed().to_string(),
        ],
        CARD_WIDTH,
        |s| s.cyan(),
    );
    println!();
}

/// Setup wizard banner — same width, green accent.
pub fn print_setup_banner() {
    println!();
    print_hero(
        &[
            &format!("⚡ {} ⚡", "ZenClaw Setup Wizard".bold()),
            "Configure your AI in seconds",
        ],
        CARD_WIDTH,
        |s| s.cyan(),
    );
    println!();
}

/// Success card after setup completes.
pub fn print_setup_complete(config_path: &str, provider: &str, model: &str, has_key: bool) {
    println!();
    print_hero(
        &[&format!("{}", "✅ Setup Complete!".bold())],
        CARD_WIDTH,
        |s| s.green(),
    );
    println!();
    // details below the card
    println!("  {} {}", "Config:".dimmed(), config_path.dimmed());
    println!("  {} {}", "Provider:".dimmed(), provider.green());
    println!("  {} {}", "Model:".dimmed(), model.cyan());
    if has_key {
        println!("  {} {}", "API Key:".dimmed(), "••••••••••••(saved)".green());
    }
    println!();
    println!("  {} {}", "🚀".green(), "Ready! Returning to Main Menu...".green().bold());
    println!();
}

// ─── Session Info ───────────────────────────────────────────

/// Compact info card at the start of `zenclaw chat`.
pub fn print_session_info(
    provider: &str,
    model: &str,
    tools_count: usize,
    skills: &[String],
) {
    print_banner();

    let tools_badge = badge(&format!("{} tools", tools_count));
    let memory_badge = badge("SQLite");

    print_card(
        "Session",
        &[
            &format!(
                "{} {} {}  {}  {} {} {}",
                "Provider".dimmed(),
                "›".dimmed(),
                provider.green().bold(),
                "│".dimmed(),
                "Model".dimmed(),
                "›".dimmed(),
                model.cyan().bold()
            ),
            &format!(
                "{}  {}  {}",
                tools_badge,
                memory_badge,
                if skills.is_empty() {
                    String::new()
                } else {
                    format!("{} {}", "Skills:".dimmed(), skills.join(", ").yellow())
                }
            ),
            &format!(
                "{}",
                "↑↓ history  │  /help for commands".dimmed()
            ),
        ],
        CARD_WIDTH,
    );
    println!();
}

// ─── Chat Bubbles ───────────────────────────────────────────

/// Print the AI label then flush so markdown follows inline.
pub fn print_ai_prefix() {
    print!("\n{} ", "AI ›".bright_cyan().bold());
    io::stdout().flush().unwrap_or(());
}

/// Thin separator between chat turns.
pub fn print_turn_divider() {
    println!("{}", format!("  {}", "─".repeat(CARD_WIDTH - 4)).dimmed());
}

// ─── Code Tip ───────────────────────────────────────────────

/// Hint shown when the AI reply contains fenced code blocks.
pub fn print_code_tip() {
    println!(
        "\n  {} {} {} {} {}",
        "💡 Tip:".dimmed(),
        "/copy".bold().cyan(),
        "copies code,".dimmed(),
        "/run".bold().cyan(),
        "executes it in your terminal.".dimmed()
    );
    println!();
}

// ─── Help ───────────────────────────────────────────────────

/// Pretty command reference table.
pub fn print_help() {
    let cmds: &[(&str, &str)] = &[
        ("/quit",   "Exit ZenClaw"),
        ("/clear",  "Clear conversation history"),
        ("/tools",  "List all registered tools"),
        ("/model",  "Switch AI provider / model on the fly"),
        ("/skills", "List available skill packs"),
        ("/copy",   "Copy last code block to clipboard"),
        ("/run",    "Execute last code block in a sub-shell"),
        ("/help",   "Show this command reference"),
    ];

    let w = CARD_WIDTH;
    let inner = w - 2;
    println!();
    println!("{}", format!("╭─ {} {}╮", "Commands".bold(), "─".repeat(inner.saturating_sub(13))).cyan());
    for (cmd, desc) in cmds {
        let content = format!("  {:10} {}  {}", cmd.bold().cyan(), "│".dimmed(), desc.dimmed());
        let visible = strip_ansi_len(&content);
        let pad = inner.saturating_sub(visible);
        println!("{}{}{}{}",  "│".cyan(), content, " ".repeat(pad), "│".cyan());
    }
    println!("{}", format!("╰{}╯", "─".repeat(inner)).cyan());
    println!();
}

// ─── Model Status ───────────────────────────────────────────

pub fn print_model_status(provider: &str, model: &str) {
    print_card(
        "Current Model",
        &[
            &format!(
                "{} {}  {}  {}",
                "Provider ›".dimmed(),
                provider.green().bold(),
                "│".dimmed(),
                model.cyan().bold()
            ),
        ],
        CARD_WIDTH,
    );
}

// ─── Tools List ─────────────────────────────────────────────

pub fn print_tools_list(names: impl Iterator<Item = String>) {
    let items: Vec<String> = names.collect();
    let lines: Vec<&str> = items.iter().map(|s| s.as_str()).collect();

    let inner = CARD_WIDTH - 2;
    println!();
    println!("{}", format!("╭─ {} {}╮", "🔧 Tools".bold(), "─".repeat(inner.saturating_sub(13))).cyan());
    for name in &lines {
        let content = format!("  {} {}", "•".dimmed(), name.cyan());
        let visible = strip_ansi_len(&content);
        let pad = inner.saturating_sub(visible);
        println!("{}{}{}{}", "│".cyan(), content, " ".repeat(pad), "│".cyan());
    }
    println!("{}", format!("╰{}╯", "─".repeat(inner)).cyan());
    println!();
}

// ─── Skills List ────────────────────────────────────────────

pub fn print_skills_list(skills: &[(String, String, bool)]) {
    let inner = CARD_WIDTH - 2;
    println!();
    println!("{}", format!("╭─ {} {}╮", "📚 Skills".bold(), "─".repeat(inner.saturating_sub(14))).cyan());
    for (name, desc, active) in skills {
        let marker = if *active { "✅" } else { "  " };
        let content = format!("  {} {} — {}", marker, name.cyan(), desc.dimmed());
        let visible = strip_ansi_len(&content);
        let pad = inner.saturating_sub(visible);
        println!("{}{}{}{}", "│".cyan(), content, " ".repeat(pad), "│".cyan());
    }
    println!("{}", format!("╰{}╯", "─".repeat(inner)).cyan());
    println!(
        "  {} {}",
        "Tip:".dimmed(),
        "Use --skill <name> to activate".dimmed()
    );
    println!();
}

// ─── Markdown Skin ──────────────────────────────────────────

/// Centralized `termimad` skin — all Markdown styles in one place.
pub fn make_mad_skin() -> termimad::MadSkin {
    let mut skin = termimad::MadSkin::default();
    skin.set_headers_fg(termimad::crossterm::style::Color::Cyan);
    skin.bold.set_fg(termimad::crossterm::style::Color::Yellow);
    skin.italic.set_fg(termimad::crossterm::style::Color::Green);
    skin.quote_mark.set_fg(termimad::crossterm::style::Color::DarkGrey);
    skin.inline_code.set_fg(termimad::crossterm::style::Color::Magenta);
    skin
}

// ─── Utility ────────────────────────────────────────────────

/// Approximate visible character width ignoring ANSI escape sequences.
fn strip_ansi_len(s: &str) -> usize {
    let mut count = 0;
    let mut in_escape = false;
    for ch in s.chars() {
        if ch == '\x1b' {
            in_escape = true;
        } else if in_escape {
            if ch == 'm' {
                in_escape = false;
            }
        } else {
            count += 1;
        }
    }
    count
}
