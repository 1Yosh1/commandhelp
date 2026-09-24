// tests/config_test.rs
use chelp::ai::clean_json_response;
use chelp::config::{AiConfig, ChelpConfig};

#[test]
fn test_config_serialization() {
    let config = ChelpConfig {
        ai: AiConfig {
            provider: "openai".to_string(),
            model: Some("gpt-4o-mini".to_string()),
            api_key: Some("sk-test12345".to_string()),
            endpoint: Some("https://api.openai.com/v1".to_string()),
        },
    };

    let toml_str = toml::to_string(&config).unwrap();
    assert!(toml_str.contains("provider = \"openai\""));
    assert!(toml_str.contains("model = \"gpt-4o-mini\""));

    let deserialized: ChelpConfig = toml::from_str(&toml_str).unwrap();
    assert_eq!(deserialized, config);
}

#[test]
fn test_clean_json_response() {
    let raw_fenced = "```json\n{\n  \"command\": \"ls -la\"\n}\n```";
    let cleaned = clean_json_response(raw_fenced);
    assert_eq!(cleaned, "{\n  \"command\": \"ls -la\"\n}");

    let raw_plain = "{\n  \"command\": \"ls -la\"\n}";
    let cleaned_plain = clean_json_response(raw_plain);
    assert_eq!(cleaned_plain, raw_plain);
}

#[test]
fn test_save_config_secure_permissions() {
    use chelp::config::{save_config, ChelpConfig};
    use std::fs;
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.toml");

    use std::sync::Mutex;
    static TEST_MUTEX: Mutex<()> = Mutex::new(());
    let _guard = TEST_MUTEX.lock().unwrap();

    std::env::set_var("CHELP_CONFIG_PATH_OVERRIDE", config_path.to_str().unwrap());

    let config = ChelpConfig::default();
    save_config(&config).unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = fs::metadata(&config_path).unwrap();
        let permissions = metadata.permissions();
        let mode = permissions.mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    std::env::remove_var("CHELP_CONFIG_PATH_OVERRIDE");
}
