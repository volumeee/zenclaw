<p align="center">
  <img src="https://img.shields.io/badge/⚡-ZenClaw-blueviolet?style=for-the-badge&logoColor=white" alt="ZenClaw" height="40"/>
</p>

<h3 align="center">Build AI the simple way 🦀</h3>

<p align="center">
  Lightweight, open-source AI agent framework for embedded &amp; edge devices.<br/>
  One binary. Zero Python. Infinite possibilities.
</p>

<p align="center">
  <a href="#-quick-start"><img alt="Get Started" src="https://img.shields.io/badge/Get%20Started-→-success?style=flat-square"/></a>
  <a href="https://github.com/volumeee/zenclaw/releases"><img alt="Release" src="https://img.shields.io/github/v/release/volumeee/zenclaw?style=flat-square&color=blue"/></a>
  <a href="https://github.com/volumeee/zenclaw/blob/main/LICENSE"><img alt="License" src="https://img.shields.io/badge/license-MIT-green?style=flat-square"/></a>
  <a href="https://github.com/volumeee/zenclaw/actions"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/volumeee/zenclaw/ci.yml?style=flat-square&label=CI"/></a>
  <img alt="Rust" src="https://img.shields.io/badge/rust-1.83+-orange?style=flat-square&logo=rust"/>
  <img alt="Binary Size" src="https://img.shields.io/badge/binary-5.1MB-blueviolet?style=flat-square"/>
</p>

---

## Why ZenClaw?

ZenClaw is part of a larger, modular AI ecosystem. While other frameworks try to do everything in one heavy package, we split responsibilities into laser-focused, high-performance tools:

### The Volumeee AI Ecosystem

|                 | [**ZenClaw**](https://github.com/volumeee/zenclaw) | [**OpenClaw**](https://github.com/volumeee/openclaw) | [**Kanbot Search**](https://github.com/volumeee/kanbot-search) | [**Claw Studio**](https://github.com/volumeee/claw-studio) |
| --------------- | :------------------------------------------------: | :--------------------------------------------------: | :------------------------------------------------------------: | :--------------------------------------------------------: |
| **Role**        |              Edge AI Agent Framework               |                AI API Gateway & Auth                 |                    Intelligent Search Agent                    |                   Visual Workflow Editor                   |
| **Tech Stack**  |                      Rust 🦀                       |                  TypeScript (Hono)                   |                        Python (FastAPI)                        |                           Vue.js                           |
| **Primary Use** |           Executing tasks, tools, memory           |            Model routing, load balancing             |                     Deep Web Search & RAG                      |                 Designing agents visually                  |
| **Deployment**  |            Native Binary (Linux/Mac/Pi)            |             Cloudflare Workers / Docker              |                       Docker / Cloud Run                       |                     Static Web Hosting                     |
| **Footprint**   |            **5.1MB binary, ~12MB RAM**             |                   Edge/Serverless                    |                       Container (~500MB)                       |                       Browser-based                        |
| **Superpower**  |            Extremely fast & lightweight            |             Unified API for 100+ models              |                  High-quality search results                   |                     No-code AI builder                     |

> **ZenClaw** focuses specifically on the **Agent Engine and Tool Execution**. It consumes models (often via **OpenClaw**), can perform dynamic searches (optionally delegating to **Kanbot Search**), and its behavior can be designed in **Claw Studio**.

> **ZenClaw** gives you a production-ready AI agent in a **single 5.1MB binary** — with built-in tools, channels, RAG, and a REST API. Deploy it on a $10 Set-Top Box or a $5 Raspberry Pi Zero.

---

## ✨ Features

<table>
<tr>
<td width="50%">

### 🤖 Agent Engine

- **ReAct reasoning loop** — think → act → observe
- **5 LLM providers** — OpenAI, Gemini, Ollama, OpenRouter, LM Studio
- **Auto-fallback** — switch models on failure
- **Multi-agent router** — specialized sub-agents

</td>
<td width="50%">

### 🔧 15 Built-in Tools

- Shell execution, file I/O, directory listing
- Web fetch (HTTP), web search (DuckDuckGo)
- Cron scheduler, system info, health monitor
- History export, file indexer, env inspector
- Webhook receiver + extensible plugins

</td>
</tr>
<tr>
<td>

### 📡 5 Channel Adapters

- **CLI** — interactive terminal chat
- **REST API** — HTTP endpoints (Axum)
- **Telegram** — bot via raw HTTP
- **Discord** — bot via gateway
- **WhatsApp** — via HTTP bridge

</td>
<td>

### 🧠 Intelligence

- **RAG** — SQLite FTS5 full-text search
- **File indexer** — index codebases for context
- **Persistent memory** — SQLite conversation history
- **Skills** — Markdown-based behavior customization
- **Plugins** — shell scripts as tools

</td>
</tr>
<tr>
<td>

### 🔒 Production Ready

- **Rate limiting** — 60 req/min per client
- **API key auth** — Bearer token or X-API-Key
- **Request logging** — method, path, status, timing
- **Runtime metrics** — requests, tokens, tool calls
- **Auto-updater** — checks GitHub releases

</td>
<td>

### 🐳 Deploy Anywhere

- **Docker** — Dockerfile + compose included
- **Systemd** — service file template
- **ARM64** — native Raspberry Pi support
- **Cross-compile** — x86_64, aarch64, macOS
- **GitHub CI/CD** — auto-build on push

</td>
</tr>
</table>

---

## 🏗️ Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│                        ZenClaw Runtime                          │
│                                                                  │
│  ┌─────────────┐   ┌──────────────┐   ┌───────────────────────┐ │
│  │   Channels   │   │  Agent Core  │   │       Tools           │ │
│  │             │   │              │   │                       │ │
│  │  • CLI      │──▶│  ReAct Loop  │──▶│  • exec (shell)      │ │
│  │  • REST API │   │              │   │  • read/write/edit    │ │
│  │  • Telegram │   │  ┌────────┐  │   │  • list_dir           │ │
│  │  • Discord  │   │  │ Router │  │   │  • web_fetch          │ │
│  │  • WhatsApp │   │  └────────┘  │   │  • web_search         │ │
│  └─────────────┘   │              │   │  • cron               │ │
│                    │  ┌────────┐  │   │  • system_info        │ │
│  ┌─────────────┐   │  │ Skills │  │   │  • health             │ │
│  │  Providers  │   │  └────────┘  │   │  • history            │ │
│  │             │   └──────────────┘   │  • index_file         │ │
│  │  • OpenAI   │          │           │  • webhooks           │ │
│  │  • Gemini   │          ▼           │  • env                │ │
│  │  • Ollama   │   ┌──────────────┐   │  • + plugins          │ │
│  │  • Router   │   │    Memory    │   └───────────────────────┘ │
│  │  • LMStudio │   │              │                             │
│  └─────────────┘   │  • SQLite    │   ┌───────────────────────┐ │
│                    │  • RAG/FTS5  │   │     Middleware         │ │
│  ┌─────────────┐   │  • InMemory  │   │  • Rate limiter       │ │
│  │  Plugins    │   └──────────────┘   │  • API key auth       │ │
│  │  (shell     │                      │  • Request logging     │ │
│  │   scripts)  │   ┌──────────────┐   │  • Metrics             │ │
│  └─────────────┘   │   Updater    │   └───────────────────────┘ │
│                    └──────────────┘                              │
└──────────────────────────────────────────────────────────────────┘
```

### Crate Structure

```
zenclaw/                                    7,758 lines of Rust
├── crates/
│   ├── zenclaw-core/                       Core abstractions
│   │   ├── agent.rs                        ReAct reasoning engine
│   │   ├── provider.rs                     LLM provider trait
│   │   ├── tool.rs                         Tool trait & registry
│   │   ├── memory.rs                       Memory trait + InMemory
│   │   ├── channel.rs                      Channel adapter trait
│   │   ├── config.rs                       TOML configuration
│   │   ├── message.rs                      Chat message types
│   │   ├── session.rs                      Session manager
│   │   ├── bus.rs                          Async event bus
│   │   └── error.rs                        Error types
│   │
│   ├── zenclaw-hub/                        Full implementations
│   │   ├── api.rs                          REST API server (Axum)
│   │   ├── middleware.rs                   Rate limit, auth, logging
│   │   ├── metrics.rs                      Runtime metrics collector
│   │   ├── router.rs                       Multi-agent router
│   │   ├── updater.rs                      Auto-update checker
│   │   ├── skills.rs                       Markdown skill system
│   │   ├── plugins.rs                      Shell script plugins
│   │   ├── providers/
│   │   │   ├── openai.rs                   OpenAI-compatible
│   │   │   └── fallback.rs                 Auto model fallback
│   │   ├── channels/
│   │   │   ├── telegram.rs                 Telegram (raw HTTP)
│   │   │   ├── discord.rs                  Discord (gateway)
│   │   │   └── whatsapp.rs                 WhatsApp (HTTP bridge)
│   │   ├── memory/
│   │   │   ├── sqlite.rs                   SQLite backend
│   │   │   └── rag.rs                      RAG via FTS5
│   │   └── tools/                          15 built-in tools
│   │       ├── shell.rs                    Execute commands
│   │       ├── filesystem.rs               File CRUD
│   │       ├── web_fetch.rs                HTTP requests
│   │       ├── web_search.rs               DuckDuckGo search
│   │       ├── system_info.rs              OS/arch info
│   │       ├── cron.rs                     Task scheduler
│   │       ├── health.rs                   System diagnostics
│   │       ├── history.rs                  Conversation export
│   │       ├── indexer.rs                  File → RAG indexer
│   │       ├── webhook.rs                  Webhook receiver
│   │       └── env.rs                      Env var inspector
│   │
│   └── zenclaw-cli/                        Binary entry point
│       ├── main.rs                         CLI commands (11 commands)
│       └── setup.rs                        Interactive TUI wizard
│
├── Dockerfile                              Multi-stage build
├── docker-compose.yml                      One-command deploy
├── .github/workflows/
│   ├── ci.yml                              Test & build on push
│   └── release.yml                         Auto-release on tag
└── README.md
```

---

## 🚀 Quick Start

### Install

```bash
# Option 1: Pre-built binary (recommended)
curl -L https://github.com/volumeee/zenclaw/releases/latest/download/zenclaw-linux-x86_64.tar.gz | tar xz
sudo mv zenclaw /usr/local/bin/

# Option 2: Cargo
cargo install --git https://github.com/volumeee/zenclaw.git

# Option 3: Build from source
git clone https://github.com/volumeee/zenclaw.git && cd zenclaw
cargo build --release  # → target/release/zenclaw (5.1MB)
```

### Setup

```bash
zenclaw setup    # Interactive wizard — pick provider, enter API key, choose model
```

### Chat

```bash
zenclaw chat                          # Interactive session
zenclaw ask "Explain Rust lifetimes"  # One-shot question
zenclaw chat --skill coding           # With coding skill active
```

---

## 📡 Deployment Modes

### CLI (Default)

```bash
zenclaw chat
```

### REST API Server

```bash
zenclaw serve --port 3000

# Chat endpoint
curl -X POST http://localhost:3000/v1/chat \
  -H "Content-Type: application/json" \
  -d '{"message": "Hello!", "session": "user1"}'

# Health check
curl http://localhost:3000/v1/health

# RAG search
curl -X POST http://localhost:3000/v1/rag/search \
  -d '{"query": "deployment guide", "limit": 5}'
```

### Telegram Bot

```bash
zenclaw telegram --token "123456:ABC..."
# or: set telegram_token in config, then just run:
zenclaw telegram
```

### Discord Bot

```bash
zenclaw discord --token "YOUR_DISCORD_TOKEN"
```

### WhatsApp Bot

```bash
# Requires a Baileys HTTP bridge running separately
zenclaw whatsapp --bridge http://localhost:3001
```

### Docker

```bash
# Using docker-compose
docker compose up -d

# Or manually
docker build -t zenclaw .
docker run -p 3000:3000 -e GEMINI_API_KEY=your-key zenclaw
```

### Systemd Service

```bash
sudo tee /etc/systemd/system/zenclaw.service << 'EOF'
[Unit]
Description=ZenClaw AI Agent
After=network.target

[Service]
Type=simple
User=pi
ExecStart=/usr/local/bin/zenclaw serve --host 0.0.0.0 --port 3000
Restart=always
Environment="RUST_LOG=info"
Environment="GEMINI_API_KEY=your-key"

[Install]
WantedBy=multi-user.target
EOF

sudo systemctl enable --now zenclaw
```

---

## 🔧 Built-in Tools

| Tool          | Description                                  |
| ------------- | -------------------------------------------- |
| `exec`        | Execute shell commands with output capture   |
| `read_file`   | Read file contents with optional line ranges |
| `write_file`  | Create or overwrite files                    |
| `edit_file`   | Search & replace within files                |
| `list_dir`    | List directory contents with metadata        |
| `web_fetch`   | HTTP requests (GET/POST/PUT/DELETE)          |
| `web_search`  | Search the internet via DuckDuckGo           |
| `system_info` | OS, architecture, hostname, user info        |
| `cron`        | Schedule delayed shell commands              |
| `health`      | CPU, memory, disk, network, uptime           |
| `history`     | Export conversations (JSON/Markdown)         |
| `index_file`  | Index files into RAG knowledge base          |
| `webhooks`    | Inspect received webhook events              |
| `env`         | Check environment variables & API keys       |
| + **Plugins** | Any shell script can become a tool           |

---

## 📚 Plugin System

Create tools without recompiling. Drop a folder in `~/.local/share/zenclaw/plugins/`:

```
my_tool/
├── plugin.json    # Manifest
└── run.sh         # Your script
```

**plugin.json:**

```json
{
  "name": "docker_status",
  "description": "Check Docker container status",
  "command": "run.sh",
  "parameters": {
    "type": "object",
    "properties": {
      "container": { "type": "string" }
    }
  }
}
```

**run.sh:**

```bash
#!/bin/sh
CONTAINER=$(echo "$ZENCLAW_ARGS" | grep -o '"container":"[^"]*"' | cut -d'"' -f4)
docker ps --filter "name=$CONTAINER" --format "table {{.Names}}\t{{.Status}}"
```

---

## 🧠 Skills

Markdown files that shape the agent's personality:

```bash
zenclaw skills list              # Show available skills
zenclaw chat --skill coding      # Activate during chat
zenclaw chat --skill sysadmin    # Multiple skills work too
```

Create custom skills as `.md` files in `~/.local/share/zenclaw/skills/`.

---

## 🔍 RAG (Retrieval-Augmented Generation)

ZenClaw includes a built-in RAG system using SQLite FTS5 — no external vector database needed.

```bash
# Index files via the agent
> Index all Rust files in /home/user/project

# Or via API
curl -X POST http://localhost:3000/v1/rag/index \
  -d '{"source": "docs/guide.md", "content": "..."}'

# Search
curl -X POST http://localhost:3000/v1/rag/search \
  -d '{"query": "how to deploy", "limit": 5}'
```

---

## 🌐 Supported Providers

| Provider          | Models                            | Free Tier |
| ----------------- | --------------------------------- | :-------: |
| **OpenAI**        | GPT-4o, GPT-4o-mini               |    ❌     |
| **Google Gemini** | Gemini 2.0 Flash, 1.5 Pro         |    ✅     |
| **OpenRouter**    | 100+ models (Claude, Llama, etc.) |  Varies   |
| **Ollama**        | Llama 3, Mistral, Phi, Gemma      | ✅ Local  |
| **LM Studio**     | Any GGUF model                    | ✅ Local  |

---

## 📊 API Endpoints

| Method | Endpoint         | Description                |
| ------ | ---------------- | -------------------------- |
| `GET`  | `/v1/health`     | Health check               |
| `GET`  | `/v1/status`     | System status + tool list  |
| `POST` | `/v1/chat`       | Send message, get response |
| `POST` | `/v1/rag/index`  | Index document into RAG    |
| `POST` | `/v1/rag/search` | Search indexed documents   |

**Authentication:** Set `ZENCLAW_API_KEY` env var, then pass `Authorization: Bearer <key>` or `X-API-Key: <key>`.

---

## ⚙️ Configuration

```bash
zenclaw config show              # View current config
zenclaw config set provider gemini
zenclaw config set model gemini-2.0-flash
zenclaw config set api_key YOUR_KEY
zenclaw config path              # Show config file location
```

Config file: `~/.config/zenclaw/config.toml`

---

## 📦 Cross-Platform Builds

| Platform            | Target                      | Binary |
| ------------------- | --------------------------- | ------ |
| Linux x86_64        | `x86_64-unknown-linux-gnu`  | ~5.1MB |
| Linux ARM64         | `aarch64-unknown-linux-gnu` | ~5.2MB |
| macOS Intel         | `x86_64-apple-darwin`       | ~5.3MB |
| macOS Apple Silicon | `aarch64-apple-darwin`      | ~5.0MB |

```bash
# Cross-compile for Raspberry Pi
cargo build --release --target aarch64-unknown-linux-gnu

# Deploy
scp target/aarch64-unknown-linux-gnu/release/zenclaw pi@raspberrypi:~/
```

---

## 🗺️ Roadmap

- [x] ReAct agent engine with tool calling
- [x] Multi-provider LLM (OpenAI, Gemini, Ollama, OpenRouter, LM Studio)
- [x] 15 built-in tools + plugin system
- [x] 5 channel adapters (CLI, REST API, Telegram, Discord, WhatsApp)
- [x] RAG / full-text search (SQLite FTS5)
- [x] Multi-agent router
- [x] Persistent memory (SQLite)
- [x] Markdown skills system
- [x] REST API with rate limiting, auth, metrics
- [x] Docker support (Dockerfile + compose)
- [x] GitHub CI/CD (4-platform builds)
- [x] Auto-update checker
- [ ] Web dashboard (React/Svelte)
- [ ] Streaming responses (SSE)
- [ ] ESP32 thin client (no_std)

---

## 🤝 Contributing

```bash
git clone https://github.com/volumeee/zenclaw.git
cd zenclaw
cargo build                       # Dev build
cargo test                        # Run tests
RUSTFLAGS="-D warnings" cargo build  # Strict mode
cargo build --release             # Optimized (~5.1MB)
```

Create a release:

```bash
git tag v0.1.0 && git push origin v0.1.0
# → GitHub Actions auto-builds for 4 platforms
```

---

## 📜 License

MIT — Use it however you want. Build amazing things.

---

<p align="center">
  <sub>Built with ❤️ and 🦀 by <a href="https://github.com/volumeee">baguse</a></sub><br/>
  <sub><b>7,758</b> lines of Rust · <b>43</b> source files · <b>5.1MB</b> binary · <b>~12MB</b> RAM</sub>
</p>
