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
