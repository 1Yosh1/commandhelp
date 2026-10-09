use crate::error::ChelpError;
use crate::models::{CliCommandSchema, CliFlag};
use regex::Regex;
use std::sync::OnceLock;

/// Switches that only appear bracketed inside usage/syntax blocks rather than
/// in an options list: `usage: git [-v | --version] [-C <path>]`, PowerShell's
/// `Get-ChildItem [[-Path] <string>] [-Filter <string>]`.
///
/// Returns `(name, takes_value)` for every dash-prefixed token in the bracket.
fn bracketed_switches(line: &str) -> Vec<(String, bool)> {
    static SPLIT: OnceLock<Regex> = OnceLock::new();
    let split = SPLIT.get_or_init(|| Regex::new(r"[|,\s]+").unwrap());

    let mut found = Vec::new();
    for group in line.split('[').skip(1) {
        let inner = &group[..group.find(']').unwrap_or(group.len())];
        if inner.is_empty() {
            continue;
        }
        let takes_value = inner.contains('<') || inner.contains('=');
        for token in split.split(inner) {
            let token = token.trim();
            let mut name = token.trim_end_matches(',');
            if let Some((flag, _)) = name.split_once('=') {
                name = flag;
            }
            if name.len() < 2 || !name.starts_with('-') {
                continue;
            }
            if !name[1..].starts_with(|c: char| c.is_ascii_alphanumeric()) {
                continue;
            }
            found.push((name.to_string(), takes_value));
        }
    }
    found
}

pub fn parse_help_output(
    binary: &str,
    subcommands: &[String],
    help_text: &str,
) -> Result<CliCommandSchema, ChelpError> {
    let mut flags = Vec::new();
    let mut detected_subcommands = Vec::new();
    let mut description = String::new();
    let mut usage = String::new();

    // Matches subcommands: "  create      Create a resource", "  auth:    Authenticate"
    let subcmd_re = Regex::new(r"^\s{2,}([a-zA-Z0-9_-]+):?\s{2,}(.*)$").unwrap();

    // Splits "-f, --flag   Description" on runs of two or more spaces. Built
    // once per parse instead of recompiled for every flag line.
    let split_pattern = Regex::new(r"\s{2,}").unwrap();

    let mut in_commands_section = false;
    let mut in_options_section = false;

    for line in help_text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Switches that only appear bracketed in usage/syntax blocks: the
        // `usage:` line itself (git, curl, terraform) and PowerShell's Get-Help
        // SYNTAX blocks (spec §4.3).
        for (name, takes_value) in bracketed_switches(trimmed) {
            let known = flags.iter().any(|f: &CliFlag| {
                f.long.as_deref() == Some(name.as_str())
                    || f.short.as_deref() == Some(name.as_str())
            });
            if known {
                continue;
            }
            if name.starts_with("--") {
                flags.push(CliFlag {
                    short: None,
                    long: Some(name),
                    takes_value,
                    value_hint: None,
                    description: String::new(),
                });
            } else {
                flags.push(CliFlag {
                    short: Some(name),
                    long: None,
                    takes_value,
                    value_hint: None,
                    description: String::new(),
                });
            }
        }

        let lower = trimmed.to_lowercase();
        if lower.starts_with("usage:") || lower.starts_with("usage") {
            if usage.is_empty() {
                usage = trimmed.to_string();
            }
            continue;
        }

        // Detect section headers
        let is_all_caps = trimmed.len() >= 4
            && trimmed
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_whitespace());
        if trimmed.ends_with(':') || is_all_caps {
            if lower.contains("command") {
                in_commands_section = true;
                in_options_section = false;
                continue;
            } else if lower.contains("option")
                || lower.contains("flag")
                || lower.contains("mode")
                || lower.contains("modifier")
            {
                in_options_section = true;
                in_commands_section = false;
                continue;
            }
        }

        // Parse flags if line starts with '-'
        if trimmed.starts_with('-') {
            let parts: Vec<&str> = split_pattern.splitn(trimmed, 2).collect();
            // A flag line with no trailing description still yields a flag
            // (`-cp <path>` in java, openssl's option lists).
            {
                let flags_part = parts[0];
                let desc = parts
                    .get(1)
                    .map(|d| d.trim().to_string())
                    .unwrap_or_default();

                let mut short: Option<String> = None;
                let mut longs: Vec<String> = Vec::new();
                let mut val_hint: Option<String> = None;

                for segment in flags_part.split(',') {
                    let seg = segment.trim();
                    if seg.is_empty() {
                        continue;
                    }

                    let (flag_tok, hint) = if let Some((f, h)) = seg.split_once('=') {
                        (f.trim(), Some(h.trim().to_string()))
                    } else if let Some((f, h)) = seg.split_once(' ') {
                        (f.trim(), Some(h.trim().to_string()))
                    } else {
                        (seg, None)
                    };

                    if hint.is_some() && val_hint.is_none() {
                        val_hint = hint;
                    }

                    if flag_tok.starts_with("--") {
                        longs.push(flag_tok.to_string());
                    } else if flag_tok.starts_with('-') && flag_tok.len() == 2 {
                        short = Some(flag_tok.to_string());
                    }
                }

                let takes_value = val_hint.is_some();

                if !longs.is_empty() {
                    for long in longs {
                        flags.push(CliFlag {
                            short: short.clone(),
                            long: Some(long),
                            takes_value,
                            value_hint: val_hint.clone(),
                            description: desc.clone(),
                        });
                    }
                    continue;
                } else if short.is_some() {
                    flags.push(CliFlag {
                        short,
                        long: None,
                        takes_value,
                        value_hint: val_hint,
                        description: desc,
                    });
                    continue;
                }
            }
        }

        // Parse subcommands
        if in_commands_section {
            if let Some(caps) = subcmd_re.captures(line) {
                let cmd_name = caps
                    .get(1)
                    .map(|m| m.as_str().trim_end_matches(':').to_string())
                    .unwrap();
                if !cmd_name.starts_with('-')
                    && cmd_name.len() >= 2
                    && !detected_subcommands.contains(&cmd_name)
                {
                    detected_subcommands.push(cmd_name);
                }
            }
        } else if !in_options_section
            && description.is_empty()
            && !trimmed.starts_with('-')
            && !lower.starts_with("usage")
        {
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
