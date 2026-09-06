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