# CommandHelp (`chelp`) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a high-performance, single-binary Rust CLI assistant (`chelp`) that dynamically parses `--help` documentation across any command-line tool, provides sub-millisecond autocomplete via local IPC, and translates natural-language requests into safe, interactive commands.

**Architecture:** Client-daemon architecture over local IPC (Named Pipe on Windows, Unix Domain Socket on Unix) backed by an in-memory Trie and SQLite schema storage. Dynamic heuristic `--help` parsing with safe timeouts, coupled with an interactive TUI preview modal for AI command verification before execution.

**Tech Stack:** Rust 2021, `tokio`, `interprocess`, `rusqlite`, `crossterm`, `ratatui`, `regex`, `reqwest`, `serde`, `serde_json`, `clap`.

**Spec:** `docs/superpowers/specs/2026-09-06-ai-cli-assistant-design.md`

## Global Constraints

* Single binary artifact: `chelp` (subcommands `daemon`, `complete`, `query`, `init`, `config`).
* Autocomplete latency budget: Maximum **15ms** IPC roundtrip response timeout; < 1ms local in-memory lookup.
* `--help` probe safety: Strict environment variables (`PAGER=cat`, `MANPAGER=cat`, `CI=true`, `TERM=dumb`) and **1,500ms** process execution timeout.
* Dangerous command defense: High-risk destructive commands (`rm -rf`, `DROP TABLE`, `mkfs`, force pushes) require explicit confirmation and cannot be executed with a single Enter press.
* Platform support: First-class Windows support (`pwsh` + Named Pipes) and Unix support (`zsh`/`bash` + Unix Domain Sockets).

---

### Task 1: Project Scaffolding & Core Domain Models

**Files:**
- Create: `Cargo.toml`
- Create: `src/lib.rs`
- Create: `src/models.rs`
- Create: `src/error.rs`
- Test: `tests/models_test.rs`

**Interfaces:**
- Consumes: None (foundational task)
- Produces:
  - `CliCommandSchema`: Struct with `binary: String`, `subcommand_path: Vec<String>`, `description: String`, `flags: Vec<CliFlag>`, `subcommands: Vec<String>`, `binary_mtime: u64`
  - `CliFlag`: Struct with `short: Option<String>`, `long: Option<String>`, `takes_value: bool`, `value_hint: Option<String>`, `description: String`
  - `SafetyLevel`: Enum `Safe`, `Caution`, `Destructive`
  - `AiCommandResponse`: Struct with `command: String`, `explanation: String`, `safety_level: SafetyLevel`, `destructive_warning: Option<String>`
  - `ChelpError`: Unified error enum using `thiserror`

- [ ] **Step 1: Write the failing test**

```rust
// tests/models_test.rs
use chelp::models::{CliCommandSchema, CliFlag, SafetyLevel, AiCommandResponse};

#[test]
fn test_schema_serialization() {
    let flag = CliFlag {
        short: Some("-v".to_string()),
        long: Some("--verbose".to_string()),
        takes_value: false,
        value_hint: None,
        description: "Show verbose output".to_string(),
    };

    let schema = CliCommandSchema {
        binary: "docker".to_string(),
        subcommand_path: vec!["container".to_string(), "ls".to_string()],
        usage: "docker container ls [OPTIONS]".to_string(),
        description: "List containers".to_string(),
        flags: vec![flag],
        subcommands: vec![],
        binary_mtime: 1700000000,
        last_indexed: 1700000000,
    };

    let json = serde_json::to_string(&schema).expect("serialization failed");
    let deserialized: CliCommandSchema = serde_json::from_str(&json).expect("deserialization failed");
    assert_eq!(deserialized.binary, "docker");
    assert_eq!(deserialized.flags.len(), 1);
    assert_eq!(deserialized.flags[0].long.as_deref(), Some("--verbose"));
}

#[test]
fn test_ai_response_safety_levels() {
    let resp = AiCommandResponse {
        command: "rm -rf /".to_string(),
        explanation: "Removes everything".to_string(),
        safety_level: SafetyLevel::Destructive,
        destructive_warning: Some("Deletes all files".to_string()),
    };

    let json = serde_json::to_string(&resp).unwrap();
    assert!(json.contains("destructive"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test models_test`  
Expected: FAIL with compilation error (modules `chelp`, `models` not found)

- [ ] **Step 3: Write minimal implementation**

```toml
# Cargo.toml
[package]
name = "chelp"
version = "0.1.0"
edition = "2021"
authors = ["chelp contributors"]
description = "Universal AI-Powered CLI Assistant & Autocomplete Engine"

[dependencies]
clap = { version = "4.5", features = ["derive"] }
tokio = { version = "1.38", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
thiserror = "1.0"
rusqlite = { version = "0.31", features = ["bundled"] }
regex = "1.10"
crossterm = "0.27"
ratatui = "0.26"
reqwest = { version = "0.12", features = ["json"] }
interprocess = "2.2"
nucleo-matcher = "0.3"
toml = "0.8"
directories = "5.0"
async-trait = "0.1"

[dev-dependencies]
tempfile = "3.10"
```

```rust
// src/error.rs
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ChelpError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("IPC error: {0}")]
    Ipc(String),
    #[error("Parser error: {0}")]
    Parser(String),
    #[error("AI provider error: {0}")]
    AiProvider(String),
    #[error("Configuration error: {0}")]
    Config(String),
}
```

```rust
// src/models.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct CliFlag {
    pub short: Option<String>,
    pub long: Option<String>,
    pub takes_value: bool,
    pub value_hint: Option<String>,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
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

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SafetyLevel {
    Safe,
    Caution,
    Destructive,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct AiCommandResponse {
    pub command: String,
    pub explanation: String,
    pub safety_level: SafetyLevel,
    pub destructive_warning: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ShellContext {
    pub os: String,
    pub shell: String,
    pub cwd: String,
}
```

```rust
// src/lib.rs
pub mod error;
pub mod models;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test models_test`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml src/ tests/
git commit -m "feat: setup project structure and core domain models"
```

---

### Task 2: Multi-Format `--help` Parser & Sandbox Crawler

**Files:**
- Create: `src/parser.rs`
- Create: `src/crawler.rs`
- Create: `tests/fixtures/git_help.txt`
- Create: `tests/fixtures/docker_help.txt`
- Test: `tests/parser_test.rs`

**Interfaces:**
- Consumes: `CliCommandSchema`, `CliFlag`, `ChelpError` from `models.rs`
- Produces:
  - `parse_help_output(binary: &str, subcommands: &[String], help_text: &str) -> Result<CliCommandSchema, ChelpError>`
  - `crawl_command_help(binary: &str, subcommands: &[String]) -> Result<String, ChelpError>` (executes with `PAGER=cat`, 1500ms timeout)

- [ ] **Step 1: Write the failing test**

```rust
// tests/parser_test.rs
use chelp::parser::parse_help_output;

#[test]
fn test_parse_gnu_and_cobra_flags() {
    let help_text = r#"
Usage: docker run [OPTIONS] IMAGE [COMMAND] [ARG...]

Run a command in a new container

Options:
  -d, --detach                         Run container in background and print container ID
  -e, --env list                       Set environment variables
  -p, --publish list                   Publish a container's port(s) to the host
      --rm                             Automatically remove the container when it exits
  -v, --volume list                    Bind mount a volume
      --name string                    Assign a name to the container

Commands:
  exec        Execute a command in a running container
  logs        Fetch the logs of a container
"#;

    let schema = parse_help_output("docker", &["run".to_string()], help_text).expect("parser failed");
    assert_eq!(schema.binary, "docker");
    assert_eq!(schema.subcommand_path, vec!["run".to_string()]);
    assert!(schema.flags.iter().any(|f| f.long.as_deref() == Some("--detach") && f.short.as_deref() == Some("-d")));
    assert!(schema.flags.iter().any(|f| f.long.as_deref() == Some("--rm") && !f.takes_value));
    assert!(schema.flags.iter().any(|f| f.long.as_deref() == Some("--name") && f.takes_value));
    assert!(schema.subcommands.contains(&"exec".to_string()));
    assert!(schema.subcommands.contains(&"logs".to_string()));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test parser_test`  
Expected: FAIL (module `chelp::parser` not found)

- [ ] **Step 3: Write minimal implementation**

```rust
// src/parser.rs
use crate::error::ChelpError;
use crate::models::{CliCommandSchema, CliFlag};
use regex::Regex;

pub fn parse_help_output(
    binary: &str,
    subcommands: &[String],
    help_text: &str,
) -> Result<CliCommandSchema, ChelpError> {
    let mut flags = Vec::new();
    let mut detected_subcommands = Vec::new();
    let mut description = String::new();
    let mut usage = String::new();

    // Regex for options/flags: e.g. "-d, --detach", "--rm", "-p, --publish list"
    let flag_re = Regex::new(r"^\s*(?:(-[a-zA-Z0-9]),?\s+)?(--[a-zA-Z0-9-_]+)(?:[=\s]+([a-zA-Z0-9_-]+))?\s{2,}(.*)$").unwrap();
    // Regex for subcommands: e.g. "  exec        Execute a command"
    let subcmd_re = Regex::new(r"^\s{2,}([a-zA-Z0-9_-]{2,20})\s{2,}(.*)$").unwrap();

    let mut in_commands_section = false;
    let mut in_options_section = false;

    for line in help_text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.to_lowercase().starts_with("usage:") {
            usage = trimmed.to_string();
            continue;
        }

        if trimmed.ends_with(':') {
            let header = trimmed.to_lowercase();
            if header.contains("command") || header.contains("subcommand") {
                in_commands_section = true;
                in_options_section = false;
                continue;
            } else if header.contains("option") || header.contains("flag") {
                in_options_section = true;
                in_commands_section = false;
                continue;
            }
        }

        if let Some(caps) = flag_re.captures(line) {
            let short = caps.get(1).map(|m| m.as_str().to_string());
            let long = caps.get(2).map(|m| m.as_str().to_string());
            let val_hint = caps.get(3).map(|m| m.as_str().to_string());
            let desc = caps.get(4).map(|m| m.as_str().trim().to_string()).unwrap_or_default();
            let takes_value = val_hint.is_some();

            flags.push(CliFlag {
                short,
                long,
                takes_value,
                value_hint: val_hint,
                description: desc,
            });
            continue;
        }

        if in_commands_section {
            if let Some(caps) = subcmd_re.captures(line) {
                let cmd_name = caps.get(1).map(|m| m.as_str().to_string()).unwrap();
                if !cmd_name.starts_with('-') {
                    detected_subcommands.push(cmd_name);
                }
            }
        } else if description.is_empty() && !trimmed.starts_with('-') && !trimmed.to_lowercase().starts_with("usage") {
            description = trimmed.to_string();
        }
    }

    Ok(CliCommandSchema {
        binary: binary.to_string(),
        subcommand_path: subcommands.to_vec(),
        usage,
        description,
        flags,
        subcommands: detected_subcommands,
        binary_mtime: 0,
        last_indexed: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    })
}
```

```rust
// src/crawler.rs
use crate::error::ChelpError;
use std::process::{Command, Stdio};
use std::time::Duration;

pub fn crawl_command_help(binary: &str, subcommands: &[String]) -> Result<String, ChelpError> {
    let mut args = subcommands.to_vec();
    args.push("--help".to_string());

    let mut child = Command::new(binary)
        .args(&args)
        .env("PAGER", "cat")
        .env("MANPAGER", "cat")
        .env("CI", "true")
        .env("TERM", "dumb")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| ChelpError::Parser(format!("Failed to execute '{}': {}", binary, e)))?;

    // Enforce 1500ms timeout
    let start = std::time::Instant::now();
    loop {
        match child.try_wait()? {
            Some(status) => {
                let output = child.wait_with_output()?;
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                if !stdout.is_empty() {
                    return Ok(stdout);
                }
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                return Ok(stderr);
            }
            None => {
                if start.elapsed() > Duration::from_millis(1500) {
                    let _ = child.kill();
                    return Err(ChelpError::Parser(format!("Timed out reading help from '{}'", binary)));
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}
```

Add `pub mod parser;` and `pub mod crawler;` to `src/lib.rs`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test parser_test`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/parser.rs src/crawler.rs tests/parser_test.rs src/lib.rs
git commit -m "feat: implement multi-format help parser and sandbox crawler"
```

---

### Task 3: SQLite Storage & Local In-Memory Schema Cache

**Files:**
- Create: `src/storage.rs`
- Test: `tests/storage_test.rs`

**Interfaces:**
- Consumes: `CliCommandSchema`, `CliFlag` from `models.rs`
- Produces:
  - `SchemaStore`:
    - `new(db_path: &Path) -> Result<SchemaStore, ChelpError>`
    - `save_schema(&self, schema: &CliCommandSchema) -> Result<(), ChelpError>`
    - `get_schema(&self, binary: &str, subcommands: &[String]) -> Result<Option<CliCommandSchema>, ChelpError>`
    - `match_flags(&self, binary: &str, subcommands: &[String], prefix: &str) -> Result<Vec<CliFlag>, ChelpError>`

- [ ] **Step 1: Write the failing test**

```rust
// tests/storage_test.rs
use chelp::models::{CliCommandSchema, CliFlag};
use chelp::storage::SchemaStore;
use tempfile::NamedTempFile;

#[test]
fn test_save_and_retrieve_schema() {
    let tmp_file = NamedTempFile::new().unwrap();
    let store = SchemaStore::new(tmp_file.path()).unwrap();

    let flag = CliFlag {
        short: Some("-p".to_string()),
        long: Some("--port".to_string()),
        takes_value: true,
        value_hint: Some("PORT".to_string()),
        description: "Port to bind".to_string(),
    };

    let schema = CliCommandSchema {
        binary: "testcli".to_string(),
        subcommand_path: vec!["serve".to_string()],
        usage: "testcli serve --port <PORT>".to_string(),
        description: "Start the server".to_string(),
        flags: vec![flag],
        subcommands: vec![],
        binary_mtime: 12345,
        last_indexed: 67890,
    };

    store.save_schema(&schema).expect("save failed");

    let loaded = store.get_schema("testcli", &["serve".to_string()]).unwrap();
    assert!(loaded.is_some());
    let loaded = loaded.unwrap();
    assert_eq!(loaded.binary, "testcli");
    assert_eq!(loaded.flags[0].long.as_deref(), Some("--port"));

    let matches = store.match_flags("testcli", &["serve".to_string()], "--p").unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].long.as_deref(), Some("--port"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test storage_test`  
Expected: FAIL (module `chelp::storage` not found)

- [ ] **Step 3: Write minimal implementation**

```rust
// src/storage.rs
use crate::error::ChelpError;
use crate::models::{CliCommandSchema, CliFlag};
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct SchemaStore {
    conn: Arc<Mutex<Connection>>,
}

impl SchemaStore {
    pub fn new(db_path: &Path) -> Result<Self, ChelpError> {
        let conn = Connection::open(db_path)?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS cli_schemas (
                binary TEXT NOT NULL,
                subcommand_path TEXT NOT NULL,
                schema_json TEXT NOT NULL,
                binary_mtime INTEGER NOT NULL,
                last_indexed INTEGER NOT NULL,
                PRIMARY KEY (binary, subcommand_path)
            )",
            [],
        )?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn save_schema(&self, schema: &CliCommandSchema) -> Result<(), ChelpError> {
        let subcmd_str = schema.subcommand_path.join(" ");
        let schema_json = serde_json::to_string(schema)?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO cli_schemas (binary, subcommand_path, schema_json, binary_mtime, last_indexed)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                schema.binary,
                subcmd_str,
                schema_json,
                schema.binary_mtime as i64,
                schema.last_indexed as i64
            ],
        )?;
        Ok(())
    }

    pub fn get_schema(&self, binary: &str, subcommands: &[String]) -> Result<Option<CliCommandSchema>, ChelpError> {
        let subcmd_str = subcommands.join(" ");
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT schema_json FROM cli_schemas WHERE binary = ?1 AND subcommand_path = ?2",
        )?;
        let mut rows = stmt.query(params![binary, subcmd_str])?;

        if let Some(row) = rows.next()? {
            let json: String = row.get(0)?;
            let schema: CliCommandSchema = serde_json::from_str(&json)?;
            Ok(Some(schema))
        } else {
            Ok(None)
        }
    }

    pub fn match_flags(&self, binary: &str, subcommands: &[String], prefix: &str) -> Result<Vec<CliFlag>, ChelpError> {
        if let Some(schema) = self.get_schema(binary, subcommands)? {
            let prefix_lower = prefix.to_lowercase();
            let matches = schema
                .flags
                .into_iter()
                .filter(|f| {
                    f.long.as_ref().map_or(false, |l| l.to_lowercase().starts_with(&prefix_lower))
                        || f.short.as_ref().map_or(false, |s| s.to_lowercase().starts_with(&prefix_lower))
                })
                .collect();
            Ok(matches)
        } else {
            Ok(vec![])
        }
    }
}
```

Add `pub mod storage;` to `src/lib.rs`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test storage_test`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/storage.rs tests/storage_test.rs src/lib.rs
git commit -m "feat: implement SQLite schema store and flag matching"
```

---

### Task 4: Local IPC Protocol & Daemon/Client Communication

**Files:**
- Create: `src/ipc.rs`
- Test: `tests/ipc_test.rs`

**Interfaces:**
- Consumes: `SchemaStore`, `CliFlag`
- Produces:
  - `IpcRequest`: Enum `Complete { buffer: String }`, `Query { prompt: String }`, `Ping`
  - `IpcResponse`: Enum `Suggestions { flags: Vec<CliFlag> }`, `Answer { text: String }`, `Pong`
  - `start_daemon(store: SchemaStore, socket_name: &str) -> JoinHandle<()>`
  - `send_ipc_request(socket_name: &str, req: &IpcRequest) -> Result<IpcResponse, ChelpError>`

- [ ] **Step 1: Write the failing test**

```rust
// tests/ipc_test.rs
use chelp::ipc::{IpcRequest, IpcResponse, send_ipc_request, start_daemon};
use chelp::storage::SchemaStore;
use tempfile::NamedTempFile;

#[tokio::test]
async fn test_ipc_roundtrip_ping() {
    let tmp_db = NamedTempFile::new().unwrap();
    let store = SchemaStore::new(tmp_db.path()).unwrap();
    let socket_name = format!("chelp-test-{}", std::process::id());

    let _server_handle = start_daemon(store, &socket_name).await.unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    let res = send_ipc_request(&socket_name, &IpcRequest::Ping).await.unwrap();
    assert_eq!(res, IpcResponse::Pong);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test ipc_test`  
Expected: FAIL (module `chelp::ipc` not found)

- [ ] **Step 3: Write minimal implementation**

```rust
// src/ipc.rs
use crate::error::ChelpError;
use crate::models::CliFlag;
use crate::storage::SchemaStore;
use interprocess::local_socket::tokio::prelude::*;
use interprocess::local_socket::{GenericNamespaced, ToNsName};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::task::JoinHandle;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "data")]
pub enum IpcRequest {
    Ping,
    Complete { buffer: String },
    Query { prompt: String },
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "data")]
pub enum IpcResponse {
    Pong,
    Suggestions { flags: Vec<CliFlag> },
    Answer { text: String },
    Error { message: String },
}

pub async fn send_ipc_request(name: &str, req: &IpcRequest) -> Result<IpcResponse, ChelpError> {
    let socket_name = name.to_ns_name::<GenericNamespaced>()
        .map_err(|e| ChelpError::Ipc(e.to_string()))?;

    let mut stream = LocalSocketStream::connect(socket_name)
        .await
        .map_err(|e| ChelpError::Ipc(format!("Cannot connect to daemon: {}", e)))?;

    let bytes = serde_json::to_vec(req)?;
    let len_prefix = (bytes.len() as u32).to_le_bytes();

    stream.write_all(&len_prefix).await?;
    stream.write_all(&bytes).await?;

    let mut res_len_buf = [0u8; 4];
    stream.read_exact(&mut res_len_buf).await?;
    let res_len = u32::from_le_bytes(res_len_buf) as usize;

    let mut res_buf = vec![0u8; res_len];
    stream.read_exact(&mut res_buf).await?;

    let res: IpcResponse = serde_json::from_slice(&res_buf)?;
    Ok(res)
}

pub async fn start_daemon(store: SchemaStore, name: &str) -> Result<JoinHandle<()>, ChelpError> {
    let socket_name = name.to_ns_name::<GenericNamespaced>()
        .map_err(|e| ChelpError::Ipc(e.to_string()))?;

    let listener = LocalSocketListener::bind(socket_name)
        .map_err(|e| ChelpError::Ipc(format!("Cannot bind socket: {}", e)))?;

    let handle = tokio::spawn(async move {
        while let Ok(mut stream) = listener.accept().await {
            let store = store.clone();
            tokio::spawn(async move {
                let mut len_buf = [0u8; 4];
                if stream.read_exact(&mut len_buf).await.is_err() {
                    return;
                }
                let len = u32::from_le_bytes(len_buf) as usize;
                let mut buf = vec![0u8; len];
                if stream.read_exact(&mut buf).await.is_err() {
                    return;
                }

                if let Ok(req) = serde_json::from_slice::<IpcRequest>(&buf) {
                    let res = match req {
                        IpcRequest::Ping => IpcResponse::Pong,
                        IpcRequest::Complete { buffer } => {
                            let parts: Vec<&str> = buffer.split_whitespace().collect();
                            if parts.is_empty() {
                                IpcResponse::Suggestions { flags: vec![] }
                            } else {
                                let binary = parts[0];
                                let last_token = if buffer.ends_with(' ') { "" } else { parts.last().unwrap_or(&"") };
                                let flags = store.match_flags(binary, &[], last_token).unwrap_or_default();
                                IpcResponse::Suggestions { flags }
                            }
                        }
                        IpcRequest::Query { prompt: _ } => IpcResponse::Answer {
                            text: "Not implemented yet".to_string(),
                        },
                    };

                    let res_bytes = serde_json::to_vec(&res).unwrap_or_default();
                    let res_len = (res_bytes.len() as u32).to_le_bytes();
                    let _ = stream.write_all(&res_len).await;
                    let _ = stream.write_all(&res_bytes).await;
                }
            });
        }
    });

    Ok(handle)
}
```

Add `pub mod ipc;` to `src/lib.rs`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test ipc_test`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/ipc.rs tests/ipc_test.rs src/lib.rs
git commit -m "feat: implement local IPC daemon and client transport"
```

---

### Task 5: Safety Guardrails & Deterministic Risk Analyzer

**Files:**
- Create: `src/safety.rs`
- Test: `tests/safety_test.rs`

**Interfaces:**
- Consumes: `SafetyLevel`, `AiCommandResponse` from `models.rs`
- Produces:
  - `evaluate_command_safety(command: &str) -> (SafetyLevel, Option<String>)`
  - `sanitize_and_verify(resp: &mut AiCommandResponse)`

- [ ] **Step 1: Write the failing test**

```rust
// tests/safety_test.rs
use chelp::models::SafetyLevel;
use chelp::safety::evaluate_command_safety;

#[test]
fn test_destructive_patterns() {
    let cases = vec![
        ("rm -rf /var/log", SafetyLevel::Destructive),
        ("Remove-Item -Recurse -Force C:\\temp", SafetyLevel::Destructive),
        ("del /f /s /q *.txt", SafetyLevel::Destructive),
        ("format D: /fs:NTFS", SafetyLevel::Destructive),
        ("dd if=/dev/zero of=/dev/sda", SafetyLevel::Destructive),
        ("DROP DATABASE production;", SafetyLevel::Destructive),
        ("git push origin main --force", SafetyLevel::Destructive),
        ("git reset --hard HEAD~1", SafetyLevel::Destructive),
    ];

    for (cmd, expected) in cases {
        let (level, warning) = evaluate_command_safety(cmd);
        assert_eq!(level, expected, "Failed for command: {}", cmd);
        assert!(warning.is_some(), "Expected warning message for: {}", cmd);
    }
}

#[test]
fn test_safe_and_caution_patterns() {
    assert_eq!(evaluate_command_safety("git status").0, SafetyLevel::Safe);
    assert_eq!(evaluate_command_safety("find . -name '*.rs'").0, SafetyLevel::Safe);
    assert_eq!(evaluate_command_safety("docker ps -a").0, SafetyLevel::Safe);
    assert_eq!(evaluate_command_safety("git push origin feature-branch").0, SafetyLevel::Caution);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test safety_test`  
Expected: FAIL (module `chelp::safety` not found)

- [ ] **Step 3: Write minimal implementation**

```rust
// src/safety.rs
use crate::models::{AiCommandResponse, SafetyLevel};
use regex::Regex;

pub fn evaluate_command_safety(command: &str) -> (SafetyLevel, Option<String>) {
    let destructive_patterns = [
        (r"(?i)\brm\s+-[a-zA-Z]*r[a-zA-Z]*\b", "Recursive file removal (rm -r) permanently deletes data."),
        (r"(?i)\bRemove-Item\b.*-Recurse", "PowerShell recursive item removal permanently deletes data."),
        (r"(?i)\bdel\b.*(/s|/q)", "Recursive/silent file deletion."),
        (r"(?i)\b(mkfs|format)\b", "Disk formatting completely wipes storage partitions."),
        (r"(?i)\bdd\b.*of=/dev/", "Raw disk write can overwrite boot sectors or partitions."),
        (r"(?i)\b(DROP\s+DATABASE|DROP\s+TABLE|TRUNCATE)\b", "Destructive SQL operation deletes database tables."),
        (r"(?i)\bgit\s+push\b.*(--force|-f)\b", "Git force-push overwrites remote repository history."),
        (r"(?i)\bgit\s+reset\s+--hard\b", "Hard git reset discards all uncommitted local changes."),
        (r"(?i)\bkill\s+-9\b", "SIGKILL forces processes to terminate without saving state."),
        (r"(?i)\bStop-Process\b.*-Force", "Forces process termination without cleanup."),
    ];

    for (pattern, warning) in destructive_patterns {
        let re = Regex::new(pattern).unwrap();
        if re.is_match(command) {
            return (SafetyLevel::Destructive, Some(warning.to_string()));
        }
    }

    let caution_patterns = [
        r"(?i)\bgit\s+push\b",
        r"(?i)\bdocker\s+(stop|rm|kill)\b",
        r"(?i)\b(npm|cargo|pip)\s+(install|uninstall)\b",
        r"(?i)\bchmod\b",
    ];

    for pattern in caution_patterns {
        let re = Regex::new(pattern).unwrap();
        if re.is_match(command) {
            return (SafetyLevel::Caution, None);
        }
    }

    (SafetyLevel::Safe, None)
}

pub fn sanitize_and_verify(resp: &mut AiCommandResponse) {
    let (computed_level, warning) = evaluate_command_safety(&resp.command);
    if computed_level == SafetyLevel::Destructive {
        resp.safety_level = SafetyLevel::Destructive;
        if resp.destructive_warning.is_none() {
            resp.destructive_warning = warning;
        }
    }
}
```

Add `pub mod safety;` to `src/lib.rs`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test safety_test`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/safety.rs tests/safety_test.rs src/lib.rs
git commit -m "feat: implement deterministic safety guardrails"
```

---

### Task 6: AI Provider Engine (BYOK: Gemini / Ollama) & Context Prompting

**Files:**
- Create: `src/ai.rs`
- Test: `tests/ai_test.rs`

**Interfaces:**
- Consumes: `CliCommandSchema`, `ShellContext`, `AiCommandResponse`
- Produces:
  - `AiProvider` trait with `resolve_intent(&self, prompt: &str, ctx: &ShellContext, schemas: &[CliCommandSchema]) -> Result<AiCommandResponse, ChelpError>`
  - `GeminiProvider::new(api_key: String)`
  - `OllamaProvider::new(endpoint: String, model: String)`
  - `build_prompt(prompt: &str, ctx: &ShellContext, schemas: &[CliCommandSchema]) -> String`

- [ ] **Step 1: Write the failing test**

```rust
// tests/ai_test.rs
use chelp::ai::build_prompt;
use chelp::models::{CliCommandSchema, ShellContext};

#[test]
fn test_build_prompt_includes_context() {
    let ctx = ShellContext {
        os: "windows".to_string(),
        shell: "pwsh".to_string(),
        cwd: "C:\\projects".to_string(),
    };

    let schema = CliCommandSchema {
        binary: "git".to_string(),
        subcommand_path: vec![],
        usage: "git [options] <command>".to_string(),
        description: "Fast version control system".to_string(),
        flags: vec![],
        subcommands: vec!["commit".to_string(), "push".to_string()],
        binary_mtime: 0,
        last_indexed: 0,
    };

    let prompt = build_prompt("commit with message wip", &ctx, &[schema]);
    assert!(prompt.contains("Target OS: windows"));
    assert!(prompt.contains("Target Shell: pwsh"));
    assert!(prompt.contains("Tool Context: git"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test ai_test`  
Expected: FAIL (module `chelp::ai` not found)

- [ ] **Step 3: Write minimal implementation**

```rust
// src/ai.rs
use crate::error::ChelpError;
use crate::models::{AiCommandResponse, CliCommandSchema, ShellContext};
use crate::safety::sanitize_and_verify;
use async_trait::async_trait;

pub fn build_prompt(user_query: &str, ctx: &ShellContext, schemas: &[CliCommandSchema]) -> String {
    let mut schema_summary = String::new();
    for s in schemas {
        schema_summary.push_str(&format!("Tool Context: {} (Subcommands: {})\n", s.binary, s.subcommands.join(", ")));
        for f in &s.flags {
            if let Some(long) = &f.long {
                schema_summary.push_str(&format!("  {} : {}\n", long, f.description));
            }
        }
    }

    format!(
r#"You are CommandHelp AI, an expert CLI assistant.
Target OS: {}
Target Shell: {}
Working Directory: {}

Available Tool Specs:
{}

User Intent: "{}"

Respond strictly with a JSON object matching this schema:
{{
  "command": "<exact runnable command line string>",
  "explanation": "<1-2 sentence explanation>",
  "safety_level": "safe" | "caution" | "destructive",
  "destructive_warning": null or "<warning message>"
}}
No markdown formatting, no code backticks. Just the raw JSON object."#,
        ctx.os, ctx.shell, ctx.cwd, schema_summary, user_query
    )
}

#[async_trait]
pub trait AiProvider: Send + Sync {
    async fn resolve_intent(
        &self,
        query: &str,
        ctx: &ShellContext,
        schemas: &[CliCommandSchema],
    ) -> Result<AiCommandResponse, ChelpError>;
}

pub struct GeminiProvider {
    pub api_key: String,
    pub client: reqwest::Client,
}

impl GeminiProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl AiProvider for GeminiProvider {
    async fn resolve_intent(
        &self,
        query: &str,
        ctx: &ShellContext,
        schemas: &[CliCommandSchema],
    ) -> Result<AiCommandResponse, ChelpError> {
        let prompt = build_prompt(query, ctx, schemas);
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent?key={}",
            self.api_key
        );

        let body = serde_json::json!({
            "contents": [{
                "parts": [{ "text": prompt }]
            }],
            "generationConfig": {
                "response_mime_type": "application/json"
            }
        });

        let res = self.client.post(&url).json(&body).send().await
            .map_err(|e| ChelpError::AiProvider(format!("HTTP request failed: {}", e)))?;

        let json_res: serde_json::Value = res.json().await
            .map_err(|e| ChelpError::AiProvider(format!("Failed to parse response: {}", e)))?;

        let text = json_res["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .ok_or_else(|| ChelpError::AiProvider("Empty model response".to_string()))?;

        let mut ai_resp: AiCommandResponse = serde_json::from_str(text)
            .map_err(|e| ChelpError::AiProvider(format!("Invalid response JSON: {}", e)))?;

        sanitize_and_verify(&mut ai_resp);
        Ok(ai_resp)
    }
}
```

Add `pub mod ai;` to `src/lib.rs`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test ai_test`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/ai.rs tests/ai_test.rs src/lib.rs
git commit -m "feat: implement AI prompt builder and Gemini provider"
```

---

### Task 7: Interactive Confirmation & Execution TUI Overlay

**Files:**
- Create: `src/tui.rs`
- Test: `tests/tui_test.rs`

**Interfaces:**
- Consumes: `AiCommandResponse`, `SafetyLevel`
- Produces:
  - `enum UserAction`: `Run(String)`, `Edit(String)`, `Cancel`
  - `render_interactive_confirmation(resp: &AiCommandResponse) -> Result<UserAction, ChelpError>`

- [ ] **Step 1: Write the failing test**

```rust
// tests/tui_test.rs
use chelp::models::{AiCommandResponse, SafetyLevel};
use chelp::tui::UserAction;

#[test]
fn test_user_action_variants() {
    let action_run = UserAction::Run("git status".to_string());
    let action_edit = UserAction::Edit("git status".to_string());
    let action_cancel = UserAction::Cancel;

    match action_run {
        UserAction::Run(cmd) => assert_eq!(cmd, "git status"),
        _ => panic!("Expected Run"),
    }
    match action_edit {
        UserAction::Edit(cmd) => assert_eq!(cmd, "git status"),
        _ => panic!("Expected Edit"),
    }
    assert_eq!(action_cancel, UserAction::Cancel);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test tui_test`  
Expected: FAIL (module `chelp::tui` not found)

- [ ] **Step 3: Write minimal implementation**

```rust
// src/tui.rs
use crate::error::ChelpError;
use crate::models::{AiCommandResponse, SafetyLevel};
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Terminal,
};
use std::io::stdout;

#[derive(Debug, PartialEq, Eq)]
pub enum UserAction {
    Run(String),
    Edit(String),
    Cancel,
}

pub fn render_interactive_confirmation(resp: &AiCommandResponse) -> Result<UserAction, ChelpError> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut action = UserAction::Cancel;

    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // Command
                    Constraint::Length(3), // Explanation
                    Constraint::Length(3), // Safety & Warning
                    Constraint::Length(3), // Controls
                ])
                .split(f.size());

            // 1. Command Block
            let cmd_p = Paragraph::new(resp.command.clone())
                .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
                .block(Block::default().borders(Borders::ALL).title(" Suggested Command "));
            f.render_widget(cmd_p, chunks[0]);

            // 2. Explanation Block
            let exp_p = Paragraph::new(resp.explanation.clone())
                .style(Style::default().fg(Color::White))
                .wrap(Wrap { trim: true })
                .block(Block::default().borders(Borders::ALL).title(" Explanation "));
            f.render_widget(exp_p, chunks[1]);

            // 3. Safety Block
            let (safety_text, safety_style) = match resp.safety_level {
                SafetyLevel::Safe => ("● SAFE (Read-Only)", Style::default().fg(Color::Green)),
                SafetyLevel::Caution => ("● CAUTION (State Change)", Style::default().fg(Color::Yellow)),
                SafetyLevel::Destructive => ("▲ HIGH RISK / DESTRUCTIVE ACTION", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            };

            let warning_line = if let Some(w) = &resp.destructive_warning {
                format!(" - {}", w)
            } else {
                String::new()
            };

            let safety_p = Paragraph::new(Line::from(vec![
                Span::styled(safety_text, safety_style),
                Span::raw(warning_line),
            ]))
            .block(Block::default().borders(Borders::ALL).title(" Safety Assessment "));
            f.render_widget(safety_p, chunks[2]);

            // 4. Controls Block
            let controls = Line::from(vec![
                Span::styled("[Enter] ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw("Run in shell   "),
                Span::styled("[Tab / e] ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw("Edit on prompt   "),
                Span::styled("[Esc] ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw("Cancel"),
            ]);
            let ctrl_p = Paragraph::new(controls)
                .block(Block::default().borders(Borders::ALL).title(" Actions "));
            f.render_widget(ctrl_p, chunks[3]);
        })?;

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Enter => {
                    action = UserAction::Run(resp.command.clone());
                    break;
                }
                KeyCode::Tab | KeyCode::Char('e') => {
                    action = UserAction::Edit(resp.command.clone());
                    break;
                }
                KeyCode::Esc | KeyCode::Char('q') => {
                    action = UserAction::Cancel;
                    break;
                }
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    action = UserAction::Cancel;
                    break;
                }
                _ => {}
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(action)
}
```

Add `pub mod tui;` to `src/lib.rs`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test tui_test`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/tui.rs tests/tui_test.rs src/lib.rs
git commit -m "feat: implement interactive confirmation TUI modal"
```

---

### Task 8: Shell Hook Generator & Unified CLI Dispatcher

**Files:**
- Create: `src/shell.rs`
- Create: `src/main.rs`
- Test: `tests/shell_test.rs`

**Interfaces:**
- Consumes: `start_daemon`, `send_ipc_request`, `render_interactive_confirmation`, `AiProvider`
- Produces:
  - `generate_hook_script(shell: &str) -> Result<String, ChelpError>`
  - Complete CLI executable `chelp` with subcommands: `daemon`, `complete`, `query`, `init`, `config`

- [ ] **Step 1: Write the failing test**

```rust
// tests/shell_test.rs
use chelp::shell::generate_hook_script;

#[test]
fn test_shell_script_generation() {
    let pwsh = generate_hook_script("pwsh").unwrap();
    assert!(pwsh.contains("Set-PSReadLineKeyHandler"));
    assert!(pwsh.contains("chelp complete"));

    let zsh = generate_hook_script("zsh").unwrap();
    assert!(zsh.contains("zle-line-update"));
    assert!(zsh.contains("chelp-query"));

    let bash = generate_hook_script("bash").unwrap();
    assert!(bash.contains("bind -x"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test shell_test`  
Expected: FAIL (module `chelp::shell` not found)

- [ ] **Step 3: Write minimal implementation**

```rust
// src/shell.rs
use crate::error::ChelpError;

pub fn generate_hook_script(shell: &str) -> Result<String, ChelpError> {
    match shell.to_lowercase().as_str() {
        "pwsh" | "powershell" => Ok(r#"
# CommandHelp PowerShell Hook
Set-PSReadLineKeyHandler -Chord 'Ctrl+Space' -ScriptBlock {
    $line = $null
    $cursor = $null
    [Microsoft.PowerShell.PSConsoleReadLine]::GetBufferState([ref]$line, [ref]$cursor)
    $result = chelp query "$line"
    if ($result) {
        [Microsoft.PowerShell.PSConsoleReadLine]::RevertLine()
        [Microsoft.PowerShell.PSConsoleReadLine]::Insert($result)
    }
}
"#.trim().to_string()),

        "zsh" => Ok(r#"
# CommandHelp Zsh Hook
chelp-query() {
    local cmd=$(chelp query "$BUFFER")
    if [[ -n "$cmd" ]]; then
        BUFFER="$cmd"
        CURSOR=${#BUFFER}
    fi
    zle redisplay
}
zle -N chelp-query
bindkey '^ ' chelp-query
"#.trim().to_string()),

        "bash" => Ok(r#"
# CommandHelp Bash Hook
_chelp_query() {
    local cmd=$(chelp query "$READLINE_LINE")
    if [[ -n "$cmd" ]]; then
        READLINE_LINE="$cmd"
        READLINE_POINT=${#READLINE_LINE}
    fi
}
bind -x '"\C-@": _chelp_query'
"#.trim().to_string()),

        _ => Err(ChelpError::Config(format!("Unsupported shell: {}", shell))),
    }
}
```

```rust
// src/main.rs
use clap::{Parser, Subcommand};
use chelp::ai::{GeminiProvider, AiProvider};
use chelp::error::ChelpError;
use chelp::ipc::{send_ipc_request, start_daemon, IpcRequest, IpcResponse};
use chelp::models::ShellContext;
use chelp::shell::generate_hook_script;
use chelp::storage::SchemaStore;
use chelp::tui::{render_interactive_confirmation, UserAction};
use directories::ProjectDirs;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "chelp", about = "Universal AI-Powered CLI Assistant & Autocomplete Engine")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the background daemon
    Daemon {
        #[arg(long)]
        detached: bool,
    },
    /// Request completion for current command line buffer
    Complete {
        buffer: String,
    },
    /// Query the AI assistant with natural language
    Query {
        prompt: String,
    },
    /// Generate shell hook script (pwsh, zsh, bash)
    Init {
        shell: String,
    },
    /// Configure API keys and models
    Config,
}

fn get_db_path() -> PathBuf {
    let proj_dirs = ProjectDirs::from("dev", "chelp", "chelp")
        .expect("Cannot determine user directory");
    let dir = proj_dirs.data_dir();
    std::fs::create_dir_all(dir).unwrap_or_default();
    dir.join("data.db")
}

#[tokio::main]
async fn main() -> Result<(), ChelpError> {
    let cli = Cli::parse();
    let socket_name = "chelp-ipc";

    match cli.command {
        Commands::Daemon { detached: _ } => {
            let store = SchemaStore::new(&get_db_path())?;
            println!("Starting chelp daemon on {}", socket_name);
            let handle = start_daemon(store, socket_name).await?;
            handle.await.map_err(|e| ChelpError::Ipc(e.to_string()))?;
        }
        Commands::Complete { buffer } => {
            match send_ipc_request(socket_name, &IpcRequest::Complete { buffer }).await {
                Ok(IpcResponse::Suggestions { flags }) => {
                    for f in flags {
                        if let Some(long) = f.long {
                            println!("{}\t{}", long, f.description);
                        }
                    }
                }
                _ => {}
            }
        }
        Commands::Query { prompt } => {
            let api_key = std::env::var("GEMINI_API_KEY").unwrap_or_default();
            if api_key.is_empty() {
                eprintln!("Error: GEMINI_API_KEY environment variable is not set.");
                eprintln!("Run 'chelp config' or set GEMINI_API_KEY to enable AI queries.");
                return Ok(());
            }

            let provider = GeminiProvider::new(api_key);
            let ctx = ShellContext {
                os: std::env::consts::OS.to_string(),
                shell: std::env::var("SHELL").unwrap_or_else(|_| "pwsh".to_string()),
                cwd: std::env::current_dir()?.to_string_lossy().to_string(),
            };

            let store = SchemaStore::new(&get_db_path())?;
            let resp = provider.resolve_intent(&prompt, &ctx, &[]).await?;

            match render_interactive_confirmation(&resp)? {
                UserAction::Run(cmd) => {
                    println!("{}", cmd);
                }
                UserAction::Edit(cmd) => {
                    println!("{}", cmd);
                }
                UserAction::Cancel => {}
            }
        }
        Commands::Init { shell } => {
            let script = generate_hook_script(&shell)?;
            println!("{}", script);
        }
        Commands::Config => {
            println!("Configuration file located at: {:?}", get_db_path());
            println!("Set GEMINI_API_KEY or configure local Ollama endpoint in ~/.chelp/config.toml");
        }
    }

    Ok(())
}
```

Add `pub mod shell;` to `src/lib.rs`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test shell_test`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/shell.rs src/main.rs tests/shell_test.rs src/lib.rs
git commit -m "feat: implement shell hook generator and unified CLI dispatcher"
```
