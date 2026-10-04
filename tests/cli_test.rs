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
        .stdout(predicate::str::contains("bindkey '^ '"))
        .stdout(predicate::str::contains("bindkey '^I'"));
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
    // A socket nobody owns: the client must stay quiet on stdout (the hook
    // treats that as "no suggestion") while still explaining itself on stderr.
    let mut cmd = Command::cargo_bin("chelp").unwrap();
    cmd.env("CHELP_NO_AUTO_SPAWN", "1")
        .env("CHELP_SOCKET", format!("chelp-absent-{}", std::process::id()))
        .args(["complete", "docker run "])
        .assert()
        .success()
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("daemon"));
}

#[test]
fn test_cli_init_fish() {
    let mut cmd = Command::cargo_bin("chelp").unwrap();
    cmd.args(["init", "fish"])
        .assert()
        .success()
        .stdout(predicate::str::contains("commandline"))
        .stdout(predicate::str::contains("chelp complete"))
        .stdout(predicate::str::contains("--out-file"));
}

#[test]
fn test_cli_complete_logs_diagnostics_into_the_state_dir() {
    // Spec §2.3: failures are explained somewhere. State (including the log)
    // lives in $CHELP_HOME, never in the developer's real home.
    let dir = tempfile::tempdir().unwrap();
    let mut cmd = Command::cargo_bin("chelp").unwrap();
    cmd.env("CHELP_HOME", dir.path())
        .env("CHELP_NO_AUTO_SPAWN", "1")
        .env("CHELP_SOCKET", format!("chelp-absent-2-{}", std::process::id()))
        .args(["complete", "git st"])
        .assert()
        .success();

    let log = dir.path().join("chelp.log");
    assert!(log.exists(), "expected a log file inside CHELP_HOME");
    let contents = std::fs::read_to_string(&log).unwrap();
    assert!(
        contents.contains("complete"),
        "log should record the failed completion, got: {}",
        contents
    );
}
