//! Centralized theme — single source of truth for all TUI colors and styles.
//!
//! Every TUI component imports from here. Changing a color is always a one-file edit.

use ratatui::style::{Color, Modifier, Style};

/// The application-wide color palette.
pub struct Theme {
    // ─── Core brand ──────────────────────────────────
    pub primary: Color,
    pub accent: Color,

    // ─── Semantic ────────────────────────────────────
    pub error: Color,
    pub warning: Color,
    pub success: Color,
    pub info: Color,
    pub muted: Color,

    // ─── Chat roles ──────────────────────────────────
    pub user_msg: Color,
    pub ai_msg: Color,
    pub system_msg: Color,

    // ─── Selection / highlighting ────────────────────
    pub bg_selected: Color,
    pub bg_marked: Color,
    pub bg_hover: Color,

    // ─── Tool use ────────────────────────────────────
    pub tool_pending: Color,
    pub tool_done: Color,
    pub tool_error: Color,

    // ─── Code / markdown ─────────────────────────────
    pub code_bg: Color,
    pub code_fg: Color,
    pub code_keyword: Color,
    pub code_string: Color,
    pub code_comment: Color,
    pub code_number: Color,

    // ─── Log levels ──────────────────────────────────
    pub log_error: Color,
    pub log_warn: Color,
    pub log_info: Color,
    pub log_debug: Color,
    pub log_trace: Color,

    // ─── Borders ─────────────────────────────────────
    pub border: Color,
    pub border_focus: Color,
}

/// The default dark theme used everywhere.
pub static THEME: Theme = Theme {
    // Core brand
    primary:       Color::Cyan,
    accent:        Color::Yellow,

    // Semantic
    error:         Color::Red,
    warning:       Color::Yellow,
    success:       Color::Green,
    info:          Color::Blue,
    muted:         Color::DarkGray,

    // Chat roles
    user_msg:      Color::Yellow,
    ai_msg:        Color::Cyan,
    system_msg:    Color::Red,

    // Selection
    bg_selected:   Color::Rgb(40, 40, 60),
    bg_marked:     Color::Rgb(30, 20, 50),
    bg_hover:      Color::Rgb(35, 35, 50),

    // Tool use
    tool_pending:  Color::DarkGray,
    tool_done:     Color::Green,
    tool_error:    Color::Red,

    // Code / markdown
    code_bg:       Color::Rgb(30, 30, 40),
    code_fg:       Color::Rgb(200, 200, 180),
    code_keyword:  Color::Rgb(198, 120, 221),
    code_string:   Color::Rgb(152, 195, 121),
    code_comment:  Color::Rgb(92, 99, 112),
    code_number:   Color::Rgb(209, 154, 102),

    // Log levels
    log_error:     Color::Red,
    log_warn:      Color::Yellow,
    log_info:      Color::Green,
    log_debug:     Color::Blue,
    log_trace:     Color::Magenta,

    // Borders
    border:        Color::DarkGray,
    border_focus:  Color::Cyan,
};

// ─── Convenience helpers ────────────────────────────────────────────────────

impl Theme {
    /// Bold primary text (headers, titles).
    pub fn title(&self) -> Style {
        Style::default().fg(self.primary).add_modifier(Modifier::BOLD)
    }

    /// Standard muted text (help, footers).
    pub fn hint(&self) -> Style {
        Style::default().fg(self.muted)
    }

    /// Error style.
    pub fn err(&self) -> Style {
        Style::default().fg(self.error).add_modifier(Modifier::BOLD)
    }

    /// Success style.
    pub fn ok(&self) -> Style {
        Style::default().fg(self.success)
    }

    /// Default border style.
    pub fn border_style(&self) -> Style {
        Style::default().fg(self.border)
    }

    /// Focused border style.
    pub fn border_focus_style(&self) -> Style {
        Style::default().fg(self.border_focus)
    }

    /// User message role style.
    pub fn user_role(&self) -> Style {
        Style::default().fg(self.user_msg).add_modifier(Modifier::BOLD)
    }

    /// AI message role style.
    pub fn ai_role(&self) -> Style {
        Style::default().fg(self.ai_msg).add_modifier(Modifier::BOLD)
    }

    /// System/error message role style.
    pub fn system_role(&self) -> Style {
        Style::default().fg(self.system_msg).add_modifier(Modifier::BOLD)
    }

    /// Code block style (foreground on dark bg).
    pub fn code(&self) -> Style {
        Style::default().fg(self.code_fg).bg(self.code_bg)
    }
}
