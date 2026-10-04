// tests/ipc_test.rs
use chelp::ipc::{
    cleanup_socket, is_running, send_ipc_request, start_daemon, IpcRequest, IpcResponse,
    COMPLETE_BUDGET,
};
use chelp::storage::SchemaStore;
use tempfile::NamedTempFile;
use std::time::{Duration, Instant};

/// Tests own the daemon lifecycle; never let the client auto-spawn one.
fn isolate() {
    std::env::set_var("CHELP_NO_AUTO_SPAWN", "1");
}

fn unique_socket(label: &str) -> String {
    format!("chelp-test-{}-{}", label, std::process::id())
}

#[tokio::test]
async fn test_ipc_roundtrip_ping() {
    isolate();
    let tmp_db = NamedTempFile::new().unwrap();
    let store = SchemaStore::new(tmp_db.path()).unwrap();
    let socket = unique_socket("ping");

    let server_handle = start_daemon(store, &socket).await.unwrap();

    // Wait for the listener to come up.
    let deadline = Instant::now() + Duration::from_secs(5);
    while !is_running(&socket).await {
        assert!(Instant::now() < deadline, "daemon never came up");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    let started = Instant::now();
    let res = send_ipc_request(&socket, &IpcRequest::Ping, Duration::from_secs(2))
        .await
        .expect("roundtrip");
    let elapsed = started.elapsed();

    assert_eq!(res, IpcResponse::Pong);
    // Spec §8.2: local roundtrip should be sub-millisecond; assert a ceiling
    // generous enough for a loaded CI box.
    assert!(elapsed < Duration::from_millis(500), "roundtrip took {:?}", elapsed);

    server_handle.abort();
    cleanup_socket(&socket);
}

#[tokio::test]
async fn test_client_gives_up_on_the_declared_budget() {
    isolate();
    let socket = unique_socket("absent");

    let started = Instant::now();
    let result = send_ipc_request(&socket, &IpcRequest::Ping, COMPLETE_BUDGET).await;
    let elapsed = started.elapsed();

    assert!(result.is_err(), "no daemon is listening");
    // The shell hook's typing must never stall: 15ms budget plus a little slack.
    assert!(
        elapsed < Duration::from_millis(250),
        "budget was {:?} but the client blocked for {:?}",
        COMPLETE_BUDGET,
        elapsed
    );
}

/// A daemon killed with SIGKILL leaves its socket file behind; the next start
/// must detect that nothing answers and reclaim the name.
#[cfg(unix)]
#[tokio::test]
async fn test_stale_socket_is_reclaimed() {
    isolate();
    let socket = unique_socket("stale");
    let tmp_db = NamedTempFile::new().unwrap();
    let store = SchemaStore::new(tmp_db.path()).unwrap();

    // Simulate the crash: a regular file sitting where the socket should be.
    std::fs::write(std::path::Path::new("/tmp").join(&socket), b"stale").unwrap();
    assert!(!is_running(&socket).await, "a plain file is not a live daemon");

    let server_handle = start_daemon(store.clone(), &socket)
        .await
        .expect("start_daemon should reclaim a stale socket");

    let res = send_ipc_request(&socket, &IpcRequest::Ping, Duration::from_secs(2))
        .await
        .expect("roundtrip after recovery");
    assert_eq!(res, IpcResponse::Pong);

    server_handle.abort();
    cleanup_socket(&socket);
}
