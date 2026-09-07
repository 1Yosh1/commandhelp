# Task 2: Multi-Format `--help` Parser & Sandbox Crawler

**Plan File:** `docs/superpowers/plans/2026-09-06-ai-cli-assistant.md`  
**Spec File:** `docs/superpowers/specs/2026-09-06-ai-cli-assistant-design.md`

## Files
- Create: `src/parser.rs`
- Create: `src/crawler.rs`
- Create: `tests/fixtures/git_help.txt`
- Create: `tests/fixtures/docker_help.txt`
- Test: `tests/parser_test.rs`

## Interfaces
- Consumes: `CliCommandSchema`, `CliFlag`, `ChelpError` from `models.rs` and `error.rs`
- Produces:
  - `parse_help_output(binary: &str, subcommands: &[String], help_text: &str) -> Result<CliCommandSchema, ChelpError>`
  - `crawl_command_help(binary: &str, subcommands: &[String]) -> Result<String, ChelpError>` (enforces `PAGER=cat`, `MANPAGER=cat`, `CI=true`, `TERM=dumb`, 1500ms timeout)

## Steps

### Step 1: Write the failing test

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

### Step 2: Run test to verify it fails

Run: `cargo test --test parser_test`  
Expected: FAIL (module `chelp::parser` not found)

### Step 3: Write minimal implementation

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

    let flag_re = Regex::new(r"^\s*(?:(-[a-zA-Z0-9]),?\s+)?(--[a-zA-Z0-9-_]+)(?:[=\s]+([a-zA-Z0-9_-]+))?\s{2,}(.*)$").unwrap();
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

    let start = std::time::Instant::now();
    loop {
        match child.try_wait()? {
            Some(_status) => {
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

### Step 4: Run test to verify it passes

Run: `cargo test --test parser_test`  
Expected: PASS

### Step 5: Commit

```bash
git add src/parser.rs src/crawler.rs tests/parser_test.rs src/lib.rs
git commit -m "feat: implement multi-format help parser and sandbox crawler"
```
