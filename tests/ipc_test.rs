// tests/ipc_test.rs
use chelp::ipc::{send_ipc_request, start_daemon, IpcRequest, IpcResponse};
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
