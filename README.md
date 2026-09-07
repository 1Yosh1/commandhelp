# CommandHelp (`chelp`) 🚀

> **Universal AI-Powered CLI Assistant & Zero-Latency Autocomplete Engine**  
> Works with *any* command-line tool, parses `--help` and documentation dynamically, and lets you speak plain English directly in your terminal.

---

## ✨ Features

- ⚡ **Zero-Latency Autocomplete (<1ms)**: Caches parsed CLI flags and subcommands in SQLite and in-memory Tries. Never slows down your typing.
- 📖 **Universal & Dynamic**: Reads `--help`, `-h`, and `man` pages of **any** command on the fly. Works on niche tools and private in-house company CLIs without needing pre-written specs.
- 🤖 **Bring Your Own Key (BYOK)**: First-class support for:
  - **Google Gemini** (`gemini-2.5-flash` - ultra-fast & free tier available)
  - **OpenAI** (`gpt-4o`, `gpt-4o-mini`)
  - **Anthropic Claude** (`claude-3-5-haiku`, `claude-3-5-sonnet`)
  - **Ollama** (100% Free & Local, completely offline, zero API keys required)
  - **OpenAI-Compatible Endpoints** (Groq, DeepSeek, OpenRouter, Mistral)
- 🛡️ **Interactive Safety Guardrails**: Prevents accidental disaster. High-risk destructive commands (`rm -rf`, `DROP TABLE`, `mkfs`, force pushes) are highlighted in bold red and require explicit confirmation.
- 💻 **Terminal & Shell Agnostic**: Works inside your favorite terminal (Windows Terminal, iTerm2, Alacritty, Ghostty, Kitty) with PowerShell, Zsh, and Bash.

---

## 🚀 Quickstart (1-Step Setup)

### 1. Build or Install

```bash
git clone https://github.com/1Yosh1/commandhelp.git
cd commandhelp
cargo build --release
```

### 2. Run Automatic Setup

Run the 1-step setup wizard:

```powershell
.\target\release\chelp.exe setup
```

The wizard will:
1. Automatically detect your active shell and install the shell hook into your profile (`$PROFILE`, `~/.zshrc`, or `~/.bashrc`).
2. Ask you to select your preferred AI provider (Gemini, OpenAI, Claude, Ollama, or Custom).
3. Test the connection and save your configuration to `~/.chelp/config.toml`.

---

## 🎯 How to Use

### 1. Natural Language Commands
Ask what you want in plain English:

```bash
chelp query "find all mp4 files larger than 50MB modified in the last 7 days"
```

An interactive terminal card appears right beneath your prompt:

```text
┌── CommandHelp AI ────────────────────────────────────────────────────────┐
│ Command:  find . -type f -name "*.mp4" -size +50M -mtime -7              │
│ Explain:  Finds all regular .mp4 files over 50MB modified within 7 days. │
│ Safety:   ● SAFE (Read-only query)                                       │
├──────────────────────────────────────────────────────────────────────────┤
│ [Enter] Run    [Tab / e] Edit on Prompt    [Esc] Cancel                  │
└──────────────────────────────────────────────────────────────────────────┘
```

- Press **`[Enter]`** to execute the command directly.
- Press **`[Tab]`** or **`[e]`** to place the command onto your shell prompt so you can review or modify flags before running.
- Press **`[Esc]`** to cancel.

### 2. Inline Shell Shortcut
In your shell, type your command intent and press:
- **`[Ctrl + Space]`**

`chelp` intercepts the prompt, generates the command, and lets you execute or edit it immediately.

### 3. Change AI Providers Anytime
Switch models or keys with a single command:

```bash
chelp config
```

---

## ⚙️ Configuration (`~/.chelp/config.toml`)

You can also edit your config directly:

```toml
[ai]
provider = "gemini" # Options: "gemini", "openai", "anthropic", "ollama", "custom"
model = "gemini-2.5-flash"
api_key = "your-api-key-here"

# For Ollama or custom providers:
# endpoint = "http://localhost:11434"
```

Or use environment variables:
- `GEMINI_API_KEY`
- `OPENAI_API_KEY`
- `ANTHROPIC_API_KEY`

---

## 🏗️ Architecture

```
User Shell (PowerShell / Zsh / Bash)
       │
       │ Local IPC (<1ms Named Pipe / Unix Socket)
       ▼
chelp Daemon (Auto-spawned in background)
  ├── SQLite Schema Cache (~/.chelp/data.db)
  ├── Fast Substring/Trie Matching
  └── Background --help Crawler (PAGER=cat, 1500ms safety timeout)
       │
       ▼
AI Provider Layer (Gemini / OpenAI / Claude / Ollama)
       │
       ▼
Interactive TUI Modal (Ratatui / Crossterm)
  ├── Deterministic Safety Verification
  └── [Enter] Run / [Tab] Edit / [Esc] Cancel
```

---

## 📜 License

MIT License. Contributions welcome!
