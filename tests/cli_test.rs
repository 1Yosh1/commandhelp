use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_cli_help_flag() {
    let mut cmd = Command::cargo_bin("chelp").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Universal AI-Powered CLI Assistant"))
        .stdout(predicate::str::contains("Usage: chelp"))
        .stdout(predicate::str::contains("setup"))
        .stdout(predicate::str::contains("config"))
        .stdout(predicate::str::contains("init"));
}

#[test]
fn test_cli_version_flag() {
    let mut cmd = Command::cargo_bin("chelp").unwrap();
    cmd.arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("chelp 0.1.2"));
}

#[test]
fn test_cli_init_pwsh() {
    let mut cmd = Command::cargo_bin("chelp").unwrap();
    cmd.args(["init", "pwsh"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Set-PSReadLineKeyHandler"))
        .stdout(predicate::str::contains("chelp query"));
}

#[test]
fn test_cli_init_bash() {
    let mut cmd = Command::cargo_bin("chelp").unwrap();
    cmd.args(["init", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("bind -x"))
        .stdout(predicate::str::contains("chelp query"));
}

#[test]
fn test_cli_init_zsh() {
    let mut cmd = Command::cargo_bin("chelp").unwrap();
    cmd.args(["init", "zsh"])
        .assert()
        .success()
        .stdout(predicate::str::contains("zle -N chelp-query"))
        .stdout(predicate::str::contains("bindkey '^ ' chelp-query"));
}

#[test]
fn test_cli_init_unsupported_shell() {
    let mut cmd = Command::cargo_bin("chelp").unwrap();
    cmd.args(["init", "csh"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Unsupported shell: csh"));
}

#[test]
fn test_cli_complete_graceful_without_daemon() {
    let mut cmd = Command::cargo_bin("chelp").unwrap();
    cmd.env("CHELP_NO_AUTO_SPAWN", "1")
        .args(["complete", "docker run "])
        .assert()
        .success();
}
