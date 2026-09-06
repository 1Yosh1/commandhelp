// tests/storage_test.rs
use chelp::models::{CliCommandSchema, CliFlag};
use chelp::storage::SchemaStore;
use tempfile::NamedTempFile;

#[test]
fn test_save_and_retrieve_schema() {
    let tmp_file = NamedTempFile::new().unwrap();
    let store = SchemaStore::new(tmp_file.path()).unwrap();

    let flag = CliFlag {
        short: Some("-p".to_string()),
        long: Some("--port".to_string()),
        takes_value: true,
        value_hint: Some("PORT".to_string()),
        description: "Port to bind".to_string(),
    };

    let schema = CliCommandSchema {
        binary: "testcli".to_string(),
        subcommand_path: vec!["serve".to_string()],
        usage: "testcli serve --port <PORT>".to_string(),
        description: "Start the server".to_string(),
        flags: vec![flag],
        subcommands: vec![],
        binary_mtime: 12345,
        last_indexed: 67890,
    };

    store.save_schema(&schema).expect("save failed");

    let loaded = store.get_schema("testcli", &["serve".to_string()]).unwrap();
    assert!(loaded.is_some());
    let loaded = loaded.unwrap();
    assert_eq!(loaded.binary, "testcli");
    assert_eq!(loaded.flags[0].long.as_deref(), Some("--port"));

    let matches = store.match_flags("testcli", &["serve".to_string()], "--p").unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].long.as_deref(), Some("--port"));
}
