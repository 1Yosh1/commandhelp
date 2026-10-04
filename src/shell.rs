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

pub fn generate_hook_script(shell: &str) -> Result<String, ChelpError> {
    match shell.to_lowercase().as_str() {
        "pwsh" | "powershell" => Ok(PSH_HOOK.trim_end().to_string()),
        "zsh" => Ok(ZSH_HOOK.trim_end().to_string()),
        "bash" => Ok(BASH_HOOK.trim_end().to_string()),
        "fish" => Ok(FISH_HOOK.trim_end().to_string()),
        other => Err(ChelpError::Config(format!("Unsupported shell: {}", other))),
    }
}
