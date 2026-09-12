// src/shell.rs
use crate::error::ChelpError;

pub fn generate_hook_script(shell: &str) -> Result<String, ChelpError> {
    match shell.to_lowercase().as_str() {
        "pwsh" | "powershell" => Ok(r#"
# CommandHelp PowerShell Hook
Set-PSReadLineKeyHandler -Chord 'Ctrl+ ' -ScriptBlock {
    $line = $null
    $cursor = $null
    [Microsoft.PowerShell.PSConsoleReadLine]::GetBufferState([ref]$line, [ref]$cursor)
    $tmp = New-TemporaryFile
    chelp query "$line" --out-file $tmp.FullName
    $result = Get-Content -Path $tmp.FullName -Raw
    Remove-Item -Path $tmp.FullName -ErrorAction SilentlyContinue
    if ($result) {
        $result = $result.Trim()
        if ($result) {
            [Microsoft.PowerShell.PSConsoleReadLine]::RevertLine()
            [Microsoft.PowerShell.PSConsoleReadLine]::Insert($result)
        }
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
