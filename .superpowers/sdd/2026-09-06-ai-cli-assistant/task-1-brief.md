# Task 1: Project Scaffolding & Core Domain Models

**Plan File:** `docs/superpowers/plans/2026-09-06-ai-cli-assistant.md`  
**Spec File:** `docs/superpowers/specs/2026-09-06-ai-cli-assistant-design.md`

## Files
- Create: `Cargo.toml`
- Create: `src/lib.rs`
- Create: `src/models.rs`
- Create: `src/error.rs`
- Test: `tests/models_test.rs`

## Interfaces
- Consumes: None (foundational task)
- Produces:
  - `CliCommandSchema`: Struct with `binary: String`, `subcommand_path: Vec<String>`, `description: String`, `flags: Vec<CliFlag>`, `subcommands: Vec<String>`, `binary_mtime: u64`
  - `CliFlag`: Struct with `short: Option<String>`, `long: Option<String>`, `takes_value: bool`, `value_hint: Option<String>`, `description: String`
  - `SafetyLevel`: Enum `Safe`, `Caution`, `Destructive`
  - `AiCommandResponse`: Struct with `command: String`, `explanation: String`, `safety_level: SafetyLevel`, `destructive_warning: Option<String>`
  - `ShellContext`: Struct with `os: String`, `shell: String`, `cwd: String`
  - `ChelpError`: Unified error enum using `thiserror`

## Steps

### Step 1: Write the failing test

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

### Step 2: Run test to verify it fails

Run: `cargo test --test models_test`  
Expected: FAIL with compilation error (modules `chelp`, `models` not found)

### Step 3: Write minimal implementation

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

### Step 4: Run test to verify it passes

Run: `cargo test --test models_test`  
Expected: PASS

### Step 5: Commit

```bash
git add Cargo.toml src/ tests/
git commit -m "feat: setup project structure and core domain models"
```
