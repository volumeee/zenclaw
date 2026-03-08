<p align="center">
  <img src="https://img.shields.io/badge/⚡-ZenClaw-blueviolet?style=for-the-badge&logoColor=white" alt="ZenClaw" height="40"/>
</p>

<h3 align="center">Build AI the simple way 🦀</h3>

<p align="center">
  Lightweight, open-source AI agent framework for embedded &amp; edge devices.<br/>
  Hybrid Architecture: Native Rust 🦀 Core + Node.js 🟢 Bridge for seamless Web &amp; WhatsApp interactions.
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

## 🌟 Why ZenClaw?

Most AI agent frameworks are built in Python or bulky Node.js environments, consuming gigabytes of RAM and taking seconds to boot. **ZenClaw flips the script.**

ZenClaw's core is written entirely in **Rust**, compiling down to a single **~5.1MB binary** that boots instantly and idles at **~12MB of RAM**. This makes it perfect for Raspberry Pi, VPS instances, and Edge devices.

For browser-level capabilities (like headless scraping and WhatsApp Web), ZenClaw gracefully delegates tasks to an **optional Node.js bridge**, giving you the extreme performance of Rust combined with the vast web ecosystem of JavaScript.

### Framework Comparison

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

---

## ✨ Features

ZenClaw is fully packed with features out of the box. No complex configurations needed.

<details open>
<summary><b>🖥️ Beautiful Terminal UI (TUI)</b></summary>
<br>

ZenClaw drops the traditional messy CLI for a fully interactive Ratatui-based UI.

- **Interactive Dashboard:** Menu-driven interface. Say goodbye to memorizing arguments.
- **Scrollable Chat:** Clean, structured conversation view with streaming text.
- **Live Logs Monitoring:** Color-coded `tail` logs right inside the terminal.
- **Instant Model Switcher:** Swap between OpenAI, Gemini, or Ollama seamlessly.
</details>

<details>
<summary><b>🤖 Agent Engine & Intelligence</b></summary>
<br>

- **ReAct Loop:** Autonomous Think → Act → Observe reasoning loop.
- **7 LLM Providers:** OpenAI, Google Gemini, Groq, Ollama, OpenRouter, LM Studio, and Custom OpenAI-compatible endpoints.
- **Built-in RAG & Auto-Inject:** Index files into SQLite FTS5 in seconds. The agent engine automatically searches and injects relevant context into the system prompt behind the scenes during conversations.
- **Persistent Memory:** SQLite-backed conversational history context.
- **Smart Memory Pruning:** Instead of discarding old messages, older conversation history is summarized into compact **"Memory Cards"** — preserving context while keeping token usage efficient.
- **Tool Error Feedback:** When a tool call fails, ZenClaw sends the tool's JSON schema back to the LLM along with the error — preventing hallucination loops and guiding correct retry.
- **Skills System:** Inject markdown files to shape the agent's behavior.
</details>

<details>
<summary><b>🔧 16 Built-In Tools & Plugins</b></summary>
<br>

- **Filesystem:** `read_file`, `write_file`, `edit_file`, `list_dir`
- **Execution:** `exec` (shell), `process` (spawn/kill/status), `sub_agent` (background AI workers)
- **Web:** `web_search` (Multi-engine: Jina AI + DuckDuckGo + Wikipedia), `web_scrape` (Jina Reader + Headless fallback), `web_fetch` (raw HTTP/API)
- **System:** `system_info`, `health` (CPU/RAM/Disk/Network), `env` (API key status check)
- **Automation:** `cron` (Persistent background scheduler with autonomous **Proactive AI Agent Tasks**)
- **Data:** `history` (export conversations), `index_file` (RAG indexer), `webhooks` (receive external events), `codebase_search` (regex code search)
- **Plugin System:** Drop any Shell/Python script in the `plugins/` folder to create a new tool.

> **Note:** `web_search` and `web_scrape` use [Jina AI](https://jina.ai/) for reliable, clean results. Configure your API key via **Settings → Set JINA_API_KEY** in the TUI dashboard.

</details>

<details>
<summary><b>📡 6 Communication Channels</b></summary>
<br>

- **TUI Dashboard:** The primary interactive hub.
- **REST API:** Axum server with Server-Sent Events (SSE) streaming.
- **Telegram Bot:** Raw HTTP client via Telegram API.
- **Discord Bot:** WebSocket gateway connection.
- **Slack Bot:** Native integration via Slack Web API polling and editing.
- **WhatsApp Web:** Secured via the accompanying Node.js Puppeteer bridge.
</details>

---

## 🚀 Quick Start

### 1. Install ZenClaw

The fastest way to get started is by downloading the pre-built binary.

```bash
# Download and install on Linux
curl -L https://github.com/volumeee/zenclaw/releases/latest/download/zenclaw-linux-x86_64.tar.gz | tar xz
sudo mv zenclaw /usr/local/bin/

# Alternatively, compile via Cargo
cargo install --git https://github.com/volumeee/zenclaw.git
```

### 2. Enter the Dashboard

Everything in ZenClaw is operated through its seamless Terminal UI.
Simply type:

```bash
zenclaw
```

This opens the **Main Menu**, where you can:

1. Run the **Interactive Setup Wizard** to securely input your API Keys (e.g., Google Gemini for the free tier).
2. Start an **Interactive Chat** session.
3. Boot up the **Telegram/Discord/WhatsApp** bots.
4. Start the **REST API** server.
5. Monitor **Live System Logs**.

### 3. Configure Tool API Keys (Optional but Recommended)

For the best web search and scraping experience, configure your Jina AI API key:

```bash
# Option A: Via the TUI Dashboard
zenclaw
# → Settings → Set JINA_API_KEY → Paste your key

# Option B: Via CLI
zenclaw config set jina_api_key "jina_xxxxxxxxxxxx"

# Option C: Via .env file (auto-loaded on startup)
echo 'JINA_API_KEY=jina_xxxxxxxxxxxx' >> .env

# Option D: Via environment variable
export JINA_API_KEY="jina_xxxxxxxxxxxx"
```

Other supported tool API keys: `openweather_api_key`, `serper_api_key`.

> Get a free Jina API key at: [https://jina.ai/](https://jina.ai/)

---

## 📡 Deployment Guides

ZenClaw supports multiple modes for different use cases. You can launch them from the TUI menu or via terminal arguments for automation.

### Mode A: Interactive TUI (Default)

```bash
zenclaw         # Opens the Menu Dashboard
zenclaw chat    # Jumps straight into a TUI chat session
zenclaw logs    # Opens the live tail log monitor
```

### Mode B: Chat Bots (Discord, Telegram, Slack)

Run ZenClaw as a fully autonomous assistant in your groups or workspaces.

```bash
# Can be run entirely from the interactive TUI, or via CLI:
zenclaw telegram --token "BOT_TOKEN_HERE"
zenclaw discord --token "BOT_TOKEN_HERE"
zenclaw slack --token "xoxb-BOT_TOKEN_HERE"
```

### Mode C: REST API Server

Serve ZenClaw for your frontend web apps or external systems.

```bash
zenclaw serve --port 3000
```

**Example API Request:**

```bash
curl -X POST http://localhost:3000/v1/chat \
  -H "Content-Type: application/json" \
  -d '{"message": "Hello, who are you?", "session": "user1"}'
```

### Mode D: WhatsApp Bot & Web Scraping (Hybrid Mode)

For WhatsApp and advanced Web Scraping to function, the Node.js bridge must be running alongside the binary.

**1. Start the Node.js Bridge:**

```bash
cd bridge/
npm install
node bridge.js  # The terminal will display a QR Code. Scan it with WhatsApp!
```

_(Tip: PM2 is highly recommended for running `bridge.js` in production)._

```bash
# In another terminal window
zenclaw whatsapp --bridge http://localhost:3001
```

### Mode E: Production Deployment Security Baseline

When deploying ZenClaw in a production environment (such as a VPS serving the REST API or Telegram bots), adhere to the following security baseline:

1. **Reverse Proxy & TLS**: Never expose the ZenClaw `serve` API (port 3000) directly to the internet. Use a reverse proxy like **Nginx** or **Caddy** to handle SSL/TLS termination and provide an initial layer of request sanitization.
2. **Rate Limiting**: Configure strict IP-based rate limiting at the reverse proxy level to prevent abuse before traffic hits ZenClaw's internal middleware.
3. **Secret Management**: Do not bake API keys into Docker images. Use a secure environment variable injector or mount the `~/.config/zenclaw/config.toml` file at runtime using your platform's secret manager. 
4. **Bridge Isolation**: If using the Node.js bridge for WhatsApp or Web Scraping, run the bridge on `127.0.0.1` and ensure it is heavily firewall-restricted so external actors cannot access the Chromium debugging ports.
5. **Least Privilege**: Run the ZenClaw binary under a dedicated, unprivileged user account.

---

## 🧠 Customizing the Agent

### Injecting Skills

Shape the AI persona by placing `.md` files in `~/.local/share/zenclaw/skills/`.

```bash
zenclaw chat --skill sysadmin
```

### Built-in RAG & Auto-Inject

Easily inject files into the agent's knowledge base. Once indexed in SQLite, ZenClaw will automatically search this database on every chat turn and silently inject relevant context into the LLM's system prompt.

```bash
# Ask the agent inside the TUI Chat:
You: "Please use the index_file tool to ingest the documentation folder."
```

### Shell Plugins

You can add custom tools without recompiling Rust! Create a folder in `~/.local/share/zenclaw/plugins/my_tool/`:

```json
// plugin.json
{
  "name": "check_docker",
  "description": "Checks the status of docker containers",
  "command": "run.sh"
}
```

The agent and UI will dynamically register `check_docker` on next boot.

---

## 🏗️ Architecture Stack

ZenClaw's design isolates safety and speed.

1. **Rust Core (`crates/`):** Houses the ReAct Agent logic, SQLite Memory, Channel Hooks (Discord/Telegram/TUI), Axum Web Server, rate-limiting, and standard OS tools.
2. **Node Bridge (`bridge/`):** Runs an isolated Puppeteer Chromium instance. Safe from memory leaks interfering with the main core logic. Rust calls Node via HTTP and Subprocesses gracefully.

### Configuration Hierarchy

ZenClaw resolves settings in this priority order (highest first):

1. **CLI flags** (`--api-key`, `--model`, etc.)
2. **Environment variables** (`ZENCLAW_API_KEY`, `JINA_API_KEY`, etc.)
3. **`.env` file** in the working directory (auto-loaded via `dotenvy`)
4. **Config file** (`~/.config/zenclaw/config.toml`)

Tool-specific API keys (`jina_api_key`, `openweather_api_key`, `serper_api_key`) are stored in the config file under `[tools]` and automatically injected into the environment at startup.

## 🤝 Contributing & Building

We welcome PRs and Issue reports!

```bash
git clone https://github.com/volumeee/zenclaw.git
cd zenclaw
cargo run
cargo clippy --workspace -- -D warnings
```

_Note: See `CONTRIBUTING.md` for our AI-assisted code commit guidelines._

---

## 📜 License

MIT License. Build amazing bots safely.

<p align="center">
  <sub>Built with ❤️ and 🦀 by <a href="https://github.com/volumeee">volumeee</a></sub>
</p>
