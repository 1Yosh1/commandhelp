# Task 4: Local IPC Protocol & Daemon/Client Communication

**Plan File:** `docs/superpowers/plans/2026-09-06-ai-cli-assistant.md`  
**Spec File:** `docs/superpowers/specs/2026-09-06-ai-cli-assistant-design.md`

## Files
- Create: `src/ipc.rs`
- Test: `tests/ipc_test.rs`

## Interfaces
- Consumes: `SchemaStore`, `CliFlag` from `storage.rs` and `models.rs`
- Produces:
  - `IpcRequest`: Enum `Ping`, `Complete { buffer: String }`, `Query { prompt: String }`
  - `IpcResponse`: Enum `Pong`, `Suggestions { flags: Vec<CliFlag> }`, `Answer { text: String }`, `Error { message: String }`
  - `start_daemon(store: SchemaStore, socket_name: &str) -> Result<JoinHandle<()>, ChelpError>`
  - `send_ipc_request(socket_name: &str, req: &IpcRequest) -> Result<IpcResponse, ChelpError>`

## Steps

### Step 1: Write the failing test

```rust
// tests/ipc_test.rs
use chelp::ipc::{IpcRequest, IpcResponse, send_ipc_request, start_daemon};
use chelp::storage::SchemaStore;
use tempfile::NamedTempFile;

#[tokio::test]
async fn test_ipc_roundtrip_ping() {
    let tmp_db = NamedTempFile::new().unwrap();
    let store = SchemaStore::new(tmp_db.path()).unwrap();
    let socket_name = format!("chelp-test-{}", std::process::id());

    let _server_handle = start_daemon(store, &socket_name).await.unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    let res = send_ipc_request(&socket_name, &IpcRequest::Ping).await.unwrap();
    assert_eq!(res, IpcResponse::Pong);
}
```

### Step 2: Run test to verify it fails

Run: `cargo test --test ipc_test`  
Expected: FAIL (module `chelp::ipc` not found)

### Step 3: Write minimal implementation

```rust
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

pub async fn send_ipc_request(name: &str, req: &IpcRequest) -> Result<IpcResponse, ChelpError> {
    let socket_name = name.to_ns_name::<GenericNamespaced>()
        .map_err(|e| ChelpError::Ipc(e.to_string()))?;

    let mut stream = LocalSocketStream::connect(socket_name)
        .await
        .map_err(|e| ChelpError::Ipc(format!("Cannot connect to daemon: {}", e)))?;

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

    let listener = LocalSocketListener::bind(socket_name)
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
```

Add `pub mod ipc;` to `src/lib.rs`.

### Step 4: Run test to verify it passes

Run: `cargo test --test ipc_test`  
Expected: PASS

### Step 5: Commit

```bash
git add src/ipc.rs tests/ipc_test.rs src/lib.rs
git commit -m "feat: implement local IPC daemon and client transport"
```
