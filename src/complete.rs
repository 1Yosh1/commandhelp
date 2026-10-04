// src/complete.rs
//
// Owns *what* a completion means: how a command line buffer is split into
// binary / subcommand path / token being typed, which schema answers it, how
// candidates become full command lines, and when a binary gets indexed.
// `ipc.rs` only carries these strings.

use crate::crawler::crawl_schema;
use crate::log::log;
use crate::models::CliCommandSchema;
use crate::storage::SchemaStore;

/// Upper bound on returned candidates so the shell hook never floods the line.
const MAX_SUGGESTIONS: usize = 8;

#[derive(Debug, PartialEq, Eq)]
pub struct BufferParts {
    /// First word of the line, e.g. `docker`.
    pub binary: String,
    /// Words after the binary and before the token being typed, e.g. `["run"]`.
    pub subcommand_path: Vec<String>,
    /// The token currently being typed (empty when the line ends with a space).
    pub last_token: String,
    /// Everything before `last_token`; candidates are appended to this.
    pub base: String,
}

/// Splits a raw shell buffer into the parts an indexer/completer needs.
pub fn parse_buffer(buffer: &str) -> Option<BufferParts> {
    let tokens: Vec<&str> = buffer.split_whitespace().collect();
    if tokens.is_empty() {
        return None;
    }

    let binary = tokens[0].to_string();
    let trailing_space = buffer.ends_with(' ') || buffer.ends_with('\t');

    let (subcommand_path, last_token, base) = if trailing_space {
        // `docker run ` -> every word after the binary is already committed.
        let path: Vec<String> = tokens[1..].iter().map(|s| (*s).to_string()).collect();
        let mut base = tokens.join(" ");
        base.push(' ');
        (path, String::new(), base)
    } else if tokens.len() == 1 {
        // `gi` -> still typing the binary itself; nothing to append to.
        (Vec::new(), tokens[0].to_string(), String::new())
    } else {
        // `docker run --` -> `--` is being typed, everything before it is fixed.
        let path: Vec<String> = tokens[1..tokens.len() - 1]
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        let mut base = tokens[..tokens.len() - 1].join(" ");
        base.push(' ');
        (path, tokens[tokens.len() - 1].to_string(), base)
    };

    Some(BufferParts {
        binary,
        subcommand_path,
        last_token,
        base,
    })
}

/// Builds full command lines (best first) for a buffer. Candidates come from
/// subcommands first — they are what the user usually wants next — then flags,
/// and only ever extend what was already typed.
pub fn suggestions(store: &SchemaStore, buffer: &str) -> Vec<String> {
    let Some(parts) = parse_buffer(buffer) else {
        return Vec::new();
    };
    let Some(schema) = store
        .get_best_schema(&parts.binary, &parts.subcommand_path)
        .ok()
        .flatten()
    else {
        return Vec::new();
    };

    let mut out: Vec<String> = Vec::new();
    let push = |candidate: &str, out: &mut Vec<String>| {
        if candidate.is_empty() || !candidate.starts_with(&parts.last_token) {
            return;
        }
        let line = format!("{}{}", parts.base, candidate);
        if line.len() > buffer.len() && line.starts_with(&parts.base) && !out.contains(&line) {
            out.push(line);
        }
    };

    for sub in &schema.subcommands {
        push(sub, &mut out);
        if out.len() >= MAX_SUGGESTIONS {
            return out;
        }
    }
    for flag in &schema.flags {
        if let Some(long) = &flag.long {
            push(long, &mut out);
        } else if let Some(short) = &flag.short {
            push(short, &mut out);
        }
        if out.len() >= MAX_SUGGESTIONS {
            break;
        }
    }
    out
}

fn needs_index(store: &SchemaStore, binary: &str, path: &[String]) -> bool {
    match store.get_schema(binary, path) {
        Ok(Some(schema)) => {
            let live = crate::crawler::binary_mtime(binary);
            // mtime differs when the binary was upgraded; `0` means unresolvable
            // (e.g. a placeholder for a binary that was missing).
            live != 0 && schema.binary_mtime != live
        }
        _ => true,
    }
}

/// Spec §4.1 non-blocking discovery: if the buffer's binary (or subcommand
/// level) is unknown or stale, queue the crawl in the background and answer
/// this request from whatever is already indexed — the user never waits on
/// `--help` parsing.
pub fn ensure_indexed(store: &SchemaStore, buffer: &str) {
    let Some(parts) = parse_buffer(buffer) else {
        return;
    };

    let mut missing: Vec<Vec<String>> = Vec::new();
    if needs_index(store, &parts.binary, &[]) {
        missing.push(Vec::new());
    }
    if !parts.subcommand_path.is_empty()
        && needs_index(store, &parts.binary, &parts.subcommand_path)
    {
        missing.push(parts.subcommand_path.clone());
    }
    if missing.is_empty() {
        return;
    }

    let binary = parts.binary.clone();
    let store = store.clone();
    std::thread::spawn(move || {
        for path in missing {
            let label = if path.is_empty() {
                binary.clone()
            } else {
                format!("{} {}", binary, path.join(" "))
            };
            match crawl_schema(&binary, &path) {
                Ok(schema) => {
                    let flags = schema.flags.len();
                    let subs = schema.subcommands.len();
                    if let Err(e) = store.save_schema(&schema) {
                        log("daemon", &format!("failed to store {}: {}", label, e));
                    } else {
                        log(
                            "daemon",
                            &format!("indexed {} ({} flags, {} subcommands)", label, flags, subs),
                        );
                    }
                }
                Err(e) => {
                    log("daemon", &format!("skipped {}: {}", label, e));
                    // Record the attempt so an absent binary is not re-probed on
                    // every keystroke; a later install changes mtime and retriggers.
                    if store.get_schema(&binary, &path).ok().flatten().is_none() {
                        let placeholder = CliCommandSchema {
                            binary: binary.clone(),
                            subcommand_path: path.clone(),
                            usage: String::new(),
                            description: "skipped".to_string(),
                            flags: Vec::new(),
                            subcommands: Vec::new(),
                            binary_mtime: crate::crawler::binary_mtime(&binary),
                            last_indexed: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .map(|d| d.as_secs())
                                .unwrap_or_default(),
                        };
                        let _ = store.save_schema(&placeholder);
                    }
                }
            }
        }
    });
}

/// One entry point for the daemon's `Complete` request.
pub fn handle_request(store: &SchemaStore, buffer: &str) -> Vec<String> {
    ensure_indexed(store, buffer);
    suggestions(store, buffer)
}

/// Tool schemas referenced by a natural-language prompt, for prompt assembly
/// (spec §5.3 "relevant parsed flags for any detected tools").
pub fn schemas_for_prompt(store: &SchemaStore, prompt: &str) -> Vec<CliCommandSchema> {
    let mut found = Vec::new();
    for token in prompt.split_whitespace() {
        let token = token.trim_matches(|c: char| !c.is_alphanumeric() && c != '-' && c != '_');
        if token.is_empty() || found.iter().any(|s: &CliCommandSchema| s.binary == token) {
            continue;
        }
        if let Ok(Some(schema)) = store.get_schema(token, &[]) {
            if !schema.flags.is_empty() || !schema.subcommands.is_empty() {
                found.push(schema);
            }
        }
    }
    found
}
