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
- **5 LLM Providers:** OpenAI, Google Gemini, Ollama, OpenRouter, and LM Studio.
- **Built-in RAG:** Index files into SQLite FTS5 in seconds (No Vector DB needed).
- **Persistent Memory:** SQLite-backed conversational history context.
- **Skills System:** Inject markdown files to shape the agent's behavior.
</details>

<details>
<summary><b>🔧 15 Built-In Tools & Plugins</b></summary>
<br>

- `exec`, `read_file`, `write_file`, `edit_file`, `list_dir`
- `web_fetch`, `web_search` (DuckDuckGo), `web_scrape` (Headless Chromium)
- `cron` (scheduler), `system_info`, `health`, `history`, `index_file`, `env`
- **Plugin System:** Drop any Shell/Python script in the `plugins/` folder to create a new tool.
</details>

<details>
<summary><b>📡 5 Communication Channels</b></summary>
<br>

- **TUI Dashboard:** The primary interactive hub.
- **REST API:** Axum server with Server-Sent Events (SSE) streaming.
- **Telegram Bot:** Raw HTTP client via Telegram API.
- **Discord Bot:** WebSocket gateway connection.
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

---

## 📡 Deployment Guides

ZenClaw supports multiple modes for different use cases. You can launch them from the TUI menu or via terminal arguments for automation.

### Mode A: Interactive TUI (Default)

```bash
zenclaw         # Opens the Menu Dashboard
zenclaw chat    # Jumps straight into a TUI chat session
zenclaw logs    # Opens the live tail log monitor
```

### Mode B: Discord & Telegram Bots

Run ZenClaw as a fully autonomous assistant in your groups.

```bash
# Can be run entirely from the interactive TUI, or via CLI:
zenclaw telegram --token "BOT_TOKEN_HERE"
zenclaw discord --token "BOT_TOKEN_HERE"
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

**2. Start the ZenClaw WhatsApp Hook:**

```bash
# In another terminal window
zenclaw whatsapp --bridge http://localhost:3001
```

---

## 🧠 Customizing the Agent

### Injecting Skills

Shape the AI persona by placing `.md` files in `~/.local/share/zenclaw/skills/`.

```bash
zenclaw chat --skill sysadmin
```

### Built-in RAG

Easily inject files into the agent's knowledge base.

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
