// tests/pty_interactive_test.rs
//
// Drives the real interactive surface instead of merely spawning the binary:
// the confirmation modal in a PTY, the cancel/confirm key paths, the destructive
// confirmation gate, the terminal-less fallback, and the completion pipeline
// end to end (daemon + on-demand `--help` indexing).

use assert_cmd::cargo::cargo_bin;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const WAIT: Duration = Duration::from_secs(20);

/// Mock OpenAI-compatible endpoint so the query flow is exercised without keys.
fn spawn_mock_ai(command: &str, explanation: &str) -> String {
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").expect("mock AI bind");
    let addr = listener.local_addr().expect("mock AI addr");
    let payload = serde_json::json!({
        "choices": [{ "message": { "content": serde_json::json!({
            "command": command,
            "explanation": explanation,
            "safety_level": "safe",
            "destructive_warning": null
        }).to_string() }}]
    })
    .to_string();

    std::thread::spawn(move || {
        use std::io::{BufRead, BufReader};
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let cloned = stream.try_clone().expect("mock AI clone");
            let mut buffered = BufReader::new(cloned);
            let mut content_length = 0usize;
            loop {
                let mut line = String::new();
                match buffered.read_line(&mut line) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
                if line == "\r\n" || line == "\n" {
                    break;
                }
                let lower = line.to_ascii_lowercase();
                if let Some(rest) = lower.strip_prefix("content-length:") {
                    content_length = rest.trim().parse().unwrap_or(0);
                }
            }
            let mut body = vec![0u8; content_length];
            let _ = buffered.read_exact(&mut body);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                payload.len(),
                payload
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });

    format!("http://{}/v1", addr)
}

fn unique_tag(label: &str) -> String {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    format!(
        "chelp-test-{}-{}-{}",
        label,
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    )
}

struct Sandbox {
    home: PathBuf,
    socket: String,
    _guard: tempfile::TempDir,
}

impl Sandbox {
    fn new(label: &str) -> Self {
        let guard = tempfile::tempdir().expect("temp home");
        Self {
            home: guard.path().to_path_buf(),
            socket: unique_tag(label),
            _guard: guard,
        }
    }

    fn env(&self) -> Vec<(String, String)> {
        vec![
            (
                "CHELP_HOME".to_string(),
                self.home.to_string_lossy().to_string(),
            ),
            ("CHELP_SOCKET".to_string(), self.socket.clone()),
            // The tests own the daemon lifecycle.
            ("CHELP_NO_AUTO_SPAWN".to_string(), "1".to_string()),
        ]
    }

    fn write_config(&self, endpoint: &str) {
        let config = format!(
            "[ai]\nprovider = \"custom\"\nmodel = \"mock\"\napi_key = \"test-key\"\nendpoint = \"{}\"\n",
            endpoint
        );
        std::fs::write(self.home.join("config.toml"), config).expect("write config");
    }

    fn binary(&self) -> PathBuf {
        cargo_bin("chelp")
    }

    /// Starts a foreground daemon bound to this sandbox's socket.
    fn start_daemon(&self, extra_env: &[(String, String)]) -> std::process::Child {
        let mut cmd = std::process::Command::new(self.binary());
        cmd.args(["daemon"]).envs(self.env());
        for (key, value) in extra_env {
            cmd.env(key, value);
        }
        cmd.stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("spawn daemon")
    }

    /// Polls `chelp complete` until it yields something or the budget runs out.
    fn wait_for_suggestion(&self, buffer: &str, timeout: Duration) -> String {
        let binary = self.binary();
        let env = self.env();
        let deadline = Instant::now() + timeout;
        let mut last = String::new();
        while Instant::now() < deadline {
            let output = std::process::Command::new(&binary)
                .args(["complete", "--", buffer])
                .envs(env.iter().cloned())
                .output();
            if let Ok(output) = output {
                last = String::from_utf8_lossy(&output.stdout).to_string();
                if !last.trim().is_empty() {
                    return last;
                }
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        last
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        chelp::ipc::cleanup_socket(&self.socket);
    }
}

enum Step {
    /// Block until the terminal shows this text.
    WaitFor(&'static str),
    /// Write raw bytes to the PTY (e.g. an Enter or Escape keypress).
    Send(&'static [u8]),
}

/// Runs `chelp` under a real PTY, performing `steps`, and returns everything the
/// terminal saw. Panics with the captured output when a marker never appears.
fn run_in_pty(
    args: &[&str],
    env: &[(String, String)],
    steps: &[Step],
) -> (String, Option<portable_pty::ExitStatus>) {
    run_program_in_pty(&cargo_bin("chelp"), args, env, steps)
}

/// Like `run_in_pty`, but launches an arbitrary program instead of `chelp`, so
/// a test can reshape the process (e.g. run it through `/bin/sh -c ... < /dev/null`)
/// while the PTY stays its controlling terminal.
fn run_program_in_pty(
    program: &std::path::Path,
    args: &[&str],
    env: &[(String, String)],
    steps: &[Step],
) -> (String, Option<portable_pty::ExitStatus>) {
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("open pty");

    let mut cmd = CommandBuilder::new(program);
    cmd.args(args);
    for (key, value) in env {
        cmd.env(key, value);
    }

    let mut child = pair.slave.spawn_command(cmd).expect("spawn in pty");
    drop(pair.slave);

    let mut reader = pair.master.try_clone_reader().expect("pty reader");
    let mut writer = pair.master.take_writer().expect("pty writer");
    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if tx.send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
            }
        }
    });

    let mut output = String::new();
    let drain = |output: &mut String| {
        while let Ok(chunk) = rx.try_recv() {
            output.push_str(&String::from_utf8_lossy(&chunk));
        }
    };

    for step in steps {
        match step {
            Step::WaitFor(marker) => {
                let deadline = Instant::now() + WAIT;
                while Instant::now() < deadline {
                    drain(&mut output);
                    if output.contains(marker) {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(25));
                }
                assert!(
                    output.contains(marker),
                    "never saw {:?} in output:\n{}",
                    marker,
                    output
                );
            }
            Step::Send(bytes) => {
                // A child that died mid-flow makes the master EIO; keep going —
                // the status/tail assertions below report the real failure.
                let _ = writer.write_all(bytes);
                writer.flush().ok();
                std::thread::sleep(Duration::from_millis(250));
                drain(&mut output);
            }
        }
    }

    let deadline = Instant::now() + WAIT;
    let mut status = None;
    while Instant::now() < deadline {
        drain(&mut output);
        match child.try_wait() {
            Ok(Some(exit)) => {
                status = Some(exit);
                break;
            }
            _ => std::thread::sleep(Duration::from_millis(50)),
        }
    }
    if status.is_none() {
        let _ = child.kill();
        let _ = child.wait();
    }
    // Let the reader thread flush whatever the child's dying breath produced.
    std::thread::sleep(Duration::from_millis(200));
    drain(&mut output);

    (output, status)
}

/// The text the terminal shows after the modal has come and gone.
fn tail(output: &str) -> &str {
    output.trim_end()
}

#[test]
fn query_enter_renders_modal_and_prints_command() {
    let sandbox = Sandbox::new("enter");
    let endpoint = spawn_mock_ai("ls -la", "Lists files in the current directory.");
    sandbox.write_config(&endpoint);

    let (output, status) = run_in_pty(
        &["query", "list the files"],
        &sandbox.env(),
        &[
            Step::WaitFor("Suggested Command"),
            Step::WaitFor("ls -la"),
            Step::Send(b"\r"),
        ],
    );

    assert!(
        status.map(|s| s.success()).unwrap_or(false),
        "exit status in:\n{}",
        output
    );
    assert!(
        tail(&output).ends_with("ls -la"),
        "Enter should surface the command; tail was:\n{}",
        tail(&output)
    );
}

#[test]
fn query_escape_cancels_without_emitting_command() {
    let sandbox = Sandbox::new("escape");
    let endpoint = spawn_mock_ai("ls -la", "Lists files in the current directory.");
    sandbox.write_config(&endpoint);

    let (output, status) = run_in_pty(
        &["query", "list the files"],
        &sandbox.env(),
        &[Step::WaitFor("Suggested Command"), Step::Send(b"\x1b")],
    );

    assert!(
        status.map(|s| s.success()).unwrap_or(false),
        "exit status in:\n{}",
        output
    );
    assert!(
        !tail(&output).ends_with("ls -la"),
        "Escape must cancel silently; tail was:\n{}",
        tail(&output)
    );
}

#[test]
fn destructive_command_needs_a_second_confirmation_key() {
    let sandbox = Sandbox::new("destructive");
    // The model calls it "safe"; the local heuristic must still gate it.
    let endpoint = spawn_mock_ai(
        "rm -rf /tmp/scratch-workspace",
        "Removes the scratch workspace.",
    );
    sandbox.write_config(&endpoint);

    let (output, status) = run_in_pty(
        &["query", "wipe the scratch workspace"],
        &sandbox.env(),
        &[
            Step::WaitFor("HIGH RISK"),
            Step::Send(b"\r"), // first Enter only arms the confirmation
            Step::Send(b"y"),
        ],
    );

    assert!(
        status.map(|s| s.success()).unwrap_or(false),
        "exit status in:\n{}",
        output
    );
    assert!(
        tail(&output).ends_with("rm -rf /tmp/scratch-workspace"),
        "pressing y after Enter should confirm; tail was:\n{}",
        tail(&output)
    );
}

#[test]
fn query_without_a_terminal_still_hands_back_the_command() {
    use std::process::{Command, Stdio};

    let sandbox = Sandbox::new("no-tty");
    let endpoint = spawn_mock_ai("ls -la", "Lists files in the current directory.");
    sandbox.write_config(&endpoint);

    let mut child = Command::new(sandbox.binary());
    child
        .args(["query", "list the files"])
        .envs(sandbox.env())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = child.output().expect("run chelp query");

    assert!(
        output.status.success(),
        "non-tty query failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "ls -la",
        "a piped invocation should print only the command"
    );
}

#[test]
fn complete_answers_from_an_indexed_schema() {
    let sandbox = Sandbox::new("seeded");

    // Seed the store the way the indexer would. The binary name does not exist
    // on disk, so the daemon will not try to re-index it (mtime stays 0).
    let store = chelp::storage::SchemaStore::new(&sandbox.home.join("data.db")).expect("store");
    let schema = chelp::models::CliCommandSchema {
        binary: "demo".to_string(),
        subcommand_path: vec![],
        usage: "demo <command>".to_string(),
        description: "Demo CLI".to_string(),
        flags: vec![],
        subcommands: vec![
            "status".to_string(),
            "stash".to_string(),
            "switch".to_string(),
        ],
        binary_mtime: 0,
        last_indexed: 1,
    };
    store.save_schema(&schema).expect("seed schema");

    let mut daemon = sandbox.start_daemon(&[]);
    let suggestion = sandbox.wait_for_suggestion("demo st", Duration::from_secs(10));
    let _ = daemon.kill();
    let _ = daemon.wait();

    assert!(
        suggestion.lines().next().map(str::trim) == Some("demo status"),
        "expected 'demo status', got {:?}",
        suggestion
    );
}

/// The pipeline the audit found dead: probe a binary, parse its `--help`, store
/// it in the background, and answer the next keystroke with it.
#[cfg(unix)]
#[test]
fn complete_indexes_an_unknown_binary_on_demand() {
    use std::os::unix::fs::PermissionsExt;

    let sandbox = Sandbox::new("on-demand");
    let bin_dir = tempfile::tempdir().expect("bin dir");
    let fake = bin_dir.path().join("fakcli");
    std::fs::write(
        &fake,
        "#!/bin/sh\ncat <<'EOF'\nUsage: fakcli [OPTIONS] COMMAND\n\nOptions:\n  -v, --verbose     Run with verbose output\n  -o, --output string   Path to write results\n\nCommands:\n  run       Run a job\n  list      List jobs\nEOF\n",
    )
    .expect("write fake cli");
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).expect("chmod");

    let path = format!(
        "{}{}",
        bin_dir.path().to_string_lossy(),
        if cfg!(windows) { ";" } else { ":" }
    );
    let path = match std::env::var("PATH") {
        Ok(existing) => format!("{}{}", path, existing),
        Err(_) => path,
    };

    let mut daemon = sandbox.start_daemon(&[("PATH".to_string(), path)]);
    let suggestion = sandbox.wait_for_suggestion("fakcli --v", Duration::from_secs(15));
    let subcommand = sandbox.wait_for_suggestion("fakcli l", Duration::from_secs(10));
    let _ = daemon.kill();
    let _ = daemon.wait();

    assert!(
        suggestion.contains("--verbose"),
        "expected on-demand flag indexing, got {:?}",
        suggestion
    );
    assert!(
        subcommand.contains("fakcli list"),
        "expected on-demand subcommand indexing, got {:?}",
        subcommand
    );
}

/// Regression: the modal must stay interactive when stdin is NOT a tty — the
/// exact shape of a child spawned from a zsh ZLE widget. On macOS a freshly
/// opened /dev/tty fd cannot be kqueue-registered (EINVAL), so crossterm's
/// default mio source died with "Failed to initialize input reader" before it
/// could read a single key. The fix is the `use-dev-tty` feature (poll(2) in
/// place of kqueue); this test fails if that feature is ever dropped.
#[cfg(unix)]
#[test]
fn modal_still_reads_keys_when_stdin_is_not_a_tty() {
    let sandbox = Sandbox::new("stdin-null");
    let endpoint = spawn_mock_ai("ls -la", "Lists files in the current directory.");
    sandbox.write_config(&endpoint);

    // sh keeps the PTY as its controlling terminal but hands /dev/null to
    // chelp as stdin, so the binary must fall back to /dev/tty for input.
    let script = format!(
        "exec '{}' query 'list the files' < /dev/null",
        cargo_bin("chelp").display()
    );

    let (output, status) = run_program_in_pty(
        std::path::Path::new("/bin/sh"),
        &["-c", &script],
        &sandbox.env(),
        &[
            Step::WaitFor("Suggested Command"),
            Step::WaitFor("ls -la"),
            Step::Send(b"\r"),
        ],
    );

    assert!(
        status.map(|s| s.success()).unwrap_or(false),
        "chelp must read keys from /dev/tty when stdin is not a tty \
         (use-dev-tty regression?); output:\n{}",
        output
    );
    assert!(
        tail(&output).ends_with("ls -la"),
        "Enter in the modal should surface the command; tail was:\n{}",
        tail(&output)
    );
}
