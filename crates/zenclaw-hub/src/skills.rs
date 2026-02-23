//! Skills system — markdown-based behavior extensions.
//!
//! Skills are markdown files that provide the agent with specialized knowledge
//! and instructions. They are loaded from the skills directory and injected
//! into the system prompt.

use std::path::{Path, PathBuf};
use tokio::fs;

use zenclaw_core::error::Result;

/// A loaded skill.
#[derive(Debug, Clone)]
pub struct Skill {
    /// Skill identifier (filename without .md).
    pub name: String,
    /// Skill display title (from frontmatter or filename).
    pub title: String,
    /// Skill description (from frontmatter).
    pub description: String,
    /// Full markdown content.
    pub content: String,
    /// File path.
    pub path: PathBuf,
}

/// Skill manager — loads and manages skills from a directory.
pub struct SkillManager {
    skills_dir: PathBuf,
    skills: Vec<Skill>,
}

impl SkillManager {
    /// Create a new skill manager.
    pub fn new(skills_dir: &Path) -> Self {
        Self {
            skills_dir: skills_dir.to_path_buf(),
            skills: Vec::new(),
        }
    }

    /// Load all skills from the skills directory.
    pub async fn load_all(&mut self) -> Result<usize> {
        self.skills.clear();

        if !self.skills_dir.exists() {
            // Create default skills directory
            fs::create_dir_all(&self.skills_dir).await.ok();
            self.create_default_skills().await?;
        }

        let mut entries = match fs::read_dir(&self.skills_dir).await {
            Ok(rd) => rd,
            Err(_) => return Ok(0),
        };

        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("md")
                && let Ok(skill) = self.load_skill(&path).await
            {
                self.skills.push(skill);
            }
        }

        self.skills.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(self.skills.len())
    }

    /// Load a single skill from a file.
    async fn load_skill(&self, path: &Path) -> Result<Skill> {
        let content = fs::read_to_string(path).await?;
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Parse YAML frontmatter
        let (title, description, body) = parse_frontmatter(&content);

        Ok(Skill {
            name: name.clone(),
            title: title.unwrap_or_else(|| name.replace('_', " ")),
            description: description.unwrap_or_default(),
            content: body.to_string(),
            path: path.to_path_buf(),
        })
    }

    /// Create default example skills.
    async fn create_default_skills(&self) -> Result<()> {
        let coding_skill = r#"---
title: Coding Assistant
description: Help with programming tasks, code review, and debugging.
---

# Coding Assistant

You are an expert programmer. When helping with code:

1. **Read before writing** — Always read the existing code first
2. **Explain changes** — Describe what you're changing and why
3. **Best practices** — Follow language conventions and best practices
4. **Error handling** — Always include proper error handling
5. **Testing** — Suggest tests when making changes

## Languages you excel at:
- Rust, Python, JavaScript/TypeScript, Go, C/C++
- SQL, HTML/CSS, Shell scripting

## When reviewing code:
- Check for bugs, security issues, and performance problems
- Suggest improvements with clear explanations
- Be constructive and specific
"#;

        let sysadmin_skill = r#"---
title: System Administrator
description: Help with Linux system administration, DevOps, and infrastructure.
---

# System Administrator

You are an experienced Linux system administrator. When helping:

1. **Safety first** — Always warn about destructive commands
2. **Explain commands** — Break down complex commands
3. **Check before acting** — Verify system state before making changes
4. **Backup** — Suggest backups before risky operations

## Expertise:
- Linux (Ubuntu, Debian, Armbian, Fedora)
- Docker, containers, systemd services
- Networking (iptables, nginx, DNS)
- Monitoring (htop, journalctl, logs)
- Storage (disk management, partitions)
"#;

        let creative_skill = r#"---
title: Creative Writer
description: Help with creative writing, storytelling, and content creation.
---

# Creative Writer

You are a talented creative writer. When creating content:

1. **Engaging** — Write compelling, attention-grabbing content
2. **Clear** — Use clear, concise language
3. **Structured** — Organize content logically
4. **Voice** — Adapt tone and style to the context
"#;

        // Write files
        fs::write(
            self.skills_dir.join("coding.md"),
            coding_skill,
        )
        .await
        .ok();

        fs::write(
            self.skills_dir.join("sysadmin.md"),
            sysadmin_skill,
        )
        .await
        .ok();

        fs::write(
            self.skills_dir.join("creative.md"),
            creative_skill,
        )
        .await
        .ok();

        Ok(())
    }

    /// Get all loaded skills.
    pub fn list(&self) -> &[Skill] {
        &self.skills
    }

    /// Get a skill by name.
    pub fn get(&self, name: &str) -> Option<&Skill> {
        self.skills.iter().find(|s| s.name == name)
    }

    /// Build a system prompt section from active skills.
    pub fn build_prompt(&self, active_skills: &[String]) -> String {
        let mut prompt = String::new();

        for skill_name in active_skills {
            if let Some(skill) = self.get(skill_name) {
                prompt.push_str(&format!("\n\n## Skill: {}\n\n", skill.title));
                prompt.push_str(&skill.content);
            }
        }

        prompt
    }

    /// Get the skills directory path.
    pub fn dir(&self) -> &Path {
        &self.skills_dir
    }
}

/// Parse YAML frontmatter from markdown.
fn parse_frontmatter(content: &str) -> (Option<String>, Option<String>, &str) {
    if !content.starts_with("---") {
        return (None, None, content);
    }

    let after_first = &content[3..];
    if let Some(end) = after_first.find("---") {
        let frontmatter = &after_first[..end];
        let body = &after_first[end + 3..].trim_start();

        let mut title = None;
        let mut description = None;

        for line in frontmatter.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("title:") {
                title = Some(rest.trim().trim_matches('"').trim_matches('\'').to_string());
            }
            if let Some(rest) = line.strip_prefix("description:") {
                description = Some(rest.trim().trim_matches('"').trim_matches('\'').to_string());
            }
        }

        (title, description, body)
    } else {
        (None, None, content)
    }
}
