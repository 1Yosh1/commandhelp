// tests/pty_interactive_test.rs
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::Read;

#[test]
fn test_pty_session_allocation_and_dimensions() {
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("Failed to create native PTY pair");

    let size = pair.master.get_size().expect("Failed to get PTY size");
    assert_eq!(size.cols, 80);
    assert_eq!(size.rows, 24);
}

#[test]
fn test_pty_spawn_binary_and_capture_output() {
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("Failed to create PTY pair");

    let exe = assert_cmd::cargo::cargo_bin("chelp");
    let mut cmd = CommandBuilder::new(exe);
    cmd.arg("--version");
    cmd.env("CHELP_NO_AUTO_SPAWN", "1");

    let mut child = pair.slave.spawn_command(cmd).expect("Failed to spawn process in PTY");
    drop(pair.slave);

    let mut reader = pair.master.try_clone_reader().expect("Failed to clone PTY reader");
    let (tx, rx) = std::sync::mpsc::channel();

    let _reader_thread = std::thread::spawn(move || {
        let mut buf = [0u8; 1024];
        while let Ok(n) = reader.read(&mut buf) {
            if n == 0 {
                break;
            }
            if tx.send(String::from_utf8_lossy(&buf[..n]).to_string()).is_err() {
                break;
            }
        }
    });

    let mut output = String::new();
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(5);

    while start.elapsed() < timeout {
        if let Ok(chunk) = rx.recv_timeout(std::time::Duration::from_millis(100)) {
            output.push_str(&chunk);
            if output.contains("chelp") {
                break;
            }
        }
        if let Ok(Some(_)) = child.try_wait() {
            while let Ok(chunk) = rx.try_recv() {
                output.push_str(&chunk);
            }
            break;
        }
    }

    let status = child.wait().expect("Failed to wait on PTY child process");
    assert!(status.success());
    assert!(output.contains("chelp 0.1.2") || output.contains("chelp"));
}

#[test]
fn test_pty_spawn_help_command() {
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 30,
            cols: 100,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("Failed to create PTY pair");

    let exe = assert_cmd::cargo::cargo_bin("chelp");
    let mut cmd = CommandBuilder::new(exe);
    cmd.arg("--help");
    cmd.env("CHELP_NO_AUTO_SPAWN", "1");

    let mut child = pair.slave.spawn_command(cmd).expect("Failed to spawn process in PTY");
    drop(pair.slave);

    let mut reader = pair.master.try_clone_reader().expect("Failed to clone PTY reader");
    let (tx, rx) = std::sync::mpsc::channel();

    let _reader_thread = std::thread::spawn(move || {
        let mut buf = [0u8; 2048];
        while let Ok(n) = reader.read(&mut buf) {
            if n == 0 {
                break;
            }
            if tx.send(String::from_utf8_lossy(&buf[..n]).to_string()).is_err() {
                break;
            }
        }
    });

    let mut output = String::new();
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(5);

    while start.elapsed() < timeout {
        if let Ok(chunk) = rx.recv_timeout(std::time::Duration::from_millis(100)) {
            output.push_str(&chunk);
            if output.contains("Usage:") || output.contains("Commands:") {
                break;
            }
        }
        if let Ok(Some(_)) = child.try_wait() {
            while let Ok(chunk) = rx.try_recv() {
                output.push_str(&chunk);
            }
            break;
        }
    }

    let status = child.wait().expect("Failed to wait on PTY child process");
    assert!(status.success());
    assert!(output.contains("Usage:") || output.contains("Commands:"));
    assert!(output.contains("query"));
    assert!(output.contains("complete"));
}
