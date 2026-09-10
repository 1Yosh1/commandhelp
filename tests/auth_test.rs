use chelp::auth::{run_cli_login, UserCredentials};
use tempfile::tempdir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[test]
fn test_credentials_save_and_load() {
    let tmp = tempdir().unwrap();
    let creds_file = tmp.path().join("credentials.json");

    let creds = UserCredentials {
        token: "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9".to_string(),
        email: Some("dev@example.com".to_string()),
        plan: Some("Pro".to_string()),
    };

    let json = serde_json::to_string_pretty(&creds).unwrap();
    std::fs::write(&creds_file, json).unwrap();

    let loaded: UserCredentials = serde_json::from_str(&std::fs::read_to_string(&creds_file).unwrap()).unwrap();
    assert_eq!(loaded.token, creds.token);
    assert_eq!(loaded.email.as_deref(), Some("dev@example.com"));
    assert_eq!(loaded.plan.as_deref(), Some("Pro"));
}

#[tokio::test]
async fn test_cli_login_loopback() {
    let port = 19432;

    // Spawn the login listener in background task
    let login_task = tokio::spawn(async move {
        run_cli_login(port, 5, Some("http://localhost")).await
    });

    // Wait 50ms for listener to bind
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Simulate the browser callback hitting http://127.0.0.1:port/callback?...
    let mut stream = TcpStream::connect(("127.0.0.1", port)).await.expect("Failed to connect to loopback");
    let request = "GET /callback?token=secret_jwt_xyz987&email=developer%40corp.com&plan=Team HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n";
    stream.write_all(request.as_bytes()).await.unwrap();

    let mut response_buf = vec![0u8; 1024];
    let n = stream.read(&mut response_buf).await.unwrap();
    let response = String::from_utf8_lossy(&response_buf[..n]);

    assert!(response.contains("200 OK"));
    assert!(response.contains("Authenticated Successfully"));

    let creds = login_task.await.unwrap().expect("Login listener returned error");
    assert_eq!(creds.token, "secret_jwt_xyz987");
    assert_eq!(creds.email.as_deref(), Some("developer@corp.com"));
    assert_eq!(creds.plan.as_deref(), Some("Team"));
}
