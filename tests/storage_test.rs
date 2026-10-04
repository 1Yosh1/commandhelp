// tests/storage_test.rs
use chelp::models::{CliCommandSchema, CliFlag};
use chelp::storage::SchemaStore;
use tempfile::NamedTempFile;

fn flag(short: &str, long: &str) -> CliFlag {
    CliFlag {
        short: Some(short.to_string()),
        long: Some(long.to_string()),
        takes_value: true,
        value_hint: Some("PORT".to_string()),
        description: "Port to bind".to_string(),
    }
}

#[test]
fn test_save_and_retrieve_schema() {
    let tmp_file = NamedTempFile::new().unwrap();
    let store = SchemaStore::new(tmp_file.path()).unwrap();

    let schema = CliCommandSchema {
        binary: "testcli".to_string(),
        subcommand_path: vec!["serve".to_string()],
        usage: "testcli serve --port <PORT>".to_string(),
        description: "Start the server".to_string(),
        flags: vec![flag("-p", "--port")],
        subcommands: vec![],
        binary_mtime: 12345,
        last_indexed: 67890,
    };
    store.save_schema(&schema).expect("save failed");

    let loaded = store.get_schema("testcli", &["serve".to_string()]).unwrap();
    let loaded = loaded.expect("schema should round-trip");
    assert_eq!(loaded.binary, "testcli");
    assert_eq!(loaded.flags[0].long.as_deref(), Some("--port"));
    assert_eq!(loaded.binary_mtime, 12345);
}

/// A partially indexed tree still answers: `container ls` falls back to
/// `container`, which falls back to the binary root.
#[test]
fn test_best_schema_walks_up_the_subcommand_path() {
    let tmp_file = NamedTempFile::new().unwrap();
    let store = SchemaStore::new(tmp_file.path()).unwrap();

    let root = CliCommandSchema {
        binary: "docker".to_string(),
        subcommand_path: vec![],
        usage: "docker <command>".to_string(),
        description: "Docker".to_string(),
        flags: vec![flag("-H", "--host")],
        subcommands: vec!["container".to_string()],
        binary_mtime: 0,
        last_indexed: 1,
    };
    let nested = CliCommandSchema {
        binary: "docker".to_string(),
        subcommand_path: vec!["container".to_string()],
        usage: "docker container".to_string(),
        description: "Manage containers".to_string(),
        flags: vec![flag("-a", "--all")],
        subcommands: vec!["ls".to_string()],
        binary_mtime: 0,
        last_indexed: 1,
    };
    store.save_schema(&root).unwrap();
    store.save_schema(&nested).unwrap();

    let exact = store
        .get_best_schema("docker", &["container".to_string()])
        .unwrap()
        .expect("exact match");
    assert_eq!(exact.description, "Manage containers");

    let fallback = store
        .get_best_schema("docker", &["container".to_string(), "ls".to_string()])
        .unwrap()
        .expect("prefix fallback");
    assert_eq!(fallback.description, "Manage containers");

    let root_fallback = store
        .get_best_schema("docker", &["volume".to_string()])
        .unwrap()
        .expect("root fallback");
    assert_eq!(root_fallback.description, "Docker");

    let unknown = store.get_best_schema("kubectl", &[]).unwrap();
    assert!(unknown.is_none());
}
