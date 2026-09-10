// src/auth.rs
use crate::config::get_config_dir;
use crate::error::ChelpError;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct UserCredentials {
    pub token: String,
    pub email: Option<String>,
    pub plan: Option<String>,
}

pub fn get_credentials_path() -> PathBuf {
    get_config_dir().join("credentials.json")
}

pub fn load_credentials() -> Result<Option<UserCredentials>, ChelpError> {
    let path = get_credentials_path();
    if path.exists() {
        let content = fs::read_to_string(&path)?;
        let creds: UserCredentials = serde_json::from_str(&content)?;
        Ok(Some(creds))
    } else {
        Ok(None)
    }
}

pub fn save_credentials(creds: &UserCredentials) -> Result<PathBuf, ChelpError> {
    let path = get_credentials_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let json = serde_json::to_string_pretty(creds)?;
    fs::write(&path, json)?;
    Ok(path)
}

pub fn open_browser(url: &str) {
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", url])
            .spawn();
    }

    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open")
            .arg(url)
            .spawn();
    }

    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open")
            .arg(url)
            .spawn();
    }
}

/// Run ephemeral localhost OAuth listener to receive the callback from browser login
pub async fn run_cli_login(
    port: u16,
    timeout_secs: u64,
    auth_url_base: Option<&str>,
) -> Result<UserCredentials, ChelpError> {
    let base_url = auth_url_base.unwrap_or("https://commandhelp.dev");
    let session_id = format!("{:x}{:x}", std::process::id(), std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis());

    let listener = TcpListener::bind(("127.0.0.1", port))
        .await
        .map_err(|e| ChelpError::Ipc(format!("Failed to bind port {}: {}", port, e)))?;

    let auth_url = format!("{}/auth/cli?session_id={}&port={}", base_url, session_id, port);
    println!("Opening your browser to authenticate:");
    println!("  {}", auth_url);
    println!("\nWaiting for authentication callback on http://127.0.0.1:{} ... (Timeout in {}s)", port, timeout_secs);

    open_browser(&auth_url);

    let accept_future = async {
        let (mut stream, _) = listener.accept().await?;
        let mut buffer = [0u8; 4096];
        let bytes_read = stream.read(&mut buffer).await?;
        let request = String::from_utf8_lossy(&buffer[..bytes_read]);

        // Parse query params from GET /callback?token=... HTTP/1.1
        let first_line = request.lines().next().unwrap_or_default();
        let path = first_line.split_whitespace().nth(1).unwrap_or_default();

        let mut token = None;
        let mut email = None;
        let mut plan = None;

        if let Some(query) = path.split_once('?') {
            for pair in query.1.split('&') {
                if let Some((k, v)) = pair.split_once('=') {
                    match k {
                        "token" => token = Some(v.to_string()),
                        "email" => email = Some(v.replace("%40", "@")),
                        "plan" => plan = Some(v.to_string()),
                        _ => {}
                    }
                }
            }
        }

        let html_response = if token.is_some() {
            r#"HTTP/1.1 200 OK
Content-Type: text/html; charset=utf-8
Connection: close

<!DOCTYPE html>
<html>
<head><title>CommandHelp Authentication</title></head>
<body style="font-family: sans-serif; text-align: center; padding: 60px; background: #0f172a; color: #f8fafc;">
  <h1 style="color: #22c55e;">✔ Authenticated Successfully!</h1>
  <p style="font-size: 1.1rem; color: #94a3b8;">You can now close this tab and return to your terminal.</p>
</body>
</html>"#
        } else {
            r#"HTTP/1.1 400 Bad Request
Content-Type: text/html; charset=utf-8
Connection: close

<!DOCTYPE html>
<html>
<head><title>Authentication Failed</title></head>
<body style="font-family: sans-serif; text-align: center; padding: 60px;">
  <h1 style="color: #ef4444;">✖ Authentication Failed</h1>
  <p>Missing authentication token in callback.</p>
</body>
</html>"#
        };

        let _ = stream.write_all(html_response.as_bytes()).await;
        let _ = stream.flush().await;

        token.map(|t| UserCredentials {
            token: t,
            email,
            plan: plan.or_else(|| Some("Pro".to_string())),
        }).ok_or_else(|| ChelpError::Config("No token received in callback".to_string()))
    };

    match tokio::time::timeout(Duration::from_secs(timeout_secs), accept_future).await {
        Ok(res) => {
            let creds = res.map_err(|e| ChelpError::Config(format!("Login failed: {}", e)))?;
            save_credentials(&creds)?;
            println!("✔ Successfully authenticated as {} ({})",
                creds.email.as_deref().unwrap_or("Pro User"),
                creds.plan.as_deref().unwrap_or("Pro Plan")
            );
            Ok(creds)
        }
        Err(_) => Err(ChelpError::Config("Authentication timed out. Please try again.".to_string())),
    }
}
