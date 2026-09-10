// tests/parser_test.rs
use chelp::crawler::crawl_command_help;
use chelp::parser::parse_help_output;

#[test]
fn test_parse_gnu_and_cobra_flags() {
    let help_text = r#"
Usage: docker run [OPTIONS] IMAGE [COMMAND] [ARG...]

Run a command in a new container

Options:
  -d, --detach                         Run container in background and print container ID
  -e, --env list                       Set environment variables
  -p, --publish list                   Publish a container's port(s) to the host
      --rm                             Automatically remove the container when it exits
  -v, --volume list                    Bind mount a volume
      --name string                    Assign a name to the container

Commands:
  exec        Execute a command in a running container
  logs        Fetch the logs of a container
"#;

    let schema = parse_help_output("docker", &["run".to_string()], help_text).expect("parser failed");
    assert_eq!(schema.binary, "docker");
    assert_eq!(schema.subcommand_path, vec!["run".to_string()]);
    assert!(schema.flags.iter().any(|f| f.long.as_deref() == Some("--detach") && f.short.as_deref() == Some("-d")));
    assert!(schema.flags.iter().any(|f| f.long.as_deref() == Some("--rm") && !f.takes_value));
    assert!(schema.flags.iter().any(|f| f.long.as_deref() == Some("--name") && f.takes_value));
    assert!(schema.subcommands.contains(&"exec".to_string()));
    assert!(schema.subcommands.contains(&"logs".to_string()));
}

#[test]
fn test_crawl_nonexistent_binary() {
    let result = crawl_command_help("nonexistent_binary_xyz_12345", &[]);
    assert!(result.is_err());
}

#[test]
fn test_parse_kubectl_fixture() {
    let help_text = include_str!("fixtures/kubectl_help.txt");
    let schema = parse_help_output("kubectl", &[], help_text).expect("kubectl parsing failed");
    assert_eq!(schema.binary, "kubectl");
    // Verify subcommands from diverse sections
    assert!(schema.subcommands.contains(&"create".to_string()));
    assert!(schema.subcommands.contains(&"get".to_string()));
    assert!(schema.subcommands.contains(&"delete".to_string()));
    assert!(schema.subcommands.contains(&"rollout".to_string()));
    assert!(schema.subcommands.contains(&"logs".to_string()));
    assert!(schema.subcommands.contains(&"exec".to_string()));
    // Verify flags
    assert!(schema.flags.iter().any(|f| f.long.as_deref() == Some("--namespace") && f.short.as_deref() == Some("-n")));
    assert!(schema.flags.iter().any(|f| f.long.as_deref() == Some("--kubeconfig")));
    assert!(schema.flags.iter().any(|f| f.long.as_deref() == Some("--insecure-skip-tls-verify") && !f.takes_value));
}

#[test]
fn test_parse_gh_fixture() {
    let help_text = include_str!("fixtures/gh_help.txt");
    let schema = parse_help_output("gh", &[], help_text).expect("gh parsing failed");
    assert_eq!(schema.binary, "gh");
    assert!(schema.subcommands.contains(&"auth".to_string()));
    assert!(schema.subcommands.contains(&"pr".to_string()));
    assert!(schema.subcommands.contains(&"repo".to_string()));
    assert!(schema.subcommands.contains(&"workflow".to_string()));
    assert!(schema.flags.iter().any(|f| f.long.as_deref() == Some("--help") && f.short.as_deref() == Some("-h")));
    assert!(schema.flags.iter().any(|f| f.long.as_deref() == Some("--version")));
}

#[test]
fn test_parse_tar_fixture() {
    let help_text = include_str!("fixtures/tar_help.txt");
    let schema = parse_help_output("tar", &[], help_text).expect("tar parsing failed");
    assert_eq!(schema.binary, "tar");
    // Verify operation modes and modifiers
    assert!(schema.flags.iter().any(|f| f.long.as_deref() == Some("--create") && f.short.as_deref() == Some("-c")));
    assert!(schema.flags.iter().any(|f| f.long.as_deref() == Some("--extract") && f.short.as_deref() == Some("-x")));
    assert!(schema.flags.iter().any(|f| f.long.as_deref() == Some("--verbose") && f.short.as_deref() == Some("-v")));
    assert!(schema.flags.iter().any(|f| f.long.as_deref() == Some("--file") && f.takes_value));
    // Verify short-only flag
    assert!(schema.flags.iter().any(|f| f.short.as_deref() == Some("-h") && f.long.is_none()));
}