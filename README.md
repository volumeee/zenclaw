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

ZenClaw's core is built in Rust — giving you a tiny, fast, self-contained binary. For features that require browser-level access (WhatsApp and headless web scraping), it delegates to a lightweight **optional Node.js bridge** that runs alongside the Rust binary.

### Comparison with Popular Agent Frameworks

|                      | [**ZenClaw**](https://github.com/volumeee/zenclaw) | [**OpenClaw**](https://github.com/openclaw/openclaw) | [**NanoClaw**](https://github.com/gavrielc/nanoclaw) | [**PicoClaw**](https://github.com/sipeed/picoclaw) |
| -------------------- | -------------------------------------------------- | ---------------------------------------------------- | ---------------------------------------------------- | -------------------------------------------------- |
| **Language**         | Rust 🦀 + Node.js bridge (opt.)                    | TypeScript / Node.js                                 | TypeScript                                           | Go                                                 |
| **Core Binary Size** | **5.1 MB**                                         | > 200MB (Node Modules)                               | Containerized (>100MB)                               | ~10MB Binary                                       |
| **Idle RAM (core)**  | **~12 MB**                                         | ~800MB – 1.5GB                                       | ~200MB – 500MB                                       | ~15 MB                                             |
| **Boot time**        | **< 100ms**                                        | 2–5s                                                 | 1–3s                                                 | < 1s                                               |
| **Runtime deps**     | **None (core)** / Node.js 18+ (WhatsApp+Scrape)    | Node.js 18+, OS libs                                 | Node.js, Container Runtime                           | 0 (Zero)                                           |
| **Architecture**     | Hybrid: Native Binary + optional Node.js bridge    | Client-Server / Gateway                              | Agent Containers                                     | Native Binary (Static)                             |
| **RAG System**       | **✅ SQLite FTS5 built-in**                        | ChromaDB / External                                  | Vector Search / Files                                | MarkDown Files                                     |
| **Edge/ARM ready**   | **✅ Yes (Pi Zero/STB)** (core only)               | ❌ Too Heavy                                         | ⚠️ Requires Docker                                   | ✅ Yes (RISC-V/ARM)                                |

> **ZenClaw core** runs as a single **5.1MB Rust binary** — zero dependencies needed for CLI, Telegram, Discord, REST API, and RAG.
> **WhatsApp** and **web scraping** use the optional `bridge/` Node.js helper (Puppeteer + whatsapp-web.js).

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
- **Exponential backoff** — 3-retry with smart delay on provider errors

</td>
<td width="50%">

### 🔧 15 Built-in Tools

- Shell execution, file I/O, directory listing
- Web fetch (HTTP), web search (DuckDuckGo)
- Web scrape (Jina AI + local Puppeteer fallback)
- Cron scheduler, system info, health monitor
- History export, file indexer, env inspector
- Webhook receiver + extensible plugins

</td>
</tr>
<tr>
<td>

### 📡 5 Channel Adapters

- **CLI** — interactive terminal chat
- **REST API** — HTTP endpoints (Axum) + SSE streaming
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
- **Live log monitoring** — real-time log tailing with color

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
┌─────────────────────────────────────────────────────────────────────────┐
│                     ZenClaw Runtime (Rust Binary ~5.1MB)               │
│                                                                         │
│  ┌──────────────┐   ┌──────────────┐   ┌───────────────────────────┐   │
│  │   Channels    │   │  Agent Core  │   │         Tools             │   │
│  │              │   │              │   │                           │   │
│  │  • CLI       │──▶│  ReAct Loop  │──▶│  • exec (shell)          │   │
│  │  • REST API  │   │              │   │  • read/write/edit/list   │   │
│  │  • Telegram  │   │  ┌────────┐  │   │  • web_fetch              │   │
│  │  • Discord   │   │  │ Router │  │   │  • web_scrape ──────────────────────┐
│  │  • WhatsApp ─│───│──│────────│──│───│──────────────────────────         │
│  └──────────────┘   │  └────────┘  │   │  • web_search             │       │
│          │          │  ┌────────┐  │   │  • cron / health          │       │
│          │          │  │ Skills │  │   │  • history / index_file   │       │
│          │          │  └────────┘  │   │  • webhooks / env         │       │
│          │          └──────────────┘   │  • + plugins              │       │
│          │                 │           └───────────────────────────┘       │
│  ┌───────────────┐         ▼                                               │
│  │   Providers   │  ┌──────────────┐   ┌───────────────────────────┐      │
│  │  • OpenAI     │  │    Memory    │   │       Middleware           │      │
│  │  • Gemini     │  │  • SQLite    │   │  • Rate limiter           │      │
│  │  • Ollama     │  │  • RAG/FTS5  │   │  • API key auth           │      │
│  │  • OpenRouter │  └──────────────┘   │  • Request logging        │      │
│  │  • LM Studio  │                     └───────────────────────────┘      │
│  └───────────────┘                                                         │
└────────────────────────────────────────────────────────────────────────────┘
         │ HTTP poll                             │ spawns process
         ▼                                       ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│              bridge/  (Node.js 18+ — optional, only for WA & scraping)     │
│                                                                             │
│  bridge.js  ─  WhatsApp Web via whatsapp-web.js + Puppeteer               │
│    • QR code scan on first run                                              │
│    • Exposes HTTP: GET /messages  POST /send  GET /status (port 3001)      │
│                                                                             │
│  scrape.js  ─  Headless Chromium scraper via Puppeteer                    │
│    • Anti-bot evasion (User-Agent, networkidle2)                           │
│    • Strips nav/header/footer/scripts → returns clean plain text           │
│    • Called as subprocess by web_scrape tool (Rust spawns node scrape.js) │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Crate Structure

```
zenclaw/                                    8,976 lines of Rust
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
│   │   ├── bus.rs                          Async event bus + format_status()
│   │   └── error.rs                        Error types
│   │
│   ├── zenclaw-hub/                        Full implementations
│   │   ├── api.rs                          REST API server (Axum) + SSE status_text
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
│   │       ├── web_scrape.rs               Extract Markdown from any URL
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
│       ├── main.rs                         CLI commands (12 commands)
│       └── setup.rs                        Interactive TUI wizard
│
├── bridge/                                 Node.js bridge (WhatsApp + Scraper)
│   ├── bridge.js                           WhatsApp Web HTTP bridge (port 3001)
│   │                                         whatsapp-web.js + Puppeteer
│   │                                         Endpoints: /messages /send /status
│   ├── scrape.js                           Headless Chromium scraper
│   │                                         Puppeteer-based, strips bloat → plain text
│   │                                         Spawned as subprocess by web_scrape tool
│   └── package.json                        Dependencies: puppeteer, whatsapp-web.js,
│                                             express, body-parser, qrcode-terminal
├── Dockerfile                              Multi-stage build
├── docker-compose.yml                      One-command deploy
├── .github/workflows/
│   ├── ci.yml                              Check/lint/test + auto-tag+build on version bump
│   └── release.yml                         Manual release on tag push
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

### Monitor Live Logs

```bash
zenclaw logs                  # Tail last 50 log lines in real-time
zenclaw logs --lines 100      # Tail last 100 lines
# Logs stored at: ~/.local/share/zenclaw/logs/zenclaw.log.YYYY-MM-DD
# Colors: ERROR=red WARN=yellow INFO=green DEBUG=blue
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

WhatsApp requires the **Node.js bridge** (uses `whatsapp-web.js` + Puppeteer to drive WhatsApp Web).

```bash
# Step 1: Start the Node.js bridge first
cd bridge/
npm install          # First time only
node bridge.js       # Scan the QR code with your phone

# Step 2: Start ZenClaw WhatsApp (in a new terminal)
zenclaw whatsapp --bridge http://localhost:3001
```

The bridge exposes a local HTTP API on port `3001`:
| Endpoint | Method | Description |
|----------|--------|-------------|
| `/messages` | `GET` | Poll new incoming messages (cleared after read) |
| `/send` | `POST` | Send a message `{"to": "628xxx@c.us", "message": "Hi"}` |
| `/status` | `GET` | Check bridge ready status |

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

| Tool          | Description                                             |
| ------------- | ------------------------------------------------------- |
| `exec`        | Execute shell commands with output capture              |
| `read_file`   | Read file contents with optional line ranges            |
| `write_file`  | Create or overwrite files                               |
| `edit_file`   | Search & replace within files                           |
| `list_dir`    | List directory contents with metadata                   |
| `web_fetch`   | HTTP requests (GET/POST/PUT/DELETE) with custom headers |
| `web_scrape`  | Extract clean Markdown from any URL (Jina + Puppeteer)  |
| `web_search`  | Search the internet via DuckDuckGo                      |
| `system_info` | OS, architecture, hostname, user info                   |
| `cron`        | Schedule delayed shell commands                         |
| `health`      | CPU, memory, disk, network, uptime                      |
| `history`     | Export conversations (JSON/Markdown)                    |
| `index_file`  | Index files into RAG knowledge base                     |
| `webhooks`    | Inspect received webhook events                         |
| `env`         | Check environment variables & API keys                  |
| + **Plugins** | Any shell script can become a tool                      |

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

| Method | Endpoint         | Description                           |
| ------ | ---------------- | ------------------------------------- |
| `GET`  | `/v1/health`     | Health check                          |
| `GET`  | `/v1/status`     | System status + tool list             |
| `POST` | `/v1/chat`       | Send message, get SSE response stream |
| `POST` | `/v1/rag/index`  | Index document into RAG               |
| `POST` | `/v1/rag/search` | Search indexed documents              |

**SSE Events** (`POST /v1/chat` streams Server-Sent Events):

| Event             | Description                                             |
| ----------------- | ------------------------------------------------------- |
| `agent_think`     | Agent iteration count payload                           |
| `tool_use`        | Tool name + args being called                           |
| `tool_result`     | Tool execution completed                                |
| `memory_truncate` | History truncation event                                |
| `tool_timeout`    | Tool exceeded 60s timeout                               |
| `status_text`     | 🆕 Human-readable status (e.g. `🛠️ Reading Page (url)`) |
| `result`          | Final agent response                                    |
| `error`           | Error payload                                           |

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

**✅ Completed**

- [x] ReAct agent engine with tool calling
- [x] Multi-provider LLM (OpenAI, Gemini, Ollama, OpenRouter, LM Studio)
- [x] 15 built-in tools + plugin system
- [x] 5 channel adapters (CLI, REST API, Telegram, Discord, WhatsApp)
- [x] Full Interactive CLI UI Loop (`v0.1.6`)
- [x] RAG / full-text search (SQLite FTS5)
- [x] Persistent memory (SQLite)
- [x] **Live Log Monitoring** — `zenclaw logs` real-time rolling tails with color (`v0.1.7`)
- [x] **Centralized Event Formatting** — DRY `SystemEvent::format_status()` across all channels (`v0.1.7`)
- [x] **SSE `status_text` stream** — human-readable status events via REST API (`v0.1.7`)
- [x] **CLI Architecture Refactor** — `setup_bot_env()` factory eliminates ~100 lines of duplicated bootstrapping code (`v0.1.7`)
- [x] **Web Scraping** — extract clean Markdown from any web page via Jina AI + local Puppeteer fallback

**🔥 High Priority (Next)**

- [ ] **Vision / Multimodal Input** — Image understanding in `ChatRequest` (OpenAI vision API)
- [ ] **Slack Channel** — adapter for Slack workspace bots
- [ ] **RAG Auto-Inject** — automatically prepend relevant RAG context to system prompt
- [ ] **Proactive Tasks** — background agent scheduling without user input trigger

**🚀 Medium Priority**

- [ ] **Local Web Dashboard** (GUI for managing settings, prompts, and plugins easily)
- [ ] **Multi-Agent Swarm** (Agent orchestration & collaboration)
- [ ] **Vector Knowledge Base** (ChromaDB/Qdrant integration)

**✨ Backlog**

- [ ] Streaming responses (chunked SSE tokens)
- [ ] ESP32 thin client (no_std)
- [ ] Signal & iMessage channel adapters

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

<p align="center">
  <sub>Built with ❤️ and 🦀 by <a href="https://github.com/volumeee">volumeee</a></sub><br/>
  <sub><b>8,976</b> lines of Rust · <b>46</b> source files · <b>5.1MB</b> binary · <b>~12MB</b> RAM</sub>
</p>
