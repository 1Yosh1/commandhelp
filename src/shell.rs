// src/shell.rs
//
// Every hook talks to the CLI with the same two conventions:
//
//   * `chelp query <buffer> --out-file <tmp>` — the interactive modal draws on
//     the terminal while the *result* goes to the file as `run\n<command>` or
//     `edit\n<command>` (empty when cancelled). Nothing of the TUI is ever
//     captured back into the command line.
//   * `chelp complete -- <buffer>` — stdout is a plain list of completed lines,
//     best first; the hook inserts the first one that extends the buffer.

use crate::error::ChelpError;

const PSH_HOOK: &str = include_str!("hooks/hook.ps1");
const ZSH_HOOK: &str = include_str!("hooks/hook.zsh");
const BASH_HOOK: &str = include_str!("hooks/hook.bash");
const FISH_HOOK: &str = include_str!("hooks/hook.fish");

/// Term/info-style key name (e.g. "ctrl-x", "ctrl-space") → per-shell binding
/// syntax. Unmapped values keep the stock binding. Set `CHELP_TRIGGER_KEY` in
/// your environment before `eval "$(chelp init zsh)"` to remap the query
/// trigger away from Ctrl+Space (tmux prefix and PSReadLine MenuComplete
/// conflicts, a top ask in the customer review).
fn substitute_trigger(shell: &str, hook: &str) -> String {
    let Ok(key) = std::env::var("CHELP_TRIGGER_KEY") else {
        return hook.to_string();
    };
    let key = key.trim().to_lowercase();
    if key.is_empty() {
        return hook.to_string();
    }
    if key == "ctrl-space" || key == "ctrl-@" {
        return hook.to_string(); // stock default, nothing to change
    }

    // Map "ctrl-x" style names into each shell's syntax.
    let letter = key
        .strip_prefix("ctrl-")
        .and_then(|rest| {
            let first = rest.chars().next()?;
            first.is_ascii_alphabetic().then(|| first.to_ascii_lowercase())
        });
    let Some(letter) = letter else {
        return hook.to_string(); // unsupported name: keep default binding
    };

    match shell {
        "zsh" => {
            // ^X binds control characters; support only letters.
            hook.replace("bindkey '^ '   chelp-query", &format!("bindkey '^{}'  chelp-query", letter))
        }
        "bash" => {
            // \C-x is readline's control-letter syntax.
            hook.replace("\"\\C-@\": _chelp_query", &format!("\"\\C-{}\": _chelp_query", letter))
        }
        "fish" => {
            hook.replace("bind \\C-space __chelp_query", &format!("bind \\C-{} __chelp_query", letter))
        }
        "powershell" | "pwsh" => {
            // PSReadLine chords like Ctrl+X.
            hook.replace("'Ctrl+Space'", &format!("'Ctrl+{}'", letter.to_ascii_uppercase()))
        }
        _ => hook.to_string(),
    }
    .replace("Ctrl+Space", "(custom trigger key)")
}

pub fn generate_hook_script(shell: &str) -> Result<String, ChelpError> {
    let normalized = shell.to_lowercase();
    let hook = match normalized.as_str() {
        "pwsh" | "powershell" => PSH_HOOK.trim_end().to_string(),
        "zsh" => ZSH_HOOK.trim_end().to_string(),
        "bash" => BASH_HOOK.trim_end().to_string(),
        "fish" => FISH_HOOK.trim_end().to_string(),
        other => return Err(ChelpError::Config(format!("Unsupported shell: {}", other))),
    };
    let shell_name = match normalized.as_str() {
        "pwsh" | "powershell" => "powershell",
        other => other,
    };
    Ok(substitute_trigger(shell_name, &hook))
}
