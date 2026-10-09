// src/safety.rs
use crate::models::{AiCommandResponse, SafetyLevel};
use regex::Regex;
use std::sync::OnceLock;

/// Spec §5.4 local heuristic validator. Patterns are compiled once: this runs
/// on every model response (and on the hot path of recipe/history rendering).
fn destructive_patterns() -> &'static [(Regex, &'static str)] {
    static PATTERNS: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        [
            (
                r"(?i)\brm\s+-[a-zA-Z]*r[a-zA-Z]*\b",
                "Recursive file removal (rm -r) permanently deletes data.",
            ),
            (
                r"(?i)\bRemove-Item\b.*-Recurse",
                "PowerShell recursive item removal permanently deletes data.",
            ),
            (r"(?i)\bdel\b.*(/s|/q)", "Recursive/silent file deletion."),
            (
                r"(?i)\b(mkfs|format)\b",
                "Disk formatting completely wipes storage partitions.",
            ),
            (
                r"(?i)\bdd\b.*of=/dev/",
                "Raw disk write can overwrite boot sectors or partitions.",
            ),
            (
                r"(?i)\b(DROP\s+DATABASE|DROP\s+TABLE|TRUNCATE)\b",
                "Destructive SQL operation deletes database tables.",
            ),
            (
                r"(?i)\bgit\s+push\b.*(--force|-f)\b",
                "Git force-push overwrites remote repository history.",
            ),
            (
                r"(?i)\bgit\s+reset\s+--hard\b",
                "Hard git reset discards all uncommitted local changes.",
            ),
            (
                r"(?i)\bkillall\b",
                "killall terminates every matching process without confirmation.",
            ),
            (
                r"(?i)\bkill\s+-9\b",
                "SIGKILL forces processes to terminate without saving state.",
            ),
            (
                r"(?i)\bStop-Process\b.*-Force",
                "Forces process termination without cleanup.",
            ),
            (
                r"(?i)\bterraform\s+.*destroy\b",
                "Terraform destroy tears down cloud infrastructure.",
            ),
            (
                r"(?i)\bkubectl\s+delete\s+(ns|namespace|all|pv|pvc)\b",
                "Kubernetes resource deletion destroys cluster infrastructure.",
            ),
            (
                r"(?i)\bdocker\s+system\s+prune\b.*(-a|--all)",
                "Docker prune removes all unused containers, networks, and images.",
            ),
            (
                r"(?i)\bconda\s+env\s+remove\b",
                "Conda environment removal permanently deletes your environment.",
            ),
            (
                r"(?i)\bhelm\s+uninstall\b",
                "Helm uninstall removes release and deletes cluster components.",
            ),
            (
                r"(?i)\baws\s+s3\s+rb\b.*--force",
                "Force-deleting S3 bucket destroys all objects inside.",
            ),
            (
                r"(?i)\baz\s+group\s+delete\b",
                "Azure resource group deletion destroys all contained cloud resources.",
            ),
        ]
        .iter()
        .map(|(pattern, warning)| (Regex::new(pattern).unwrap(), *warning))
        .collect()
    })
}

fn caution_patterns() -> &'static [Regex] {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        [
            r"(?i)\bgit\s+push\b",
            r"(?i)\bdocker\s+(stop|rm|kill)\b",
            r"(?i)\b(npm|cargo|pip)\s+(install|uninstall)\b",
            r"(?i)\bchmod\b",
        ]
        .iter()
        .map(|pattern| Regex::new(pattern).unwrap())
        .collect()
    })
}

pub fn evaluate_command_safety(command: &str) -> (SafetyLevel, Option<String>) {
    for (pattern, warning) in destructive_patterns() {
        if pattern.is_match(command) {
            return (SafetyLevel::Destructive, Some((*warning).to_string()));
        }
    }
    for pattern in caution_patterns() {
        if pattern.is_match(command) {
            return (SafetyLevel::Caution, None);
        }
    }
    (SafetyLevel::Safe, None)
}

/// Model self-rating is advisory: a locally detected destructive command always
/// wins (spec §5.4 layer 2), and a safe downgrade is never possible.
pub fn sanitize_and_verify(resp: &mut AiCommandResponse) {
    let (computed_level, warning) = evaluate_command_safety(&resp.command);
    if computed_level == SafetyLevel::Destructive {
        resp.safety_level = SafetyLevel::Destructive;
        if resp.destructive_warning.is_none() {
            resp.destructive_warning = warning;
        }
    }
}
