# CommandHelp (`chelp`): Universal AI-Powered CLI Assistant & Autocomplete Engine

**Date:** 2026-09-06  
**Status:** Approved Specification  
**Binary Name:** `chelp` (client) / `chelpd` (daemon mode)  
**Implementation Language:** Rust (2021 Edition)

---

## 1. Executive Summary & Goals

### 1.1 Problem Statement
Developers frequently context-switch from terminal environments to web browsers or AI chat apps to search for CLI flags, subcommands, and syntax rules for tools like `git`, `docker`, `kubectl`, `ffmpeg`, `tar`, and custom in-house enterprise binaries. Existing solutions either:
1. Require switching to a proprietary terminal application (e.g., Warp).
2. Depend on manually curated, crowdsourced static completion specs (e.g., Amazon Q / Fig), which fail on undocumented, niche, or internal corporate tools.
3. Function as slow chat bots without instant, sub-millisecond typing autocompletion.

### 1.2 Value Proposition
`chelp` is a fast, terminal-agnostic CLI companion written in Rust that:
* Dynamically reads `--help`, `-h`, and `man` pages of **any** command on the fly.
* Builds an in-memory and SQLite-backed structured syntax schema.
* Provides zero-latency inline ghost-text autocompletion as the user types.
* Allows developers to express intent in natural language (`? <prompt>` or `Ctrl+Space`), generating exact commands with explanations, danger analysis, and interactive execution controls (`[Enter] Run`, `[Tab/e] Edit`, `[Esc] Cancel`).
* Operates under a hybrid monetization model (Free BYOK + Cloud Pro subscription).

---

## 2. Architecture & Process Model

`chelp` operates as a client-daemon architecture over local IPC to ensure that user keystrokes are never blocked.

```
┌─────────────────────────────────────────────────────────────┐
│                      User Terminal                          │
│  (PowerShell / Zsh / Bash / Fish on Windows / macOS / Linux)│
└───────────────────────────┬─────────────────────────────────┘
                            │
              Shell Hook    │ Fast IPC (< 1ms)
              (Keypresses)  ▼
┌─────────────────────────────────────────────────────────────┐
│                    chelp (Client Binary)                    │
│   - chelp complete <buffer>  (Autofill request)             │
│   - chelp query <prompt>     (Natural language request)     │
│   - chelp init <shell>       (Emits hook scripts)           │
└───────────────────────────┬─────────────────────────────────┘
                            │
              Named Pipe /  │ Local IPC (< 1ms roundtrip)
              Unix Socket   ▼
┌─────────────────────────────────────────────────────────────┐
│                  chelpd (Background Daemon)                 │
│  ┌────────────────────────┐    ┌─────────────────────────┐  │
│  │ In-Memory Trie Cache   │    │ SQLite Schema Database  │  │
│  │ (Active CLI Schemas)   │    │ (~/.chelp/data.db)      │  │
│  └───────────┬────────────┘    └────────────┬────────────┘  │
│              ▲                              ▲               │
│              └──────────────┬───────────────┘               │
│                             │                               │
│              ┌──────────────┴──────────────┐                │
│              │ Background Crawler & Parser │                │
│              │ (Runs `<bin> --help` async) │                │
│              └─────────────────────────────┘                │
└─────────────────────────────┬───────────────────────────────┘
                              │
               HTTPS / REST   │ (On-Demand NL Queries)
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                       AI Providers                          │
│  - BYOK: Gemini 2.5 Flash / OpenAI / Anthropic / Ollama     │
│  - Pro Cloud: chelp Hosted Low-Latency Gateway              │
└─────────────────────────────────────────────────────────────┘
```

### 2.1 Single Binary Architecture
A single compiled binary (`chelp`) serves all roles:
* `chelp daemon`: Launches the persistent background service.
* `chelp daemon --detached`: Spawns the daemon in the background if not already running.
* `chelp complete "<buffer>"`: Queried by shell line-editors for instant completions.
* `chelp query "<prompt>"`: Invoked when a natural language prefix or hotkey is triggered.
* `chelp init <shell>`: Outputs shell hook code for `pwsh`, `zsh`, `bash`, `fish`.
* `chelp config`: Interactive configuration for API keys, model preferences, and telemetry.

### 2.2 Local IPC Specifications
* **Windows**: Named Pipe `\\.\pipe\chelp-ipc-<username>`.
* **macOS / Linux**: Unix Domain Socket `$XDG_RUNTIME_DIR/chelp.sock` (fallback: `~/.chelp/chelp.sock`).
* **Message Format**: Length-prefixed JSON-RPC 2.0 payloads.
* **Timeout SLA**: Max client wait time for autocomplete requests is **15ms**. If the daemon does not respond within 15ms, the hook returns empty immediately so terminal typing never stutters.
* **Auto-Spawn**: If the client fails to connect to the socket, it invokes `chelp daemon --detached` and retries after 40ms.

### 2.3 Storage & Persistence (`~/.chelp/`)
* `~/.chelp/config.toml`: Stores user preferences, API keys, and model parameters.
* `~/.chelp/data.db`: SQLite database storing parsed CLI schemas, binary modification timestamps (`mtime`), and query history.
* `~/.chelp/chelp.log`: Rolling diagnostic log for debugging parser edge cases.

---

## 3. Shell Integration & Dual-Trigger UX

### 3.1 Supported Shells & Integration Strategy
1. **PowerShell (Windows)**:
   * Uses `Set-PSReadLineOption` and `Set-PSReadLineKeyHandler`.
   * Intercepts buffer modifications to display ghost text.
   * `Ctrl+Space` or typing `? <query>` + `Enter` calls `chelp query`.
2. **Zsh (macOS / Linux)**:
   * Leverages ZLE (Zsh Line Editor) widgets (`zle-line-update`, `accept-line`).
   * Renders ghost text in ANSI-8 (dim gray) following the cursor.
   * `bindkey '^ ' chelp-query`.
3. **Bash**:
   * Uses `bind -x` and Readline macros.
4. **Fish**:
   * Utilizes `fish_commandline` functions and event-driven hooks.

### 3.2 Dual-Trigger Workflow
* **Trigger 1: Ghost Text (Deterministic / Autocomplete)**:
  * User enters: `docker run --`
  * Daemon looks up cached schema for `docker run` in < 1ms.
  * Suggests matching flags (e.g., `--rm`, `--detach`, `--name`) with short descriptions inline.
  * Pressing `[Tab]` or `[Right Arrow]` accepts the suggestion.
* **Trigger 2: Plain English / Natural Language**:
  * Activated by starting a line with `? ` or pressing `Ctrl+Space`.
  * Example: `? find all mp4 files larger than 100mb modified this week`
  * Shell hook forwards the prompt to `chelp query`.

### 3.3 Interactive Confirmation & Safety Modal
When natural language resolves, `chelp` switches to raw terminal mode via `crossterm` and renders an interactive popup directly below the prompt:

```text
┌── CommandHelp AI ────────────────────────────────────────────────────────┐
│ Command:  find . -type f -name "*.mp4" -size +100M -mtime -7             │
│ Explain:  Searches current folder for .mp4 files >100MB updated <7 days. │
│ Safety:   ● SAFE (Read-only query)                                       │
├──────────────────────────────────────────────────────────────────────────┤
│ [Enter] Run    [Tab / e] Edit on Prompt    [Esc] Cancel                  │
└──────────────────────────────────────────────────────────────────────────┘
```

#### Action Controls:
* `[Enter]`: Executes command in active shell session; adds it to shell history.
* `[Tab]` or `[e]`: Injects the command string directly into the shell prompt line buffer, allowing manual review and flag tweaking before running.
* `[Esc]`: Cancels out, restores the previous terminal buffer, and cleans up the overlay.

---

## 4. CLI Doc Ingestion & Schema Engine

### 4.1 Non-Blocking Discovery
1. When a user enters a command binary name (e.g., `ffmpeg`), the shell hook emits a lightweight `CheckBinary("ffmpeg")` event to `chelpd`.
2. If `ffmpeg` is not present in SQLite, `chelpd` queues a background task to index it. The user continues typing uninterrupted.

### 4.2 Safe Execution Sandbox for `--help`
To prevent the crawler from locking up on interactive pagers:
* All probe commands are executed with environment variables: `PAGER=cat`, `MANPAGER=cat`, `CI=true`, `TERM=dumb`.
* Strict process execution timeout: **1,500ms**.
* Probe sequence: `<bin> --help` -> `<bin> -h` -> `help <bin>` -> `man <bin>`.

### 4.3 Multi-Format Heuristic Parser
The parser uses a deterministic regex and AST state machine to handle major CLI conventions:
* **GNU / POSIX style**: `-v, --verbose     Description text`
* **Go / Cobra style**: `--config string   Configuration file path`
* **Python Click / Typer style**: `-p, --port INTEGER   Port number [default: 8080]`
* **Windows CMD / PowerShell**: `/?`, `/F:value`, `/Q`

### 4.4 Schema Data Structures
```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CliCommandSchema {
    pub binary: String,
    pub subcommand_path: Vec<String>,
    pub usage: String,
    pub description: String,
    pub flags: Vec<CliFlag>,
    pub subcommands: Vec<String>,
    pub binary_mtime: u64,
    pub last_indexed: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CliFlag {
    pub short: Option<String>,
    pub long: Option<String>,
    pub takes_value: bool,
    pub value_hint: Option<String>,
    pub description: String,
}
```

### 4.5 Lazy Subcommand Traversal
For complex CLI ecosystems (`git`, `docker`, `kubectl`, `aws`):
* The root binary `--help` is indexed first.
* Subcommands (`docker container`, `git rebase`) are marked as unexpanded stubs in SQLite.
* When the user types `docker container `, the daemon indexes `docker container --help` on demand.

---

## 5. AI Translation Engine & Safety Guardrails

### 5.1 Provider Architecture
`chelp` defines a unified asynchronous trait:

```rust
#[async_trait]
pub trait AiProvider: Send + Sync {
    async fn resolve_intent(
        &self,
        query: &str,
        context: &ShellContext,
        schemas: &[CliCommandSchema],
    ) -> Result<AiCommandResponse, ChelpError>;
}
```

### 5.2 Supported Providers
1. **Google Gemini (Default for BYOK)**: Model `gemini-2.5-flash` via REST API. Extremely fast (<500ms) with a generous free tier.
2. **OpenAI**: Model `gpt-4o-mini`.
3. **Anthropic**: Model `claude-3-5-haiku`.
4. **Ollama (Local / Private)**: Connects to local endpoint `http://localhost:11434` (e.g. `qwen2.5-coder:7b`). Zero token cost, completely offline.
5. **Cloud Pro**: Authenticated reverse proxy routing requests with zero user configuration.

### 5.3 Prompt Engineering & Context Assembly
* Injects:
  * Target OS (`windows`, `macos`, `linux`).
  * Target Shell (`pwsh`, `zsh`, `bash`).
  * Relevant parsed flags for any detected tools in the query.
* Model is constrained via schema or system prompt to return strict JSON:
  ```json
  {
    "command": "git log --graph --oneline --decorate --all",
    "explanation": "Visualizes the full commit history graph with one line per commit across all branches.",
    "safety_level": "safe",
    "destructive_warning": null
  }
  ```

### 5.4 Dual-Layer Safety Verification
1. **Model Self-Rating**: The LLM assigns `safe`, `caution`, or `destructive`.
2. **Local Hard-Coded Heuristic Validator**:
   * Evaluates command tokens against regex patterns for destructive actions:
     * `rm\s+-.*r`, `del\s+/.*s`, `Remove-Item.*-Recurse`
     * `dd\s+if=.*of=`, `mkfs`, `format`
     * `DROP\s+(DATABASE|TABLE)`, `TRUNCATE`
     * `git\s+push.*--force`, `git\s+reset\s+--hard`
     * `killall`, `kill\s+-9`, `Stop-Process\s+-Force`
3. **High-Risk Enforcement**:
   * Destructive commands are rendered in bold **RED** with a prominent warning banner.
   * `[Enter]` is disabled by default for destructive commands; the user must explicitly press `[Tab]` to review in their prompt or confirm by typing `yes`.

---

## 6. Business, Monetization & Distribution Plan

### 6.1 Pricing Tiers
* **Community Tier (Free / Open Source)**:
  * Full local `--help` parsing and ghost text autocomplete.
  * Full natural language query capabilities using **BYOK** (Gemini, OpenAI, Anthropic, Ollama).
* **Pro Tier ($8/month or $49 Lifetime)**:
  * Hosted zero-config cloud AI (no API keys required).
  * Cloud sync for custom team aliases, indexed internal company CLIs, and history.
  * Access to pre-warmed indexed schemas for 1,000+ CLI tools.
* **Team / Enterprise ($20/seat/month)**:
  * Centralized catalog of proprietary internal microservice CLIs.
  * Enforced company safety policies (block execution of dangerous commands).
  * SSO, audit logging, and spend controls.

### 6.2 Distribution Channels
* **macOS / Linux**: Homebrew (`brew install chelp`).
* **Windows**: Winget (`winget install chelp`) and Scoop (`scoop install chelp`).
* **Cargo**: `cargo install chelp`.
* **Universal Script**: `curl -fsSL https://chelp.dev/install.sh | sh`.

---

## 7. Error Handling, Edge Cases & Telemetry

1. **Unresponsive Daemon**: If IPC times out (>15ms), client silently falls back to standard shell behavior.
2. **Offline Mode**: If offline and no local Ollama is detected, informs the user with an actionable message. Local autocomplete remains 100% operational.
3. **CLI Errors**: If a tool returns exit code `!= 0` during `--help` inspection, `chelp` aborts indexing without crashing and marks the binary as skipped.
4. **Binary Updates**: Stores binary `mtime` in SQLite. If a binary is updated (e.g. via `brew upgrade` or `winget upgrade`), the daemon automatically re-indexes it.

---

## 8. Verification & Testing Strategy

1. **Unit Tests**:
   * Multi-format parser tests against 30+ real-world fixture texts (`git --help`, `docker --help`, `kubectl --help`, `ffmpeg -h`, `tar --help`, PowerShell `Get-Help`).
   * Safety validator tests asserting 100% detection of critical destructive commands.
2. **IPC Integration Tests**:
   * Automated tests verifying named pipe / Unix domain socket message roundtrip benchmarks (< 1ms).
   * Daemon auto-spawn and graceful recovery tests.
3. **Shell Hook Tests**:
   * Script tests verifying output syntax for `chelp init pwsh`, `chelp init zsh`, and `chelp init bash`.
