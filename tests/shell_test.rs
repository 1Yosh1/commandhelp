// tests/shell_test.rs
use chelp::shell::generate_hook_script;
use std::io::Write;
use std::process::{Command, Stdio};

/// Every hook must speak the same contract as `chelp query` / `chelp complete`.
fn assert_common_contract(shell: &str, script: &str) {
    assert!(
        script.contains("chelp query"),
        "{}: missing query hook",
        shell
    );
    assert!(
        script.contains("--out-file"),
        "{}: must capture the result in a file, never via stdout",
        shell
    );
    assert!(
        script.contains("chelp daemon --detached"),
        "{}: must warm the daemon at shell startup",
        shell
    );
    assert!(
        script.contains("chelp complete"),
        "{}: must ask for completions",
        shell
    );
}

/// Syntax-check the generated hook with the shell itself when it is installed.
fn assert_shell_syntax(shell: &str, script: &str) {
    let mut child = match Command::new(shell)
        .arg("-n")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => return, // shell not installed on this runner
    };
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(script.as_bytes())
        .expect("write hook");
    drop(child.stdin.take());
    let output = child.wait_with_output().expect("run shell syntax check");
    assert!(
        output.status.success(),
        "{} hook failed `{} -n`:\n{}\n--- hook ---\n{}",
        shell,
        shell,
        String::from_utf8_lossy(&output.stderr),
        script
    );
}

#[test]
fn test_pwsh_hook_contract() {
    let pwsh = generate_hook_script("pwsh").unwrap();
    assert!(pwsh.contains("Set-PSReadLineKeyHandler"));
    assert!(pwsh.contains("Ctrl+ "));
    assert!(
        pwsh.contains("AcceptLine"),
        "Enter must execute the command in the shell, not only insert it"
    );
    assert_common_contract("pwsh", &pwsh);
}

#[test]
fn test_zsh_hook_contract_and_syntax() {
    let zsh = generate_hook_script("zsh").unwrap();
    assert!(zsh.contains("zle -N chelp-query"));
    assert!(zsh.contains("bindkey '^ '"));
    assert!(
        zsh.contains("bindkey '^I'"),
        "Tab must accept the suggestion"
    );
    assert!(
        zsh.contains("POSTDISPLAY"),
        "zsh renders ghost text via POSTDISPLAY"
    );
    assert!(zsh.contains("accept-line"), "Enter must run the command");
    assert_common_contract("zsh", &zsh);
    assert_shell_syntax("zsh", &zsh);
}

#[test]
fn test_bash_hook_contract_and_syntax() {
    let bash = generate_hook_script("bash").unwrap();
    assert!(bash.contains("bind -x"));
    assert!(bash.contains("READLINE_LINE"));
    assert!(
        bash.contains("history -s"),
        "run must land in shell history"
    );
    assert_common_contract("bash", &bash);
    assert_shell_syntax("bash", &bash);
}

#[test]
fn test_fish_hook_contract() {
    let fish = generate_hook_script("fish").unwrap();
    assert!(fish.contains("commandline"));
    assert!(
        fish.contains("commandline -f execute"),
        "run must execute in fish"
    );
    assert!(fish.contains("bind "));
    assert_common_contract("fish", &fish);
    assert_shell_syntax("fish", &fish);
}

#[test]
fn test_unsupported_shell_is_rejected() {
    let err = generate_hook_script("csh").unwrap_err();
    assert!(
        err.to_string().contains("Unsupported shell: csh"),
        "{}",
        err
    );
}
