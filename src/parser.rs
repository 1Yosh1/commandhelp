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

    // Matches subcommands: "  create      Create a resource", "  auth:    Authenticate"
    let subcmd_re = Regex::new(
        r"^\s{2,}([a-zA-Z0-9_-]+):?\s{2,}(.*)$"
    ).unwrap();

    let mut in_commands_section = false;
    let mut in_options_section = false;

    for line in help_text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let lower = trimmed.to_lowercase();
        if lower.starts_with("usage:") || lower.starts_with("usage") {
            if usage.is_empty() {
                usage = trimmed.to_string();
            }
            continue;
        }

        // Detect section headers
        let is_all_caps = trimmed.len() >= 4 && trimmed.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_whitespace());
        if trimmed.ends_with(':') || is_all_caps {
            if lower.contains("command") {
                in_commands_section = true;
                in_options_section = false;
                continue;
            } else if lower.contains("option") || lower.contains("flag") || lower.contains("mode") || lower.contains("modifier") {
                in_options_section = true;
                in_commands_section = false;
                continue;
            }
        }

        // Parse flags if line starts with '-'
        if trimmed.starts_with('-') {
            let split_pattern = Regex::new(r"\s{2,}").unwrap();
            let parts: Vec<&str> = split_pattern.splitn(trimmed, 2).collect();
            if parts.len() >= 2 {
                let flags_part = parts[0];
                let desc = parts[1].trim().to_string();

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
                let cmd_name = caps.get(1).map(|m| m.as_str().trim_end_matches(':').to_string()).unwrap();
                if !cmd_name.starts_with('-') && cmd_name.len() >= 2 && !detected_subcommands.contains(&cmd_name) {
                    detected_subcommands.push(cmd_name);
                }
            }
        } else if !in_options_section && description.is_empty() && !trimmed.starts_with('-') && !lower.starts_with("usage") {
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