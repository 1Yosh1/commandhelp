// tests/models_test.rs
use chelp::models::{CliCommandSchema, CliFlag, SafetyLevel, AiCommandResponse};

#[test]
fn test_schema_serialization() {
    let flag = CliFlag {
        short: Some("-v".to_string()),
        long: Some("--verbose".to_string()),
        takes_value: false,
        value_hint: None,
        description: "Show verbose output".to_string(),
    };

    let schema = CliCommandSchema {
        binary: "docker".to_string(),
        subcommand_path: vec!["container".to_string(), "ls".to_string()],
        usage: "docker container ls [OPTIONS]".to_string(),
        description: "List containers".to_string(),
        flags: vec![flag],
        subcommands: vec![],
        binary_mtime: 1700000000,
        last_indexed: 1700000000,
    };

    let json = serde_json::to_string(&schema).expect("serialization failed");
    let deserialized: CliCommandSchema = serde_json::from_str(&json).expect("deserialization failed");
    assert_eq!(deserialized.binary, "docker");
    assert_eq!(deserialized.flags.len(), 1);
    assert_eq!(deserialized.flags[0].long.as_deref(), Some("--verbose"));
}

#[test]
fn test_ai_response_safety_levels() {
    let resp = AiCommandResponse {
        command: "rm -rf /".to_string(),
        explanation: "Removes everything".to_string(),
        safety_level: SafetyLevel::Destructive,
        destructive_warning: Some("Deletes all files".to_string()),
    };

    let json = serde_json::to_string(&resp).unwrap();
    assert!(json.contains("destructive"));
}
