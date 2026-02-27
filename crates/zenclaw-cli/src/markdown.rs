//! Markdown → ratatui renderer.
//!
//! Converts markdown text into styled `Vec<Line>` for the chat widget.
//! Supports: code fences, bold, italic, inline code, headers, lists, and HR.

use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::theme::THEME;

/// Render markdown text into a sequence of styled `Line`s.
///
/// `width` is used for horizontal rules; pass the available chat width.
pub fn render_markdown(text: &str, width: usize) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut in_code_block = false;
    let mut code_lang = String::new();

    for raw in text.lines() {
        // ── Code fence toggle ──────────────────────────
        if raw.trim_start().starts_with("```") {
            if in_code_block {
                // End code block
                in_code_block = false;
                lines.push(Line::from(Span::styled(
                    format!(" └─ end {} ", if code_lang.is_empty() { "code" } else { &code_lang }),
                    Style::default().fg(THEME.muted),
                )));
                code_lang.clear();
            } else {
                // Start code block
                in_code_block = true;
                code_lang = raw.trim_start().trim_start_matches('`').trim().to_string();
                let label = if code_lang.is_empty() {
                    " ┌─ code ".to_string()
                } else {
                    format!(" ┌─ {} ", code_lang)
                };
                lines.push(Line::from(Span::styled(label, Style::default().fg(THEME.muted))));
            }
            continue;
        }

        if in_code_block {
            // Inside code block → apply code styling
            let styled = style_code_line(raw, &code_lang);
            lines.push(styled);
            continue;
        }

        // ── Horizontal rule ────────────────────────────
        let trimmed = raw.trim();
        if (trimmed.starts_with("---") || trimmed.starts_with("***") || trimmed.starts_with("___"))
            && trimmed.chars().all(|c| c == '-' || c == '*' || c == '_' || c == ' ')
            && trimmed.len() >= 3
        {
            lines.push(Line::from(Span::styled(
                "─".repeat(width.min(60)),
                Style::default().fg(THEME.muted),
            )));
            continue;
        }

        // ── Headers ────────────────────────────────────
        if let Some(rest) = trimmed.strip_prefix("### ") {
            lines.push(Line::from(Span::styled(
                format!("   {}", rest),
                Style::default().fg(THEME.primary).add_modifier(Modifier::ITALIC),
            )));
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("## ") {
            lines.push(Line::from(Span::styled(
                format!("  {}", rest),
                Style::default().fg(THEME.accent).add_modifier(Modifier::BOLD),
            )));
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("# ") {
            lines.push(Line::from(Span::styled(
                rest.to_string(),
                Style::default().fg(THEME.primary).add_modifier(Modifier::BOLD),
            )));
            continue;
        }

        // ── Bullet lists ───────────────────────────────
        if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            let indent = raw.len() - raw.trim_start().len();
            let pad = " ".repeat(indent);
            let body = &trimmed[2..];
            let mut spans = vec![Span::styled(
                format!("{}  • ", pad),
                Style::default().fg(THEME.muted),
            )];
            spans.extend(parse_inline_markdown(body));
            lines.push(Line::from(spans));
            continue;
        }

        // ── Numbered lists ─────────────────────────────
        if let Some(pos) = trimmed.find(". ") {
            if pos <= 3 && trimmed[..pos].chars().all(|c| c.is_ascii_digit()) {
                let body = &trimmed[pos + 2..];
                let num = &trimmed[..pos];
                let indent = raw.len() - raw.trim_start().len();
                let pad = " ".repeat(indent);
                let mut spans = vec![Span::styled(
                    format!("{}  {}. ", pad, num),
                    Style::default().fg(THEME.muted),
                )];
                spans.extend(parse_inline_markdown(body));
                lines.push(Line::from(spans));
                continue;
            }
        }

        // ── Regular paragraph with inline markdown ─────
        if trimmed.is_empty() {
            lines.push(Line::from(""));
        } else {
            lines.push(Line::from(parse_inline_markdown(raw)));
        }
    }

    // Close unclosed code block
    if in_code_block {
        lines.push(Line::from(Span::styled(
            " └─ end code ".to_string(),
            Style::default().fg(THEME.muted),
        )));
    }

    lines
}

/// Parse inline markdown: **bold**, *italic*, `inline code`.
fn parse_inline_markdown(text: &str) -> Vec<Span<'static>> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut chars = text.char_indices().peekable();
    #[allow(unused_variables)]
    let mut current = String::new();
    let default_style = Style::default();

    while let Some((_i, ch)) = chars.next() {
        match ch {
            '`' => {
                // Inline code
                if !current.is_empty() {
                    spans.push(Span::styled(current.clone(), default_style));
                    current.clear();
                }
                let mut code = String::new();
                for (_, c) in chars.by_ref() {
                    if c == '`' { break; }
                    code.push(c);
                }
                spans.push(Span::styled(
                    format!(" {} ", code),
                    Style::default().fg(THEME.code_fg).bg(THEME.code_bg),
                ));
            }
            '*' | '_' => {
                // Check for bold (**) or italic (*)
                let next_is_same = chars.peek().map_or(false, |(_, nc)| *nc == ch);
                if next_is_same {
                    // Bold
                    chars.next(); // consume second *
                    if !current.is_empty() {
                        spans.push(Span::styled(current.clone(), default_style));
                        current.clear();
                    }
                    let mut bold_text = String::new();
                    while let Some((_, c)) = chars.next() {
                        if c == ch {
                            if chars.peek().map_or(false, |(_, nc)| *nc == ch) {
                                chars.next();
                                break;
                            }
                        }
                        bold_text.push(c);
                    }
                    spans.push(Span::styled(
                        bold_text,
                        Style::default().add_modifier(Modifier::BOLD),
                    ));
                } else {
                    // Italic
                    if !current.is_empty() {
                        spans.push(Span::styled(current.clone(), default_style));
                        current.clear();
                    }
                    let mut italic_text = String::new();
                    for (_, c) in chars.by_ref() {
                        if c == ch { break; }
                        italic_text.push(c);
                    }
                    spans.push(Span::styled(
                        italic_text,
                        Style::default().add_modifier(Modifier::ITALIC),
                    ));
                }
            }
            _ => {
                current.push(ch);
            }
        }
    }

    if !current.is_empty() {
        spans.push(Span::styled(current, default_style));
    }

    // If nothing parsed, return a single empty span
    if spans.is_empty() {
        spans.push(Span::raw(""));
    }

    spans
}

/// Basic syntax coloring for code lines.
fn style_code_line(line: &str, lang: &str) -> Line<'static> {
    let base_style = THEME.code();
    let line_str = format!("  {}", line);

    // Simple keyword highlighting for common languages
    let keywords = match lang {
        "rust" | "rs" => &[
            "fn", "let", "mut", "pub", "use", "mod", "struct", "enum", "impl",
            "trait", "async", "await", "match", "if", "else", "for", "while",
            "return", "self", "Self", "crate", "super", "where", "type", "const",
            "static", "ref", "move", "true", "false", "Some", "None", "Ok", "Err",
        ][..],
        "python" | "py" => &[
            "def", "class", "import", "from", "if", "else", "elif", "for",
            "while", "return", "self", "True", "False", "None", "with", "as",
            "try", "except", "raise", "yield", "async", "await", "lambda",
        ][..],
        "javascript" | "js" | "typescript" | "ts" => &[
            "function", "const", "let", "var", "if", "else", "for", "while",
            "return", "class", "import", "export", "from", "async", "await",
            "new", "this", "true", "false", "null", "undefined", "try", "catch",
        ][..],
        _ => &[][..],
    };

    if keywords.is_empty() {
        return Line::from(Span::styled(line_str, base_style));
    }

    // Tokenize and highlight
    let mut spans: Vec<Span<'static>> = vec![Span::styled("  ", base_style)];


    // Simple word-by-word highlighting
    let words: Vec<&str> = line.split_inclusive(|c: char| !c.is_alphanumeric() && c != '_')
        .collect();

    spans.clear();
    spans.push(Span::styled("  ".to_string(), base_style));

    for word in words {
        let trimmed = word.trim();
        if keywords.contains(&trimmed) {
            // Keyword
            let non_kw_suffix: String = word.chars().skip(trimmed.len()).collect();
            spans.push(Span::styled(
                trimmed.to_string(),
                Style::default().fg(THEME.code_keyword).bg(THEME.code_bg),
            ));
            if !non_kw_suffix.is_empty() {
                spans.push(Span::styled(non_kw_suffix, base_style));
            }
        } else if trimmed.starts_with('"') || trimmed.starts_with('\'') {
            spans.push(Span::styled(
                word.to_string(),
                Style::default().fg(THEME.code_string).bg(THEME.code_bg),
            ));
        } else if trimmed.starts_with("//") || trimmed.starts_with('#') {
            spans.push(Span::styled(
                word.to_string(),
                Style::default().fg(THEME.code_comment).bg(THEME.code_bg),
            ));
        } else if trimmed.chars().all(|c| c.is_ascii_digit() || c == '.') && !trimmed.is_empty() {
            spans.push(Span::styled(
                word.to_string(),
                Style::default().fg(THEME.code_number).bg(THEME.code_bg),
            ));
        } else {
            spans.push(Span::styled(word.to_string(), base_style));
        }
    }

    Line::from(spans)
}
