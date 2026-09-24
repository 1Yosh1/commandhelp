use chelp::crawler::crawl_command_help;
use std::fs;

#[test]
#[cfg(unix)]
fn test_crawl_command_help_success() {
    let script = r#"#!/bin/sh
if [ "$1" = "--help" ]; then
    echo "mock help output"
fi
"#;
    let dir = tempfile::tempdir().unwrap();
    let script_path = dir.path().join("mock_cmd");
    fs::write(&script_path, script).unwrap();

    let mut perms = fs::metadata(&script_path).unwrap().permissions();
    use std::os::unix::fs::PermissionsExt;
    perms.set_mode(0o755);
    fs::set_permissions(&script_path, perms).unwrap();

    // Use canonicalize to get the absolute path
    let script_path_abs = script_path.canonicalize().unwrap();
    let result = crawl_command_help(script_path_abs.to_str().unwrap(), &[]);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().trim(), "mock help output");
}

#[test]
#[cfg(unix)]
fn test_crawl_command_help_with_subcommand() {
    let script = r#"#!/bin/sh
if [ "$1" = "subcmd" ] && [ "$2" = "--help" ]; then
    echo "mock subcommand help output"
fi
"#;
    let dir = tempfile::tempdir().unwrap();
    let script_path = dir.path().join("mock_subcmd");
    fs::write(&script_path, script).unwrap();

    let mut perms = fs::metadata(&script_path).unwrap().permissions();
    use std::os::unix::fs::PermissionsExt;
    perms.set_mode(0o755);
    fs::set_permissions(&script_path, perms).unwrap();

    let script_path_abs = script_path.canonicalize().unwrap();
    let result = crawl_command_help(script_path_abs.to_str().unwrap(), &["subcmd".to_string()]);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().trim(), "mock subcommand help output");
}

#[test]
#[cfg(unix)]
fn test_crawl_command_help_stderr() {
    let script = r#"#!/bin/sh
if [ "$1" = "--help" ]; then
    >&2 echo "mock help stderr output"
fi
"#;
    let dir = tempfile::tempdir().unwrap();
    let script_path = dir.path().join("mock_cmd_stderr");
    fs::write(&script_path, script).unwrap();

    let mut perms = fs::metadata(&script_path).unwrap().permissions();
    use std::os::unix::fs::PermissionsExt;
    perms.set_mode(0o755);
    fs::set_permissions(&script_path, perms).unwrap();

    let script_path_abs = script_path.canonicalize().unwrap();
    let result = crawl_command_help(script_path_abs.to_str().unwrap(), &[]);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().trim(), "mock help stderr output");
}

#[test]
#[cfg(unix)]
fn test_crawl_command_help_timeout() {
    let script = r#"#!/bin/sh
sleep 2
"#;
    let dir = tempfile::tempdir().unwrap();
    let script_path = dir.path().join("mock_cmd_timeout");
    fs::write(&script_path, script).unwrap();

    let mut perms = fs::metadata(&script_path).unwrap().permissions();
    use std::os::unix::fs::PermissionsExt;
    perms.set_mode(0o755);
    fs::set_permissions(&script_path, perms).unwrap();

    let script_path_abs = script_path.canonicalize().unwrap();
    let result = crawl_command_help(script_path_abs.to_str().unwrap(), &[]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("Timed out reading help from"));
}

#[test]
fn test_crawl_command_help_nonexistent() {
    let result = crawl_command_help("nonexistent_binary_xyz_12345", &[]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("Failed to execute 'nonexistent_binary_xyz_12345'"));
}
