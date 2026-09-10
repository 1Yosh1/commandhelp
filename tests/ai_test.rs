// tests/ai_test.rs
use chelp::ai::build_prompt;
use chelp::models::{CliCommandSchema, ShellContext};

#[test]
fn test_build_prompt_includes_context() {
    let ctx = ShellContext {
        os: "windows".to_string(),
        shell: "pwsh".to_string(),
        cwd: "C:\\projects".to_string(),
    };

    let schema = CliCommandSchema {
        binary: "git".to_string(),
        subcommand_path: vec![],
        usage: "git [options] <command>".to_string(),
        description: "Fast version control system".to_string(),
        flags: vec![],
        subcommands: vec!["commit".to_string(), "push".to_string()],
        binary_mtime: 0,
        last_indexed: 0,
    };

    let prompt = build_prompt("commit with message wip", &ctx, &[schema]);
    assert!(prompt.contains("Target OS: windows"));
    assert!(prompt.contains("Target Shell: pwsh"));
    assert!(prompt.contains("Tool Context: git"));
}

#[test]
fn test_create_provider_from_config_pro() {
    let cfg = chelp::config::AiConfig {
        provider: "pro".to_string(),
        model: None,
        api_key: Some("test-pro-token-12345".to_string()),
        endpoint: Some("https://api.commandhelp.dev".to_string()),
    };
    let provider = chelp::ai::create_provider_from_config(&cfg);
    assert!(provider.is_ok());

    let missing_cfg = chelp::config::AiConfig {
        provider: "pro".to_string(),
        model: None,
        api_key: None,
        endpoint: None,
    };
    // If no CHELP_API_TOKEN env var and no stored credentials, it should return Err
    std::env::remove_var("CHELP_API_TOKEN");
    let missing_provider = chelp::ai::create_provider_from_config(&missing_cfg);
    // Either fails or falls back to stored credentials if present on disk
    if std::env::var("CHELP_API_TOKEN").is_err() && chelp::auth::load_credentials().ok().flatten().is_none() {
        assert!(missing_provider.is_err());
    }
}

#[tokio::test]
async fn test_pro_provider_resolve_intent() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    let server_task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = vec![0u8; 4096];
        let n = socket.read(&mut buf).await.unwrap();
        let request = String::from_utf8_lossy(&buf[..n]);

        assert!(request.to_lowercase().contains("authorization: bearer mock-secret-token"));
        assert!(request.contains("/api/query"));

        let response_body = serde_json::json!({
            "command": "docker ps -a",
            "explanation": "Lists all containers including stopped ones",
            "safety_level": "safe",
            "destructive_warning": null
        }).to_string();

        let http_response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            response_body.len(),
            response_body
        );

        socket.write_all(http_response.as_bytes()).await.unwrap();
        socket.flush().await.unwrap();
    });

    let provider = chelp::ai::ProProvider::new(
        "mock-secret-token".to_string(),
        Some(format!("http://127.0.0.1:{}", port)),
    );

    let ctx = ShellContext {
        os: "linux".to_string(),
        shell: "bash".to_string(),
        cwd: "/home/user".to_string(),
    };

    use chelp::ai::AiProvider;
    let res = provider.resolve_intent("list docker containers", &ctx, &[]).await;
    assert!(res.is_ok(), "Expected Ok response, got: {:?}", res.err());

    let ai_resp = res.unwrap();
    assert_eq!(ai_resp.command, "docker ps -a");
    assert_eq!(ai_resp.safety_level, chelp::models::SafetyLevel::Safe);
    assert_eq!(ai_resp.destructive_warning, None);

    server_task.await.unwrap();
}

