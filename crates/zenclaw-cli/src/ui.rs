//! ZenClaw CLI — Terminal UI Components
//!
//! All visual rendering lives here. `main.rs` calls these functions
//! and stays clean. No stray `println!` scattered everywhere.

#![allow(dead_code)] // components library — some items are for future use

use colored::*;
use std::io::{self, Write};

// ─── Palette ────────────────────────────────────────────────
// Keep all colour decisions in one place so changing the theme
// is a single-file edit.

/// Brand accent: electric cyan — used for provider names, highlights.
pub const COLOR_ACCENT: &str = "cyan";
/// Success / AI speaker colour: bright green.
pub const COLOR_SUCCESS: &str = "bright green";
/// Muted labels: dark-grey dimmed text.
pub const COLOR_DIM: &str = "white"; // will be .dimmed()

// ─── Box Drawing Helpers ─────────────────────────────────────

/// Full-width card with a title line.
///
/// ```text
/// ╭── Title ──────────────────────╮
/// │  line1                        │
/// │  line2                        │
/// ╰───────────────────────────────╯
/// ```
pub fn print_card(title: &str, lines: &[&str], width: usize) {
    let inner = width - 2; // border chars on each side

    // ── top bar ────────────────────────────────────────────
    let label = if title.is_empty() {
        "─".repeat(inner)
    } else {
        let t = format!(" {} ", title);
        let dashes = inner.saturating_sub(t.chars().count() + 2);
        format!("─ {}{}", t, "─".repeat(dashes))
    };
    println!("{}", format!("╭{}╮", label).cyan());

    // ── body ───────────────────────────────────────────────
    for line in lines {
        let line_chars = strip_ansi_len(line);
        let padding = inner.saturating_sub(line_chars + 2);
        println!(
            "{}  {}{}{}",
            "│".cyan(),
            line,
            " ".repeat(padding),
            "│".cyan()
        );
    }

    // ── bottom bar ────────────────────────────────────────
    println!("{}", format!("╰{}╯", "─".repeat(inner)).cyan());
}

/// Minimal inline badge  `[label]` coloured accent.
pub fn badge(label: &str) -> ColoredString {
    format!("[{}]", label).cyan().bold()
}

// ─── Banner ──────────────────────────────────────────────────

/// The big intro card printed at startup / menu return.
pub fn print_banner() {
    let version = env!("CARGO_PKG_VERSION");

    // gradient-style separator using block characters
    let bar = "▓▓▓▒▒▒░░░ ZenClaw ░░░▒▒▒▓▓▓";

    println!();
    println!("{}", "  ╔════════════════════════════════════════════╗".cyan());
    println!(
        "{}",
        format!(
            "  ║  ⚡ ZenClaw v{:<29}⚡  ║",
            format!("{} ", version)
        )
        .cyan()
        .bold()
    );
    println!("{}", "  ║     Build AI the simple way  🦀           ║".cyan());
    println!("{}", format!("  ║  {}  ║", bar).cyan().dimmed());
    println!("{}", "  ╚════════════════════════════════════════════╝".cyan());
    println!();
}

// ─── Session Info Card ───────────────────────────────────────

/// Printed at the start of `zenclaw chat`.
pub fn print_session_info(
    provider: &str,
    model: &str,
    tools_count: usize,
    skills: &[String],
) {
    println!(
        "  {} {} {}  {} {} {}  {} {}",
        "Provider".dimmed(),
        "›".dimmed(),
        provider.green().bold(),
        "│".dimmed(),
        "Model".dimmed(),
        "›".dimmed(),
        model.cyan().bold(),
        badge(&format!("tools:{}", tools_count)),
    );

    if !skills.is_empty() {
        println!(
            "  {} {} {}",
            "Skills".dimmed(),
            "›".dimmed(),
            skills.join(", ").yellow()
        );
    }

    println!(
        "  {}",
        "Memory: SQLite  │  History: up/down arrow  │  /help for commands"
            .dimmed()
    );
    println!();
}

// ─── Chat Bubbles ────────────────────────────────────────────

/// Print the AI response prefix — `AI › ` — then flush so the
/// `termimad` output follows on the same first line.
pub fn print_ai_prefix() {
    print!("\n{} ", "AI ›".bright_cyan().bold());
    io::stdout().flush().unwrap_or(());
}

/// Print a short separator after each AI turn.
pub fn print_turn_divider() {
    println!("{}", "  ─────────────────────────────────────".dimmed());
}

// ─── Command Hint after Code Block ───────────────────────────

/// Printed when the AI returns at least one fenced code block.
pub fn print_code_tip() {
    println!(
        "\n  {} {} {} {} {}",
        "💡 Tip:".dimmed(),
        "/copy".bold().cyan(),
        "copies code,".dimmed(),
        "/run".bold().cyan(),
        "executes it directly in your terminal.".dimmed()
    );
    println!();
}

// ─── Help Table ──────────────────────────────────────────────

/// Pretty-printed command reference.
pub fn print_help() {
    let cmds: &[(&str, &str)] = &[
        ("/quit", "Exit ZenClaw"),
        ("/clear", "Clear conversation history"),
        ("/tools", "List all registered tools"),
        ("/model", "Switch AI provider or model on the fly"),
        ("/skills", "List available skill packs"),
        ("/copy", "Copy last code block (or whole reply) to clipboard"),
        ("/run", "Execute last code block in a sub-shell"),
        ("/help", "Show this command reference"),
    ];

    println!();
    println!("{}", "  Commands ─────────────────────────────────────".cyan());
    for (cmd, desc) in cmds {
        println!(
            "  {:12} {}  {}",
            cmd.bold().cyan(),
            "│".dimmed(),
            desc.dimmed()
        );
    }
    println!("{}", "  ──────────────────────────────────────────────".cyan());
    println!();
}

// ─── Model / Provider Status ─────────────────────────────────

/// One-liner status after `/model`.
pub fn print_model_status(provider: &str, model: &str) {
    println!(
        "\n  {} {} {} {} {}",
        "Provider".dimmed(),
        "›".dimmed(),
        provider.green().bold(),
        "│".dimmed(),
        model.cyan().bold()
    );
    println!();
}

// ─── Tools List ──────────────────────────────────────────────

pub fn print_tools_list(names: impl Iterator<Item = String>) {
    println!("\n{}", "  🔧 Registered Tools ───────────────────────".cyan());
    for name in names {
        println!("     {} {}", "•".dimmed(), name.cyan());
    }
    println!();
}

// ─── Utility ─────────────────────────────────────────────────

/// Approximate visible character width ignoring ANSI sequences.
/// We walk the string and skip `ESC[…m` escape runs.
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

/// Build the `termimad` skin used for Markdown rendering.
pub fn make_mad_skin() -> termimad::MadSkin {
    let mut skin = termimad::MadSkin::default();
    skin.set_headers_fg(termimad::crossterm::style::Color::Cyan);
    skin.bold.set_fg(termimad::crossterm::style::Color::Yellow);
    skin.italic.set_fg(termimad::crossterm::style::Color::Green);
    skin.quote_mark.set_fg(termimad::crossterm::style::Color::DarkGrey);
    skin.inline_code.set_fg(termimad::crossterm::style::Color::Magenta);
    skin
}
