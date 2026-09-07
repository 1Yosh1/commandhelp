# Task 5: Safety Guardrails & Deterministic Risk Analyzer

**Plan File:** `docs/superpowers/plans/2026-09-06-ai-cli-assistant.md`  
**Spec File:** `docs/superpowers/specs/2026-09-06-ai-cli-assistant-design.md`

## Files
- Create: `src/safety.rs`
- Test: `tests/safety_test.rs`

## Interfaces
- Consumes: `SafetyLevel`, `AiCommandResponse` from `models.rs`
- Produces:
  - `evaluate_command_safety(command: &str) -> (SafetyLevel, Option<String>)`
  - `sanitize_and_verify(resp: &mut AiCommandResponse)`

## Steps

### Step 1: Write the failing test

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

### Step 2: Run test to verify it fails

Run: `cargo test --test safety_test`  
Expected: FAIL (module `chelp::safety` not found)

### Step 3: Write minimal implementation

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

### Step 4: Run test to verify it passes

Run: `cargo test --test safety_test`  
Expected: PASS

### Step 5: Commit

```bash
git add src/safety.rs tests/safety_test.rs src/lib.rs
git commit -m "feat: implement deterministic safety guardrails"
```
