# CommandHelp (`chelp`) 🚀

> **Universal AI-Powered CLI Assistant & Zero-Latency Autocomplete Engine**  
> Works with *any* command-line tool, parses `--help` and documentation dynamically, and lets you speak plain English directly in your terminal.

---

## ✨ Features

- ⚡ **Sub-15ms Autocomplete**: Caches parsed CLI flags and subcommands in SQLite and serves completions from a warm daemon inside a 15ms budget — slow machines degrade gracefully instead of stuttering your typing.
- 👻 **Ghost Text**: Start a command and a dim suggestion appears after the cursor. Accept it with **Tab** / **→** (zsh), **→** (bash ≥ 4), or ignore it.
- 📖 **Universal & Dynamic**: Reads `--help`, `-h`, and `man` pages of **any** command on the fly. Works on niche tools and private in-house company CLIs without needing pre-written specs.
- 🤖 **Bring Your Own Key (BYOK)**: First-class support for:
  - **Google Gemini** (`gemini-2.5-flash` - ultra-fast & free tier available)
  - **OpenAI** (`gpt-4o`, `gpt-4o-mini`)
  - **Anthropic Claude** (`claude-3-5-haiku`, `claude-3-5-sonnet`)
  - **Ollama** (100% Free & Local, completely offline, zero API keys required)
  - **OpenAI-Compatible Endpoints** (Groq, DeepSeek, OpenRouter, Mistral)
- 🛡️ **Interactive Safety Guardrails**: Prevents accidental disaster. High-risk destructive commands (`rm -rf`, `DROP TABLE`, `mkfs`, force pushes) are highlighted in bold red and require explicit confirmation — the first `Enter` only arms the prompt, `y` executes, `Tab`/`e` sends it back to your prompt for editing.
- 💻 **Terminal & Shell Agnostic**: Works inside your favorite terminal (Windows Terminal, iTerm2, Alacritty, Ghostty, Kitty) with Zsh, Bash (≥ 4; macOS' stock bash 3.2 is left untouched), Fish, and PowerShell. Diagnostics land in `~/.chelp/chelp.log` instead of your prompt.

---

## 🚀 Quickstart (1-Step Setup)

### 1. Instant One-Line Install

**macOS / Linux:**
```bash
curl -fsSL https://raw.githubusercontent.com/1Yosh1/commandhelp/master/install.sh | bash
```

**Homebrew (macOS / Linux):**
```bash
brew install 1Yosh1/tap/chelp
```

**Windows (PowerShell):**
```powershell
irm https://raw.githubusercontent.com/1Yosh1/commandhelp/master/install.ps1 | iex
```

*Or build from source:*
```bash
cargo install --git https://github.com/1Yosh1/commandhelp.git
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

An interactive modal takes over the terminal:

```text
┌ Suggested Command ─────────────────────────────────────────┐
│ find . -type f -name "*.mp4" -size +50M -mtime -7          │
└────────────────────────────────────────────────────────────┘
┌ Explanation ──────────────────────────────────────────────┐
│ Finds all regular .mp4 files over 50MB modified in 7 days. │
└───────────────────────────────────────────────────────────┘
┌ Safety Assessment ────────────────────────────────────────┐
│ ● SAFE (Read-Only)                                        │
└───────────────────────────────────────────────────────────┘
┌ Actions ──────────────────────────────────────────────────┐
│ [Enter] Run in shell   [Tab / e] Edit on prompt   [Esc] Cancel │
└───────────────────────────────────────────────────────────┘
```

- Press **`[Enter]`** to choose the command; run directly with the shell hook (below) or capture the printed command from a plain `chelp query` invocation.
- Press **`[Tab]`** or **`[e]`** to review or modify the command before running it.
- Press **`[Esc]`** to cancel.
- Destructive suggestions start with **`[Enter] Confirm`** — pressing it only arms the dialog, which then demands **`y`** to execute (`Tab`/`e` always backs out to your prompt).

### 2. Inline Ghost Text + Shell Shortcut
Inside zsh or bash (≥ 4) with the hook installed:

- Start typing and `chelp` shows a **ghost-text** suggestion; press **`Tab`/`→`** to accept it (bash uses `→` — `Tab` stays with readline's filename completion).
- Press **`[Ctrl + Space]`** to translate your current prompt line into a command.

The modal draws on the terminal only — your typed buffer is never overwritten by rendering. When it resolves:

- **`Run`** — the command executes in your shell (and lands in history)
- **`Edit`** — the command is placed on your prompt for review
- **`Cancel` (`Esc`)** — your original buffer is restored untouched

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
- `CHELP_API_KEY` — highest-priority override; the 1Password/Bitwarden/Vault pattern:
  `export CHELP_API_KEY="$(op read 'op://Vault/chelp/api-key')"`

### Environment & control variables

| Variable | Effect |
|---|---|
| `CHELP_API_KEY` | Overrides `ai.api_key` from the config file (vault-CLI friendly) |
| `CHELP_DISABLE_DAEMON=1` | Kill switch: hooks and auto-spawn become silent no-ops (audited/HIPAA workstations) |
| `CHELP_SOCKET_PATH` | Directory for the Unix socket (default: system temp). Its dir is tightened to `0700`, socket file to `0600` |
| `CHELP_TRIGGER_KEY=ctrl-x` | Remap the Ctrl+Space query trigger in hooks generated by `chelp init` (all four shells) |
| `CHELP_SOCKET` | Rename the IPC socket (dev/testing) |
| `CHELP_NO_AUTO_SPAWN` | Forbid the client from lazily spawning the daemon |

### Diagnostic commands

```bash
chelp status    # daemon liveness, kill switch, socket path/mode, cache size, provider/model
chelp privacy   # security/data-flow audit (see docs/security.md)
chelp bench 50  # warm ghost-text latency p50/p95/max vs the 15 ms budget
chelp cache clear | info   # GDPR purge / cache stats
chelp --dump-ast docker    # replay the CLI parser on any binary, offline
```

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
