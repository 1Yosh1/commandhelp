# CommandHelp (`chelp`) Developer Launch Kit

> **Ready-to-publish launch materials, technical post-mortems, and community copy for Hacker News, Reddit, Twitter/X, and Product Hunt.**

---

## 1. Hacker News — Show HN Submission

### Timing & Logistics
* **Best Days:** Tuesday or Wednesday
* **Best Window:** 8:00 AM – 9:30 AM EST (1:00 PM – 2:30 PM UTC)
* **URL:** `https://github.com/1Yosh1/commandhelp` (or `https://commandhelp.dev`)

### Title Options
* **Recommended (Technical & Direct):**
  > `Show HN: Chelp – Fast, local-first CLI assistant and man-page parser in Rust`
* **Alternative (Problem-focused):**
  > `Show HN: Chelp – Zero-latency terminal AI with client-side secret redaction`

---

### Body Text (Markdown for Show HN)

```markdown
Hi HN! I built **chelp** (CommandHelp) — a single-binary Rust CLI tool that turns natural language into validated shell commands and gives you instant autocomplete for local CLI tools.

GitHub: https://github.com/1Yosh1/commandhelp
Docs & Demo: https://commandhelp.dev

### Why I built this
I love terminal workflows, but I constantly hit two friction points:
1. **Context switching & documentation lag:** I forget complex argument syntax for tools I use weekly (`tar`, `find`, `kubectl`, `ffmpeg`, `docker`). Searching Google or opening browser tabs breaks flow, while reading 800-line `--help` dumps takes too long.
2. **Terminal AI privacy & safety risks:** Most CLI AI tools pipe your raw terminal inputs and environment straight to a remote model without redaction. Worse, they blindly execute generated commands without warning you when a flag is destructive (`rm -rf`, `kubectl delete namespace`, `docker system prune -a`).

I wanted a tool that felt native to the shell: sub-50ms latency, zero keystroke logging, client-side secret scrubbing, and local documentation indexing that works even completely offline.

### How it works under the hood
1. **Local CLI Documentation Crawler & Parser (`src/parser.rs`):**
   Instead of scraping web pages, `chelp` inspects local binaries (`git`, `docker`, `kubectl`, `gh`, `tar`) and crawls `--help` outputs across GNU POSIX, Cobra, and Go conventions. It extracts subcommands, long flags, short aliases, and value hints into an indexed local SQLite database (`~/.local/share/chelp/cache.db`).

2. **Background IPC Daemon (`src/ipc.rs`):**
   To keep terminal prompt latency under 15ms, a lightweight background daemon runs in the background. Shell completions communicate with it over Unix domain sockets (Linux/macOS) or Named Pipes (Windows) via the `interprocess` crate. If the daemon isn't running, it auto-spawns transparently without blocking stdout/stderr.

3. **Client-Side Data Loss Prevention (DLP) (`src/dlp.rs`):**
   Before any prompt or shell context leaves your machine, a deterministic regex & entropy engine scrubs sensitive data:
   - AWS access/secret keys (`AKIA...`)
   - GitHub PATs (`ghp_...`, `gho_...`)
   - Slack tokens, private RSA/ED25519 keys, JWTs
   - Generic environment variables (`PASSWORD=...`, `SECRET=...`, `TOKEN=...`)
   Only sanitized tokens like `[REDACTED_AWS_KEY]` are passed to the model.

4. **Multi-Provider & Bring-Your-Own-Key (BYOK):**
   `chelp` has zero vendor lock-in. You can use:
   - Local offline models via **Ollama** (`qwen2.5-coder`, `llama3.2`) — 100% free, zero network packets.
   - Direct BYOK for Google Gemini, OpenAI, or Anthropic.
   - An optional hosted edge proxy on Cloudflare Workers for Pro users with ephemeral browser login (`chelp login`).

5. **Destructive Command Guardrails (`src/safety.rs`):**
   Every generated command is classified into `safe`, `caution`, or `destructive`. If a command mutates git history (`git reset --hard`), removes containers/volumes, or deletes namespaces, `chelp` highlights the risky flag in high-contrast red/amber and requires explicit confirmation.

### Installation
You can install via Homebrew, shell script, or build from source:

```bash
# macOS / Linux (Homebrew)
brew tap 1Yosh1/commandhelp
brew install chelp

# macOS / Linux (curl)
curl -fsSL https://raw.githubusercontent.com/1Yosh1/commandhelp/master/install.sh | sh

# Windows (PowerShell)
irm https://raw.githubusercontent.com/1Yosh1/commandhelp/master/install.ps1 | iex

# Or via Cargo
cargo install --path .
```

To bind it to your shell (`bash`, `zsh`, `fish`, `pwsh`):
```bash
# In ~/.zshrc or ~/.bashrc:
eval "$(chelp init zsh)"
```

Then type `? <what you want to do>` or run `chelp query "extract tar.gz to /tmp"`.

The codebase is open source (MIT/Apache), has 100% passing integration tests, and compiles across x86_64 and aarch64 on Linux, macOS, and Windows.

I'd love feedback on the parser heuristics and prompt format! What CLI utilities do you want parsed next?
```

---

## 2. Reddit — r/rust Technical Architecture Post

### Target Subreddit
* **r/rust**
* **Title:** `Building chelp: A sub-50ms CLI assistant in Rust with local IPC daemon and client-side DLP`

### Post Body
```markdown
Hey r/rust!

Over the past few weeks, I’ve been building **[chelp](https://github.com/1Yosh1/commandhelp)**, a single-binary CLI assistant and autocomplete engine. I wanted to share some technical architecture decisions, crate choices, and lessons learned while building a low-latency, cross-platform terminal utility.

### 1. Zero-Latency Terminal IPC: `interprocess` vs Detached Daemons
Running an LLM prompt or searching thousands of flags on every keystroke cannot block the shell prompt. If autocomplete takes >30ms, the shell feels sluggish.

We solved this with a split-binary architecture:
- Shell hooks send lightweight JSON-RPC requests to an IPC socket.
- On Unix (macOS, Linux), we use Unix Domain Sockets (`tokio` async UDS).
- On Windows, we use Named Pipes (`\\.\pipe\chelp-daemon-ipc`).
- We used the `interprocess` (v2.2+) crate which provides unified async traits over both.

**The Windows Detached Process Gotcha:**
In our early integration tests on Windows, spawning the background daemon via `std::process::Command` inherited the test runner's stdout/stderr pipe handles. This caused `assert_cmd` and `cargo test` to hang indefinitely waiting for the pipes to close!
The fix was ensuring stdio is explicitly redirected to `Stdio::null()` on Windows with `creation_flags(0x08000000)` (`CREATE_NO_WINDOW`), plus guarded tests with an env var override (`CHELP_NO_AUTO_SPAWN=1`).

### 2. Deterministic Client-Side DLP with Linear-Time Regexes
A core requirement was that secret keys (`ghp_`, `AKIA`, private keys, passwords) must **never** leave the developer's laptop, even if typed inside an intent prompt.

Rust’s `regex` crate is strictly linear-time ($O(mn)$) and explicitly rejects PCRE lookarounds (`(?!)`) and backreferences. To redact both quoted (`FOO="secret"`) and unquoted (`FOO=secret`) values safely without lookarounds or quadratic backtracking, we designed segmented character classes (`[^\[\]\s"']`) and pass tokens through a single-pass streaming redactor.

### 3. Parsing Heterogeneous `--help` Conventions
Command line tools have no universal documentation schema:
- GNU utilities use `--option=VALUE` or `-o VALUE`.
- Cobra/Go tools (`kubectl`, `gh`, `hugo`) use two spaces indentation and trailing flag descriptions.
- POSIX tools (`tar`, `ps`) often only have single-dash flags (`-x`, `-v`, `-f`).

We built a parser in `src/parser.rs` that tokenizes flag headers, extracts aliases (e.g., `-f, --file FILE`), detects boolean vs value-bearing flags, and persists the resulting schema to SQLite (`rusqlite` bundled) with fuzzy matching via `nucleo-matcher`.

### 4. Cross-Platform Release Matrix & Packaging
We automated the entire build and release matrix using GitHub Actions:
- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu`
- `x86_64-apple-darwin`
- `aarch64-apple-darwin` (Apple Silicon M1/M2/M3)
- `x86_64-pc-windows-msvc` & `x86_64-pc-windows-gnu`

We also wrote one-line self-extracting installers (`install.sh` and `install.ps1`) and a Homebrew Tap Formula (`Formula/chelp.rb`).

The project is fully open source: https://github.com/1Yosh1/commandhelp

Would love to hear feedback on the IPC and parser design!
```

---

## 3. Reddit — r/commandline & r/devops Post

### Target Subreddits
* **r/commandline** and **r/devops**
* **Title:** `Chelp – A local-first CLI tool to explain & construct complex terminal commands with safety guardrails`

### Post Body
```markdown
How often do you find yourself looking up:
- How to extract or create a `.tar.gz` with specific excludes
- Complex `find` commands with `-exec` and `mtime`
- `kubectl` jsonpath / custom-columns syntax
- `docker` and `podman` filtering strings
- `ffmpeg` audio/video stream mapping

I got tired of opening browser tabs or reading hundreds of lines of man pages, so I built **chelp** (CommandHelp).

### Key Features for DevOps / Terminal users:
- **Offline & Local-First:** Parses your installed local `--help` manuals into a fast SQLite database. Works without internet connection if paired with local Ollama (`qwen2.5-coder`).
- **Destructive Command Warnings:** Commands that mutate git history, wipe disk partitions, kill namespaces, or prune containers get flagged with safety warnings before you execute them.
- **Client-Side Secret Redaction:** Any credentials in your command context (`AWS_SECRET_KEY`, `ghp_...`, `.env` variables) are scrubbed locally before prompt resolution.
- **Works in Any Shell:** Native support for Zsh, Bash, Fish, and PowerShell.

```bash
# Query natural language:
chelp query "find all files over 500MB modified in last 7 days"

# Shell hotkey:
? extract archive.tar.bz2 to /opt/app
```

Check out the GitHub repo here: https://github.com/1Yosh1/commandhelp
Install via Homebrew: `brew install 1yosh1/commandhelp/chelp`
Or script: `curl -fsSL https://raw.githubusercontent.com/1Yosh1/commandhelp/master/install.sh | sh`

Any feedback or favorite command recipes you'd like to see supported are welcome!
```

---

## 4. Twitter / X Launch Thread

### Tweet 1 (Hook + Demo)
> 🚀 Introducing **chelp** — a fast, local-first CLI assistant and autocomplete engine built in Rust.
>
> ⚡ Sub-50ms shell response  
> 🔒 Client-side secret redaction (DLP)  
> 🛡️ Destructive command warnings  
> 📴 Works 100% offline with Ollama or BYOK  
>
> 🔗 https://github.com/1Yosh1/commandhelp  
>
> 🧵 Here’s why we built it and how it works under the hood 👇

### Tweet 2 (The Problem)
> Most terminal AI assistants get two things wrong:
> 1. They add 200ms+ lag to your prompt loop.
> 2. They pipe your raw terminal history, `.env` variables, and API keys straight to a remote server.
>
> `chelp` runs locally as a native compiled binary with a zero-friction background IPC daemon.

### Tweet 3 (Secret Redaction Engine)
> Security first: `chelp` includes an integrated Data Loss Prevention (DLP) engine.
>
> Before any query is processed, AWS tokens, GitHub PATs, Slack credentials, private keys, and passwords are automatically scrubbed on your CPU.
>
> Only sanitized tokens like `[REDACTED_AWS_KEY]` ever reach model inference.

### Tweet 4 (Destructive Command Guardrails)
> We’ve all felt that pit in our stomach after hitting Enter on a risky command.
>
> `chelp` inspects generated syntax and flags destructive actions (`rm -rf`, `git push -f`, `kubectl delete ns`, `docker prune`) with high-visibility warnings so you never accidentally wipe production state.

### Tweet 5 (Get Started in 10 Seconds)
> Try it today in your favorite shell (`zsh`, `bash`, `fish`, or `pwsh`):
>
> 🍺 `brew install 1yosh1/commandhelp/chelp`  
> 🐧 `curl -fsSL https://commandhelp.dev/install.sh | sh`  
> 🪟 `irm https://commandhelp.dev/install.ps1 | iex`  
>
> Star the repo on GitHub: https://github.com/1Yosh1/commandhelp ⭐
> Feedback & PRs welcome!

---

## 5. Hacker News Comment Response Playbook

When launching on Hacker News, expect tough, technical questions. Here are standard responses to common inquiries:

### Q1: "Why not just use GitHub Copilot CLI or an alias to llm/tgpt?"
> **Answer:**
> "Great question! Three main reasons:
> 1. **Latency & Shell Integration:** Most wrappers spin up a full Python/Node runtime on every query or pipe raw keystrokes over the internet. `chelp` is a compiled Rust binary with a local background daemon (`interprocess` IPC), giving sub-15ms prompt response times.
> 2. **Client-Side Secret Scrubbing:** Standard CLI wrappers pipe your exact terminal environment and arguments directly to cloud APIs. `chelp` runs a deterministic client-side DLP pass that removes AWS keys, GitHub tokens, and credentials before transmission.
> 3. **Local Tool Documentation Context:** Instead of guessing flags from general LLM training weights (which frequently hallucinate deprecated or non-existent flags), `chelp` parses your machine's installed `--help` docs locally so the prompt includes the actual valid flags for your specific tool version."

### Q2: "Isn't parsing `--help` output fragile across different CLI tools?"
> **Answer:**
> "It definitely can be if you rely on naive regexes! That's why in `src/parser.rs`, we built dedicated parsers for GNU POSIX conventions (`-f, --flag VALUE`), Go/Cobra style (`kubectl`, `gh`, `hugo`), and tokenized aliases. We maintain snapshot regression fixtures (`insta`) against real outputs from `kubectl`, `tar`, `gh`, and `git` to ensure compatibility across tool versions."

### Q3: "What telemetry or data collection does chelp have?"
> **Answer:**
> "`chelp` collects **zero telemetry**. No analytics pings, no usage tracking, no prompt logging. In BYOK mode, requests travel directly from your computer to your configured provider (OpenAI, Gemini, Anthropic, or local Ollama). In offline Ollama mode, zero packets ever leave your local network."
