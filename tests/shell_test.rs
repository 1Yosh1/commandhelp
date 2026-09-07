// tests/shell_test.rs
use chelp::shell::generate_hook_script;

#[test]
fn test_shell_script_generation() {
    let pwsh = generate_hook_script("pwsh").unwrap();
    assert!(pwsh.contains("Set-PSReadLineKeyHandler"));
    assert!(pwsh.contains("chelp query"));

    let zsh = generate_hook_script("zsh").unwrap();
    assert!(zsh.contains("zle-line-update") || zsh.contains("chelp-query"));
    assert!(zsh.contains("chelp query"));

    let bash = generate_hook_script("bash").unwrap();
    assert!(bash.contains("bind -x"));
    assert!(bash.contains("chelp query"));
}
