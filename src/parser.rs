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
        } else if !in_options_section && description.is_empty() && !trimmed.starts_with('-') && !trimmed.to_lowercase().starts_with("usage") {
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