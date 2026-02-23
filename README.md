<p align="center">
  <h1 align="center">⚡ ZenClaw</h1>
  <p align="center">
    <strong>Build AI the simple way 🦀</strong>
  </p>
  <p align="center">
    Lightweight, open-source AI agent framework for embedded &amp; edge devices.
    <br/>
    One binary. Zero Python. Infinite possibilities.
  </p>
  <p align="center">
    <a href="#installation"><img alt="Install" src="https://img.shields.io/badge/install-cargo-orange?style=for-the-badge&logo=rust"/></a>
    <a href="https://github.com/volumeee/zenclaw/releases"><img alt="Release" src="https://img.shields.io/github/v/release/volumeee/zenclaw?style=for-the-badge"/></a>
    <a href="https://github.com/volumeee/zenclaw/blob/main/LICENSE"><img alt="License" src="https://img.shields.io/badge/license-MIT-blue?style=for-the-badge"/></a>
  </p>
</p>

---

## What is ZenClaw?

ZenClaw is a **lightweight AI agent framework** written in Rust, designed to run on resource-constrained devices like **Set-Top Boxes**, **Raspberry Pi**, and **ARM servers** — places where Python- or Node.js-based agent frameworks are too heavy.

Think of it as a tiny, self-contained AI assistant binary that fits anywhere — from a $10 STB to a cloud server.

### How Does It Compare?

| Metric                | [OpenClaw](https://github.com/openclaw/openclaw) (Node.js) | [PicoClaw](https://github.com/nicholasgasior/picoclaw) (Go) | [Rig](https://github.com/0xPlaygrounds/rig) (Rust) |     **ZenClaw** (Rust)     |
| --------------------- | :--------------------------------------------------------: | :---------------------------------------------------------: | :------------------------------------------------: | :------------------------: |
| **Binary / Install**  |                       ~200MB+ (npm)                        |                           15–25MB                           |                  Library (crate)                   |         **4.7MB**          |
| **Idle RAM**          |                           ~1GB+                            |                            <10MB                            |                   N/A (library)                    |         **~12MB**          |
| **Boot time**         |                           ~3–5s                            |                             <1s                             |                        N/A                         |         **<100ms**         |
| **Language**          |                         TypeScript                         |                             Go                              |                        Rust                        |          **Rust**          |
| **Runtime**           |                        Node.js 22+                         |                            None                             |                        N/A                         |          **None**          |
| **Agent loop**        |                          ✅ ReAct                          |                          ✅ Basic                           |                      ✅ ReAct                      |        **✅ ReAct**        |
| **Tool calling**      |                             ✅                             |                             ✅                              |                         ✅                         |    **✅ (10 built-in)**    |
| **Channels**          |                Telegram, Discord, WhatsApp                 |                          Telegram                           |                         —                          | **CLI, Telegram, Discord** |
| **Persistent memory** |                     ✅ SQLite + Files                      |                          ✅ SQLite                          |                  ✅ Vector stores                  |       **✅ SQLite**        |
| **Plugin system**     |                       ✅ AgentSkills                       |                         ✅ Plugins                          |                     ✅ Crates                      |    **✅ Shell scripts**    |
| **Edge/Embedded**     |                        ❌ Too heavy                        |                             ✅                              |                      ✅ WASM                       |     **✅ Native ARM**      |
| **Self-hosted**       |                             ✅                             |                             ✅                              |                         ✅                         |           **✅**           |
| **Model-agnostic**    |                             ✅                             |                             ✅                              |                         ✅                         |    **✅ (5 providers)**    |

> **Why ZenClaw?** OpenClaw is powerful but requires Node.js and 1GB+ RAM. PicoClaw is lightweight but has limited tooling. Rig is a library, not a turnkey agent. ZenClaw gives you a **complete, production-ready AI agent in a single 4.7MB binary** — with built-in tools, TUI setup wizard, channel adapters, skills, and plugins.

## Features

- 🤖 **Multi-Provider LLM** — OpenAI, Gemini, Ollama, OpenRouter, LM Studio
- 🔧 **13 Built-in Tools** — Shell, filesystem, web, cron, health, history, file indexer
- 🔌 **Plugin System** — Extend with shell scripts, no recompile needed
- 📚 **Skills System** — Markdown-based behavior customization
- 💬 **Channel Adapters** — CLI, Telegram, Discord
- 🌐 **REST API Server** — Expose ZenClaw as an HTTP service (Axum)
- 🧠 **Persistent Memory** — SQLite-backed conversation history
- 🔍 **RAG / Full-Text Search** — SQLite FTS5, index files & search knowledge base
- 🔀 **Multi-Agent Router** — Route messages to specialized sub-agents
- 🔄 **Model Fallback** — Auto-switch models on failure
- ⚡ **Interactive Setup** — Beautiful TUI wizard, configure in seconds
- ⏰ **Task Scheduler** — Schedule delayed commands via cron tool
- 🏥 **Health Monitor** — CPU, RAM, disk, network, temp (edge devices!)
- 🐳 **Docker Ready** — Dockerfile + docker-compose included
- 🎯 **Edge-Ready** — Runs on ARM STB, Raspberry Pi, embedded Linux

## Installation

### Option 1: Download Binary (Recommended)

Download the pre-built binary for your platform from [Releases](https://github.com/volumeee/zenclaw/releases):

```bash
# Linux x86_64
curl -L https://github.com/volumeee/zenclaw/releases/latest/download/zenclaw-linux-x86_64.tar.gz | tar xz
sudo mv zenclaw-linux-x86_64 /usr/local/bin/zenclaw

# Linux ARM64 (Raspberry Pi, STB, Armbian)
curl -L https://github.com/volumeee/zenclaw/releases/latest/download/zenclaw-linux-aarch64.tar.gz | tar xz
sudo mv zenclaw-linux-aarch64 /usr/local/bin/zenclaw

# macOS (Apple Silicon)
curl -L https://github.com/volumeee/zenclaw/releases/latest/download/zenclaw-macos-aarch64.tar.gz | tar xz
sudo mv zenclaw-macos-aarch64 /usr/local/bin/zenclaw

# macOS (Intel)
curl -L https://github.com/volumeee/zenclaw/releases/latest/download/zenclaw-macos-x86_64.tar.gz | tar xz
sudo mv zenclaw-macos-x86_64 /usr/local/bin/zenclaw
```

### Option 2: Install via Cargo

```bash
cargo install --git https://github.com/volumeee/zenclaw.git
```

### Option 3: Build from Source

```bash
git clone https://github.com/volumeee/zenclaw.git
cd zenclaw
cargo build --release
# Binary at: target/release/zenclaw (~4.7MB)
```

## Quick Start

### 1. Setup (Interactive Wizard)

```bash
zenclaw setup
```

The setup wizard lets you:

- **Select provider** with arrow keys (OpenAI, Gemini, Ollama, etc.)
- **Enter API key** with masked input (secrets never shown)
- **Choose model** from provider's available models
- **Save to config** — you never have to pass flags again!

### 2. Chat

```bash
# Interactive chat (uses saved config)
zenclaw chat

# Quick one-shot question
zenclaw ask "Explain quantum computing in 3 sentences"

# Chat with a skill activated
zenclaw chat --skill coding

# Override provider for this session
zenclaw chat --provider ollama --model llama3.2
```

### 3. Manage Configuration

```bash
# Show current config
zenclaw config show

# Set individual values
zenclaw config set provider gemini
zenclaw config set model gemini-2.0-flash
zenclaw config set api_key YOUR_KEY
zenclaw config set telegram_token YOUR_BOT_TOKEN

# Show config file path
zenclaw config path
```

### 4. System Status

```bash
zenclaw status
```

### 5. Telegram Bot

```bash
# Using saved config (after running setup)
zenclaw telegram

# Or with explicit token
zenclaw telegram --token "123456:ABC..."

# Restrict to specific users
zenclaw telegram --allowed-users "123456789,987654321"
```

### 6. Discord Bot

```bash
zenclaw discord --token "YOUR_DISCORD_BOT_TOKEN"
```

## Architecture

```
zenclaw/                              Total: ~6,200 lines of Rust
├── crates/
│   ├── zenclaw-core/                 # Core traits & types
│   │   ├── agent.rs                  # ReAct reasoning engine
│   │   ├── provider.rs               # LLM provider trait
│   │   ├── tool.rs                   # Tool trait & registry
│   │   ├── memory.rs                 # Memory trait + InMemory store
│   │   ├── channel.rs                # Channel adapter trait
│   │   ├── config.rs                 # TOML configuration
│   │   ├── message.rs                # Chat message types
│   │   ├── session.rs                # Session manager
│   │   ├── bus.rs                    # Async event bus
│   │   └── error.rs                  # Error types
│   │
│   ├── zenclaw-hub/                  # Implementations
│   │   ├── providers/
│   │   │   ├── openai.rs             # OpenAI-compatible provider
│   │   │   └── fallback.rs           # Auto model fallback
│   │   ├── tools/
│   │   │   ├── shell.rs              # Execute commands
│   │   │   ├── filesystem.rs         # File operations (CRUD)
│   │   │   ├── web_fetch.rs          # HTTP requests
│   │   │   ├── web_search.rs         # DuckDuckGo search
│   │   │   ├── system_info.rs        # OS/arch info
│   │   │   ├── cron.rs               # Task scheduler
│   │   │   ├── health.rs             # System health monitor
│   │   │   └── history.rs            # Conversation export
│   │   ├── channels/
│   │   │   ├── telegram.rs           # Telegram bot (raw HTTP)
│   │   │   └── discord.rs            # Discord bot (raw HTTP)
│   │   ├── memory/
│   │   │   ├── sqlite.rs             # SQLite memory backend
│   │   │   └── rag.rs                # RAG via SQLite FTS5
│   │   ├── skills.rs                 # Markdown skill system
│   │   ├── plugins.rs                # Shell script plugins
│   │   └── router.rs                 # Multi-agent router
│   │
│   └── zenclaw-cli/                  # Binary
│       ├── main.rs                   # CLI commands & handlers
│       └── setup.rs                  # Interactive wizard
│
├── .github/workflows/
│   ├── ci.yml                        # Test & build on push
│   └── release.yml                   # Auto-release on tag
│
├── Cargo.toml                        # Workspace config
└── README.md                         # This file
```

## Built-in Tools

| Tool          | Description                                                |
| ------------- | ---------------------------------------------------------- |
| `exec`        | Execute shell commands safely                              |
| `read_file`   | Read file contents with line ranges                        |
| `write_file`  | Create or overwrite files                                  |
| `edit_file`   | Search & replace within files                              |
| `list_dir`    | List directory contents with metadata                      |
| `web_fetch`   | HTTP requests (GET/POST/PUT/DELETE)                        |
| `web_search`  | Search the internet via DuckDuckGo (no API key!)           |
| `system_info` | OS, architecture, hostname, user info                      |
| `cron`        | Schedule delayed shell commands                            |
| `health`      | CPU load, memory, disk, network, uptime (edge monitoring!) |
| `history`     | Export/import conversation history (JSON/Markdown)         |
| + **Plugins** | Any shell script can become a tool!                        |

## Plugin System

Create custom tools without recompiling ZenClaw! Add a folder in `~/.local/share/zenclaw/plugins/`:

```
~/.local/share/zenclaw/plugins/
└── my_tool/
    ├── plugin.json     # Tool manifest
    └── run.sh          # Your script
```

**plugin.json:**

```json
{
  "name": "docker_status",
  "description": "Check Docker container status",
  "version": "1.0.0",
  "command": "run.sh",
  "parameters": {
    "type": "object",
    "properties": {
      "container": { "type": "string", "description": "Container name" }
    }
  }
}
```

**run.sh:**

```bash
#!/bin/sh
# Args are passed via $ZENCLAW_ARGS as JSON
CONTAINER=$(echo "$ZENCLAW_ARGS" | grep -o '"container":"[^"]*"' | cut -d'"' -f4)
docker ps --filter "name=$CONTAINER" --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"
```

## Skills

Skills are markdown files that shape the agent's personality and expertise:

```bash
# List available skills
zenclaw skills list

# View a skill
zenclaw skills show coding

# Activate during chat
zenclaw chat --skill coding
zenclaw chat --skill sysadmin
```

Create custom skills as `.md` files in `~/.local/share/zenclaw/skills/`:

```markdown
---
title: DevOps Engineer
description: Expert in Docker, K8s, CI/CD, and infrastructure.
---

# DevOps Engineer

When helping with infrastructure:

1. Always check current state before making changes
2. Suggest docker-compose for multi-service setups
3. Use systemd for service management
4. Monitor with proper health checks
```

## Supported Providers

| Provider          | Models                       | Key Required | Free Tier  |
| ----------------- | ---------------------------- | :----------: | :--------: |
| **OpenAI**        | GPT-4o, GPT-4o-mini          |      ✅      |     ❌     |
| **Google Gemini** | Gemini 2.0 Flash, 1.5 Pro    |      ✅      |     ✅     |
| **OpenRouter**    | 100+ models                  |      ✅      |   Varies   |
| **Ollama**        | Llama 3, Mistral, Phi, Gemma |      ❌      | ✅ (local) |
| **LM Studio**     | Any GGUF model               |      ❌      | ✅ (local) |
| **Custom**        | Any OpenAI-compatible API    |    Varies    |   Varies   |

## Deploy to Edge Devices

### Raspberry Pi / STB (ARM64)

```bash
# On your dev machine — cross-compile
cargo build --release --target aarch64-unknown-linux-gnu

# Copy to device
scp target/aarch64-unknown-linux-gnu/release/zenclaw user@raspberrypi:~/

# On the device
ssh user@raspberrypi
./zenclaw setup          # Interactive wizard
./zenclaw chat           # Start chatting!
./zenclaw telegram       # Or run as Telegram bot
```

Or simply download the pre-built `zenclaw-linux-aarch64` binary from [Releases](https://github.com/volumeee/zenclaw/releases).

### Run as a Systemd Service

```bash
sudo tee /etc/systemd/system/zenclaw.service << 'EOF'
[Unit]
Description=ZenClaw AI Agent
After=network.target

[Service]
Type=simple
User=pi
ExecStart=/usr/local/bin/zenclaw telegram
Restart=always
RestartSec=5
Environment="RUST_LOG=info"

[Install]
WantedBy=multi-user.target
EOF

sudo systemctl enable --now zenclaw
```

## Development

```bash
# Clone
git clone https://github.com/volumeee/zenclaw.git
cd zenclaw

# Build (debug)
cargo build

# Build (release, optimized — ~4.7MB)
cargo build --release

# Run with logging
RUST_LOG=info cargo run -- chat

# Strict lint check (zero warnings)
RUSTFLAGS="-D warnings" cargo build

# Create a new release
git tag v0.1.0
git push origin v0.1.0
# → GitHub Actions auto-builds for 4 platforms!
```

## Roadmap

- [x] Multi-provider LLM (OpenAI, Gemini, Ollama, OpenRouter)
- [x] ReAct agent loop with tool calling
- [x] Persistent memory (SQLite)
- [x] Built-in tools (shell, filesystem, web, cron, health, history, indexer, webhook)
- [x] Interactive setup wizard
- [x] Telegram bot channel
- [x] Discord bot channel
- [x] WhatsApp adapter (via HTTP bridge)
- [x] Markdown skills system
- [x] Shell script plugin system
- [x] Model fallback provider
- [x] GitHub CI/CD (multi-platform builds)
- [x] RAG / Full-text search (SQLite FTS5)
- [x] Multi-agent router
- [x] System health monitoring
- [x] Conversation history export
- [x] REST API server (Axum)
- [x] File indexer for RAG
- [x] Docker support (Dockerfile + compose)
- [x] Auto-update checker
- [x] Webhook receiver
- [ ] Web dashboard
- [ ] ESP32 thin client (no_std)

## License

MIT — Use it however you want. Build amazing things! 🚀

---

<p align="center">
  Built with ❤️ and 🦀 by <a href="https://github.com/volumeee">baguse</a>
</p>
