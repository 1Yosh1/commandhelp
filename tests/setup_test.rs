use chelp::setup::install_hook_to_file;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_install_hook_to_file_creates_file_if_not_exists() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join(".bashrc");

    let hook_line = "eval \"$(\"chelp\" init bash)\"";
    let comment = "# CommandHelp Hook";

    install_hook_to_file(&file_path, hook_line, comment).unwrap();

    assert!(file_path.exists());
    let content = fs::read_to_string(&file_path).unwrap();
    assert!(content.contains(hook_line));
    assert!(content.contains(comment));
}

#[test]
fn test_install_hook_to_file_appends_to_existing() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join(".zshrc");

    fs::write(&file_path, "export PATH=/usr/local/bin:$PATH\n").unwrap();

    let hook_line = "eval \"$(\"chelp\" init zsh)\"";
    let comment = "# CommandHelp Hook";

    install_hook_to_file(&file_path, hook_line, comment).unwrap();

    let content = fs::read_to_string(&file_path).unwrap();
    assert!(content.starts_with("export PATH=/usr/local/bin:$PATH\n"));
    assert!(content.contains(hook_line));
    assert!(content.contains(comment));
}

#[test]
fn test_install_hook_to_file_idempotent() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join(".bashrc");

    let hook_line = "eval \"$(\"chelp\" init bash)\"";
    let comment = "# CommandHelp Hook";

    // Install first time
    install_hook_to_file(&file_path, hook_line, comment).unwrap();

    let content1 = fs::read_to_string(&file_path).unwrap();

    // Install second time
    install_hook_to_file(&file_path, hook_line, comment).unwrap();

    let content2 = fs::read_to_string(&file_path).unwrap();

    // Content should be exactly the same (no duplicates)
    assert_eq!(content1, content2);
}

#[test]
fn test_install_hook_to_file_creates_parent_dirs() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("deep").join("nested").join("profile");

    let hook_line = "hook";
    let comment = "# comment";

    install_hook_to_file(&file_path, hook_line, comment).unwrap();

    assert!(file_path.exists());
    let content = fs::read_to_string(&file_path).unwrap();
    assert!(content.contains(hook_line));
}
