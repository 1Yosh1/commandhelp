// src/ipc.rs
use crate::error::ChelpError;
use crate::models::CliFlag;
use crate::storage::SchemaStore;
use interprocess::local_socket::tokio::prelude::*;
use interprocess::local_socket::{GenericNamespaced, ToNsName};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::task::JoinHandle;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "data")]
pub enum IpcRequest {
    Ping,
    Complete { buffer: String },
    Query { prompt: String },
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "data")]
pub enum IpcResponse {
    Pong,
    Suggestions { flags: Vec<CliFlag> },
    Answer { text: String },
    Error { message: String },
}

fn spawn_daemon_detached() {
    if let Ok(exe) = std::env::current_exe() {
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const DETACHED_PROCESS: u32 = 0x00000008;
            const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
            let _ = std::process::Command::new(exe)
                .arg("daemon")
                .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
                .spawn();
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = std::process::Command::new(exe)
                .arg("daemon")
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
        }
    }
}

pub async fn send_ipc_request(name: &str, req: &IpcRequest) -> Result<IpcResponse, ChelpError> {
    let socket_name = name.to_ns_name::<GenericNamespaced>()
        .map_err(|e| ChelpError::Ipc(e.to_string()))?;

    let mut stream = match LocalSocketStream::connect(socket_name).await {
        Ok(s) => s,
        Err(_) => {
            spawn_daemon_detached();
            let mut connected = None;
            for _ in 0..8 {
                tokio::time::sleep(tokio::time::Duration::from_millis(25)).await;
                if let Ok(sn) = name.to_ns_name::<GenericNamespaced>() {
                    if let Ok(s) = LocalSocketStream::connect(sn).await {
                        connected = Some(s);
                        break;
                    }
                }
            }
            connected.ok_or_else(|| ChelpError::Ipc("Cannot connect to chelp daemon".to_string()))?
        }
    };

    let bytes = serde_json::to_vec(req)?;
    let len_prefix = (bytes.len() as u32).to_le_bytes();

    stream.write_all(&len_prefix).await?;
    stream.write_all(&bytes).await?;

    let mut res_len_buf = [0u8; 4];
    stream.read_exact(&mut res_len_buf).await?;
    let res_len = u32::from_le_bytes(res_len_buf) as usize;

    let mut res_buf = vec![0u8; res_len];
    stream.read_exact(&mut res_buf).await?;

    let res: IpcResponse = serde_json::from_slice(&res_buf)?;
    Ok(res)
}

pub async fn start_daemon(store: SchemaStore, name: &str) -> Result<JoinHandle<()>, ChelpError> {
    let socket_name = name.to_ns_name::<GenericNamespaced>()
        .map_err(|e| ChelpError::Ipc(e.to_string()))?;

    let listener = interprocess::local_socket::ListenerOptions::new()
        .name(socket_name)
        .create_tokio()
        .map_err(|e| ChelpError::Ipc(format!("Cannot bind socket: {}", e)))?;

    let handle = tokio::spawn(async move {
        while let Ok(mut stream) = listener.accept().await {
            let store = store.clone();
            tokio::spawn(async move {
                let mut len_buf = [0u8; 4];
                if stream.read_exact(&mut len_buf).await.is_err() {
                    return;
                }
                let len = u32::from_le_bytes(len_buf) as usize;
                let mut buf = vec![0u8; len];
                if stream.read_exact(&mut buf).await.is_err() {
                    return;
                }

                if let Ok(req) = serde_json::from_slice::<IpcRequest>(&buf) {
                    let res = match req {
                        IpcRequest::Ping => IpcResponse::Pong,
                        IpcRequest::Complete { buffer } => {
                            let parts: Vec<&str> = buffer.split_whitespace().collect();
                            if parts.is_empty() {
                                IpcResponse::Suggestions { flags: vec![] }
                            } else {
                                let binary = parts[0];
                                let last_token = if buffer.ends_with(' ') { "" } else { parts.last().unwrap_or(&"") };
                                let flags = store.match_flags(binary, &[], last_token).unwrap_or_default();
                                IpcResponse::Suggestions { flags }
                            }
                        }
                        IpcRequest::Query { prompt: _ } => IpcResponse::Answer {
                            text: "Not implemented yet".to_string(),
                        },
                    };

                    let res_bytes = serde_json::to_vec(&res).unwrap_or_default();
                    let res_len = (res_bytes.len() as u32).to_le_bytes();
                    let _ = stream.write_all(&res_len).await;
                    let _ = stream.write_all(&res_bytes).await;
                }
            });
        }
    });

    Ok(handle)
}
