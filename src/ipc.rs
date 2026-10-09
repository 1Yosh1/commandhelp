// src/ipc.rs
//
// Transport only: framing, deadlines, auto-spawn and socket lifecycle. What a
// `Complete` request *means* lives in `complete.rs`.

use crate::complete;
use crate::error::ChelpError;
use crate::log::log;
use crate::storage::SchemaStore;
use interprocess::local_socket::tokio::prelude::*;
use interprocess::local_socket::{GenericNamespaced, ToNsName};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::task::JoinHandle;

pub const DEFAULT_SOCKET_NAME: &str = "chelp-ipc";

/// Spec §2.2: the hook must never stutter — a completion request that outlives
/// this budget resolves to "no suggestion".
pub const COMPLETE_BUDGET: Duration = Duration::from_millis(15);

const MAX_FRAME_BYTES: usize = 1 << 20;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "data")]
pub enum IpcRequest {
    Ping,
    Complete { buffer: String },
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "data")]
pub enum IpcResponse {
    Pong,
    Suggestions { lines: Vec<String> },
    Error { message: String },
}

/// On unix interprocess maps namespaced names to `$TMPDIR`-less `/tmp/<name>`;
/// knowing the path is what lets us recover from a crashed daemon.
#[cfg(unix)]
fn socket_file(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from("/tmp").join(name)
}

fn to_socket_name(name: &str) -> Result<interprocess::local_socket::Name<'_>, ChelpError> {
    name.to_ns_name::<GenericNamespaced>()
        .map_err(|e| ChelpError::Ipc(e.to_string()))
}

/// Spawns a detached `chelp daemon` (spec §2.2 auto-spawn, and `--detached`).
pub fn spawn_daemon_detached() {
    // kill switch must also stop the lib-level auto-spawn path, not just hooks
    if std::env::var("CHELP_NO_AUTO_SPAWN").is_ok()
        || std::env::var("CHELP_DISABLE_DAEMON").as_deref() == Ok("1")
    {
        return;
    }
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x00000008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
        let _ = std::process::Command::new(exe)
            .arg("daemon")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
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

/// True when a live daemon answers on `name`.
pub async fn is_running(name: &str) -> bool {
    let Ok(socket_name) = to_socket_name(name) else {
        return false;
    };
    LocalSocketStream::connect(socket_name).await.is_ok()
}

/// Sends one request, giving up when `budget` elapses. Connect failures trigger
/// a single auto-spawn plus short retries, all inside the same deadline.
pub async fn send_ipc_request(
    name: &str,
    req: &IpcRequest,
    budget: Duration,
) -> Result<IpcResponse, ChelpError> {
    match tokio::time::timeout(budget, send(name, req)).await {
        Ok(inner) => inner,
        Err(_) => Err(ChelpError::Ipc(format!(
            "daemon did not answer within {}ms",
            budget.as_millis()
        ))),
    }
}

async fn send(name: &str, req: &IpcRequest) -> Result<IpcResponse, ChelpError> {
    let mut stream = match LocalSocketStream::connect(to_socket_name(name)?).await {
        Ok(s) => s,
        Err(first_err) => {
            spawn_daemon_detached();
            let mut connected = None;
            for _ in 0..4 {
                tokio::time::sleep(Duration::from_millis(3)).await;
                if let Ok(socket_name) = to_socket_name(name) {
                    if let Ok(s) = LocalSocketStream::connect(socket_name).await {
                        connected = Some(s);
                        break;
                    }
                }
            }
            connected.ok_or_else(|| {
                ChelpError::Ipc(format!("cannot connect to chelp daemon ({})", first_err))
            })?
        }
    };

    let bytes = serde_json::to_vec(req)?;
    let len_prefix = (bytes.len() as u32).to_le_bytes();
    stream.write_all(&len_prefix).await?;
    stream.write_all(&bytes).await?;

    let mut res_len_buf = [0u8; 4];
    stream.read_exact(&mut res_len_buf).await?;
    let res_len = u32::from_le_bytes(res_len_buf) as usize;
    if res_len > MAX_FRAME_BYTES {
        return Err(ChelpError::Ipc(format!("oversized response ({} bytes)", res_len)));
    }

    let mut res_buf = vec![0u8; res_len];
    stream.read_exact(&mut res_buf).await?;
    Ok(serde_json::from_slice(&res_buf)?)
}

/// Removes a socket file left behind by a killed daemon. Named pipes on
/// Windows do not persist, so this is unix-only.
pub fn cleanup_socket(name: &str) {
    #[cfg(unix)]
    {
        let _ = std::fs::remove_file(socket_file(name));
    }
    #[cfg(not(unix))]
    {
        let _ = name;
    }
}

/// Runs the daemon listener until the returned handle is dropped/cancelled.
/// Binding recovers from a stale socket file left by a daemon that was killed
/// before it could clean up (spec §8.2 graceful recovery).
pub async fn start_daemon(store: SchemaStore, name: &str) -> Result<JoinHandle<()>, ChelpError> {
    let mut recovered = false;
    let listener = loop {
        match interprocess::local_socket::ListenerOptions::new()
            .name(to_socket_name(name)?)
            .create_tokio()
        {
            Ok(listener) => break listener,
            Err(e) => {
                if recovered {
                    return Err(ChelpError::Ipc(format!("cannot bind socket: {}", e)));
                }
                if is_running(name).await {
                    return Err(ChelpError::Ipc(format!(
                        "another chelp daemon already owns socket '{}'",
                        name
                    )));
                }
                log("daemon", &format!("removing stale socket '{}'", name));
                cleanup_socket(name);
                recovered = true;
            }
        }
    };

    let handle = tokio::spawn(async move {
        loop {
            let stream = match listener.accept().await {
                Ok(s) => s,
                Err(_) => break,
            };
            let store = store.clone();
            tokio::spawn(async move {
                let _ = serve(stream, store).await;
            });
        }
    });

    Ok(handle)
}

async fn serve<S>(mut stream: S, store: SchemaStore) -> Result<(), ChelpError>
where
    S: AsyncReadExt + AsyncWriteExt + Unpin,
{
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await?;
    let len = u32::from_le_bytes(len_buf) as usize;
    if len > MAX_FRAME_BYTES {
        return Err(ChelpError::Ipc(format!("oversized request ({} bytes)", len)));
    }
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf).await?;

    let req: IpcRequest = serde_json::from_slice(&buf)?;
    let res = match req {
        IpcRequest::Ping => IpcResponse::Pong,
        IpcRequest::Complete { buffer } => IpcResponse::Suggestions {
            lines: complete::handle_request(&store, &buffer),
        },
    };

    let res_bytes = serde_json::to_vec(&res)?;
    let res_len = (res_bytes.len() as u32).to_le_bytes();
    stream.write_all(&res_len).await?;
    stream.write_all(&res_bytes).await?;
    Ok(())
}
