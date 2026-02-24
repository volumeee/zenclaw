# Contributing to ZenClaw

Thank you for your interest in contributing to ZenClaw! This project is a community-driven effort to build the lightest, fastest, and most versatile AI agent framework for embedded and edge devices. We welcome contributions of all kinds: bug fixes, features, documentation, new tools, channel adapters, LLM providers, and testing on exotic hardware.

ZenClaw itself was substantially developed with AI assistance — we embrace this approach and have built our contribution process around it.

---

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Ways to Contribute](#ways-to-contribute)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Making Changes](#making-changes)
- [AI-Assisted Contributions](#ai-assisted-contributions)
- [Pull Request Process](#pull-request-process)
- [Branch Strategy](#branch-strategy)
- [Code Review](#code-review)
- [Communication](#communication)

---

## Code of Conduct

We are committed to maintaining a welcoming and respectful community. Be kind, constructive, and assume good faith. Harassment or discrimination of any kind will not be tolerated.

---

## Ways to Contribute

- **Bug reports** — Open an issue using the bug report template.
- **Feature requests** — Open an issue first to discuss before implementing; this prevents wasted effort.
- **Code** — Fix bugs or implement features (new tools, channel adapters, LLM providers, memory backends).
- **Documentation** — Improve READMEs, inline comments, or usage examples.
- **Testing** — Run ZenClaw on new hardware (Raspberry Pi, STB, RISC-V), new LLM providers, or new channels and report your results.
- **Node.js bridge** — Improve `bridge/bridge.js` (WhatsApp) or `bridge/scrape.js` (headless scraping).

> For substantial new features, please **open an issue first** to discuss the design before writing code. This prevents wasted effort and ensures alignment with the project's direction.

---

## Getting Started

1. **Fork** the repository on GitHub.
2. **Clone** your fork locally:
   ```bash
   git clone https://github.com/<your-username>/zenclaw.git
   cd zenclaw
   ```
3. **Add the upstream remote:**
   ```bash
   git remote add upstream https://github.com/volumeee/zenclaw.git
   ```

---

## Development Setup

### Prerequisites

| Tool    | Version             | Required for                         |
| ------- | ------------------- | ------------------------------------ |
| Rust    | 1.83+               | Core binary                          |
| Cargo   | (bundled with Rust) | Build & test                         |
| Node.js | 18+                 | `bridge/` only (WhatsApp & scraping) |
| SQLite  | any                 | Tests (bundled via `rusqlite`)       |

Install Rust via [rustup](https://rustup.rs/):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Build

```bash
cargo build              # Dev build (unoptimized)
cargo build --release    # Release build → target/release/zenclaw (~5.1MB)
```

### Run dev binary directly

```bash
cargo run -- chat          # Interactive chat
cargo run -- ask "Hello"   # One-shot
cargo run -- serve         # REST API server
```

### Bridge setup (optional — only for WhatsApp & web scraping)

```bash
cd bridge/
npm install
node bridge.js   # WhatsApp bridge (scan QR first time)
# Separate terminal:
node scrape.js https://example.com   # Test scraper standalone
```

### Running Tests

```bash
cargo test --workspace                    # Run all tests
cargo test -p zenclaw-core               # Single crate
cargo test memory::sqlite::tests         # Single module
```

### Code Style Checks

```bash
cargo check --workspace                  # Type-check fast
cargo clippy --workspace -- -D warnings  # Linter (must pass, no warnings)
cargo fmt --all -- --check               # Format check
cargo fmt --all                          # Auto-format
```

> **All CI checks must pass before a PR can be merged.** Run `cargo clippy --workspace -- -D warnings` locally before pushing to catch issues early.

---

## Making Changes

### Branching

Always branch off `main` and target `main` in your PR. **Never push directly to `main`.**

```bash
git checkout main
git pull upstream main
git checkout -b your-feature-branch
```

Use descriptive branch names, e.g.:

- `fix/telegram-timeout`
- `feat/anthropic-provider`
- `docs/contributing-guide`
- `tool/browser-control`

### Commits

- Write clear, concise commit messages in **English**.
- Use the **imperative mood**: `"Add retry logic"` not `"Added retry logic"`.
- Follow [Conventional Commits](https://www.conventionalcommits.org/):

  ```
  feat: add Anthropic (Claude) provider
  fix: handle tool timeout in Discord channel
  docs: expand RAG setup guide
  refactor: extract setup_bot_env factory
  chore: bump version to v0.2.0
  ```

- Reference the related issue when relevant: `Fix session leak (#42)`.
- Keep commits focused. One logical change per commit is preferred.

### Version Bumping & Releases

**Do not bump the version in `Cargo.toml` unless you intend to trigger a release.** The CI auto-tags and auto-releases when it detects a version change:

```toml
# Cargo.toml — workspace version
version = "0.2.0"   # Changing this → CI auto-creates tag v0.2.0 and builds all platforms
```

Only maintainers bump the version. If you think a release is warranted, mention it in your PR or issue.

### Adding a New Tool

1. Create `crates/zenclaw-hub/src/tools/your_tool.rs`
2. Implement the `Tool` trait from `zenclaw-core`:

   ```rust
   use async_trait::async_trait;
   use serde_json::{json, Value};
   use zenclaw_core::error::Result;
   use zenclaw_core::tool::Tool;

   pub struct YourTool;

   #[async_trait]
   impl Tool for YourTool {
       fn name(&self) -> &str { "your_tool" }
       fn description(&self) -> &str { "What this tool does" }
       fn parameters(&self) -> Value { json!({ "type": "object", "properties": {} }) }
       async fn execute(&self, args: Value) -> Result<String> { Ok("result".into()) }
   }
   ```

3. Register in `crates/zenclaw-hub/src/tools/mod.rs`
4. Add to the tool registry in `crates/zenclaw-cli/src/main.rs` → `build_agent()`

### Adding a New LLM Provider

Implement `LlmProvider` from `zenclaw-core::provider` and add a constructor variant in `crates/zenclaw-hub/src/providers/openai.rs` (or create a new file). Update `create_provider()` in the CLI.

### Adding a New Channel

Implement the channel logic in `crates/zenclaw-hub/src/channels/your_channel.rs` and add a corresponding `run_your_channel()` command in `crates/zenclaw-cli/src/main.rs`.

### Keeping Up to Date

Rebase your branch onto upstream `main` before opening a PR:

```bash
git fetch upstream
git rebase upstream/main
```

---

## AI-Assisted Contributions

ZenClaw was built with substantial AI assistance, and we fully embrace AI-assisted development. However, contributors must understand their responsibilities when using AI tools.

### Disclosure Is Required

Every PR must disclose AI involvement in the PR description. There are three levels:

| Level                       | Description                                                       |
| --------------------------- | ----------------------------------------------------------------- |
| 🤖 **Fully AI-generated**   | AI wrote the code; contributor reviewed and validated it          |
| 🛠️ **Mostly AI-generated**  | AI produced the draft; contributor made significant modifications |
| 👨‍💻 **Mostly Human-written** | Contributor led; AI provided suggestions or none at all           |

Honest disclosure is expected. There is **no stigma** attached to any level — what matters is the quality of the contribution.

### You Are Responsible for What You Submit

Using AI to generate code does **not** reduce your responsibility as the contributor. Before opening a PR with AI-generated code, you must:

- **Read and understand every line** of the generated code.
- **Test it** in a real environment (see Test Environment below).
- **Check for security issues** — AI models can generate subtly insecure code (e.g., path traversal, shell injection, credential exposure). Review carefully.
- **Verify correctness** — AI-generated logic can be plausible-sounding but wrong. Validate the behavior, not just the syntax.

PRs where it is clear the contributor has not read or tested the AI-generated code **will be closed without review.**

### AI-Generated Code Quality Standards

AI-generated contributions are held to the **same quality bar** as human-written code:

- Must pass `cargo clippy --workspace -- -D warnings` (zero warnings).
- Must be idiomatic Rust and consistent with the existing codebase style.
- Must not introduce unnecessary abstractions, dead code, or over-engineering.
- Must include or update tests where appropriate.

### Security Review

AI-generated code requires extra security scrutiny. Pay special attention to:

- **File path handling** — sandbox escapes via `..` or absolute paths (see `resolve_path()` in `filesystem.rs`)
- **Shell injection** — inputs passed to `exec` or shell commands
- **External input validation** — especially in channel handlers and tool `execute()` methods
- **Credential handling** — API keys, tokens must never be logged or exposed
- **RAG data** — user-indexed content must not leak across sessions

If you are unsure whether a piece of AI-generated code is safe, **say so in the PR** — reviewers will help.

---

## Pull Request Process

### Before Opening a PR

- [ ] Run `cargo clippy --workspace -- -D warnings` — zero warnings required.
- [ ] Run `cargo test --workspace` — all tests pass.
- [ ] Run `cargo fmt --all -- --check` — code is formatted.
- [ ] Fill in the PR description completely, including the **AI disclosure section**.
- [ ] Link any related issue(s).
- [ ] Keep the PR focused. Avoid bundling unrelated changes.

### PR Description Template

```markdown
## Description

<!-- What does this change do and why? -->

## Type of Change

- [ ] Bug fix
- [ ] New feature (tool / provider / channel)
- [ ] Documentation
- [ ] Refactor / cleanup
- [ ] Performance improvement

## 🤖 AI Code Generation

<!-- Required — pick one -->

- [ ] 🤖 Fully AI-generated (reviewed and validated by me)
- [ ] 🛠️ Mostly AI-generated (I made significant modifications)
- [ ] 👨‍💻 Mostly Human-written

## Related Issue

Closes #

## Test Environment

- OS: (e.g. Ubuntu 24.04, Raspberry Pi OS Bookworm)
- Architecture: (e.g. x86_64, aarch64)
- Provider: (e.g. Gemini Flash, Ollama/Llama3)
- Channel tested: (e.g. CLI, Telegram, REST API)

## Evidence

<!-- Optional: logs, screenshots, or terminal output showing it works -->

## Checklist

- [ ] I have read every line of added/changed code
- [ ] I have tested the change in a live environment
- [ ] I have checked for security issues in any AI-generated code
- [ ] CI checks pass locally
```

### PR Size

Prefer **small, reviewable PRs**. A PR that changes 200 lines across 5 files is much easier to review than one that changes 2000 lines across 30 files. If your feature is large, consider splitting it into a series of smaller, logically complete PRs.

---

## Branch Strategy

### Long-Lived Branches

- **`main`** — active development branch. All feature PRs target `main`. Protected: direct pushes are not permitted.
- **`release/x.y`** — stable release branches, cut from `main` when a version is ready. More strictly protected.

### Requirements to Merge into `main`

A PR can only be merged when **all** of the following are satisfied:

1. ✅ **CI passes** — Check, lint (`clippy`), test, and build workflows are green.
2. ✅ **Reviewer approval** — At least one maintainer has approved the PR.
3. ✅ **No unresolved comments** — All review threads are resolved.
4. ✅ **PR description is complete** — Including AI disclosure.

### Who Can Merge

Only **maintainers** can merge PRs. Contributors cannot merge their own PRs.

### Merge Strategy

We use **squash merge** for most PRs to keep the `main` history clean and readable. Each merged PR becomes a single commit referencing the PR number:

```
feat: Add Anthropic Claude provider (#42)
```

### Release Branches

When a version is ready, maintainers cut a `release/x.y` branch from `main`:

- **New features are not backported.** Release branches receive no new functionality after being cut.
- **Security fixes and critical bug fixes** are cherry-picked. If a fix in `main` qualifies, maintainers will cherry-pick it onto the affected `release/x.y` branch and issue a patch release.

If you believe a fix should be backported, note it in the PR description or open a separate issue.

---

## Code Review

### For Contributors

- Respond to review comments within a reasonable time. If you need more time, say so.
- When you update a PR in response to feedback, briefly note what changed.
- If you disagree with feedback, engage respectfully — reviewers can be wrong too.
- **Do not force-push after a review has started.** Use additional commits instead; the maintainer will squash on merge.

### For Reviewers

Review for:

- **Correctness** — Does the code do what it claims? Are there edge cases?
- **Security** — Especially for AI-generated code, tool implementations, and channel handlers.
- **Architecture** — Is the approach consistent with the existing design?
- **Simplicity** — Is there a simpler solution? Does this add unnecessary complexity?
- **Tests** — Are the changes covered by tests? Are existing tests still meaningful?

Be constructive and specific: _"This could panic if `args["url"]` is not a string — use `.unwrap_or("")` or return an error"_ is better than _"this looks wrong"_.

---

## Communication

- **GitHub Issues** — Bug reports, feature requests, design discussions.
- **GitHub Discussions** — General questions, ideas, community conversation.
- **Pull Request comments** — Code-specific feedback.

When in doubt, **open an issue before writing code.** It costs little and prevents wasted effort.

---

## A Note on ZenClaw's AI-Driven Origin

ZenClaw's architecture was substantially designed and implemented with AI assistance, guided by human oversight. If you find something that looks odd or over-engineered, it may be an artifact of that process — opening an issue to discuss it is always welcome.

We believe AI-assisted development done responsibly produces great results. We also believe humans must remain accountable for what they ship. These two beliefs are not in conflict.

**Thank you for contributing! 🦀**
