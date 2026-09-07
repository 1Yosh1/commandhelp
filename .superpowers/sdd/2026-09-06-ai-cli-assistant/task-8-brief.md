# Task 8: Shell Hook Generator & Unified CLI Dispatcher

**Plan File:** `docs/superpowers/plans/2026-09-06-ai-cli-assistant.md`  
**Spec File:** `docs/superpowers/specs/2026-09-06-ai-cli-assistant-design.md`

## Files
- Create: `src/shell.rs`
- Create: `src/main.rs`
- Test: `tests/shell_test.rs`

## Interfaces
- Consumes: `start_daemon`, `send_ipc_request`, `render_interactive_confirmation`, `AiProvider`
- Produces:
  - `generate_hook_script(shell: &str) -> Result<String, ChelpError>`
  - Complete CLI executable `chelp` with subcommands: `daemon`, `complete`, `query`, `init`, `config`

## Steps

### Step 1: Write the failing test

```rust
// tests/shell_test.rs
use chelp::shell::generate_hook_script;

#[test]
fn test_shell_script_generation() {
    let pwsh = generate_hook_script("pwsh").unwrap();
    assert!(pwsh.contains("Set-PSReadLineKeyHandler"));
    assert!(pwsh.contains("chelp complete"));

    let zsh = generate_hook_script("zsh").unwrap();
    assert!(zsh.contains("zle-line-update"));
    assert!(zsh.contains("chelp-query"));

    let bash = generate_hook_script("bash").unwrap();
    assert!(bash.contains("bind -x"));
}
```

### Step 2: Run test to verify it fails

Run: `cargo test --test shell_test`  
Expected: FAIL (module `chelp::shell` not found)

### Step 3: Write minimal implementation

```rust
// src/shell.rs
use crate::error::ChelpError;

pub fn generate_hook_script(shell: &str) -> Result<String, ChelpError> {
    match shell.to_lowercase().as_str() {
        "pwsh" | "powershell" => Ok(r#"
# CommandHelp PowerShell Hook
Set-PSReadLineKeyHandler -Chord 'Ctrl+Space' -ScriptBlock {
    $line = $null
    $cursor = $null
    [Microsoft.PowerShell.PSConsoleReadLine]::GetBufferState([ref]$line, [ref]$cursor)
    $result = chelp query "$line"
    if ($result) {
        [Microsoft.PowerShell.PSConsoleReadLine]::RevertLine()
        [Microsoft.PowerShell.PSConsoleReadLine]::Insert($result)
    }
}
"#.trim().to_string()),

        "zsh" => Ok(r#"
# CommandHelp Zsh Hook
chelp-query() {
    local cmd=$(chelp query "$BUFFER")
    if [[ -n "$cmd" ]]; then
        BUFFER="$cmd"
        CURSOR=${#BUFFER}
    fi
    zle redisplay
}
zle -N chelp-query
bindkey '^ ' chelp-query
"#.trim().to_string()),

        "bash" => Ok(r#"
# CommandHelp Bash Hook
_chelp_query() {
    local cmd=$(chelp query "$READLINE_LINE")
    if [[ -n "$cmd" ]]; then
        READLINE_LINE="$cmd"
        READLINE_POINT=${#READLINE_LINE}
    fi
}
bind -x '"\C-@": _chelp_query'
"#.trim().to_string()),

        _ => Err(ChelpError::Config(format!("Unsupported shell: {}", shell))),
    }
}
```

Add `pub mod shell;` to `src/lib.rs`.

### Step 4: Run test to verify it passes

Run: `cargo test --test shell_test`  
Expected: PASS

### Step 5: Commit

```bash
git add src/shell.rs src/main.rs tests/shell_test.rs src/lib.rs
git commit -m "feat: implement shell hook generator and unified CLI dispatcher"
```
