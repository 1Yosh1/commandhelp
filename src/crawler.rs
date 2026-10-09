use crate::error::ChelpError;
use crate::models::CliCommandSchema;
use crate::parser::parse_help_output;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

/// Spec §4.2: strict per-probe execution timeout so a paging/interactive tool
/// can never wedge the indexer.
const PROBE_TIMEOUT: Duration = Duration::from_millis(1500);

/// Runs one probe attempt and returns its output (stdout, or stderr when
/// stdout is empty), `None` when the probe produced nothing usable.
///
/// `require_success` is for probes that are *not* the binary under test: a
/// missing `man` page prints an error to stderr, which must not be mistaken for
/// documentation.
fn run_probe(binary: &str, args: &[String], require_success: bool) -> Option<String> {
    let mut child = Command::new(binary)
        .args(args)
        .env("PAGER", "cat")
        .env("MANPAGER", "cat")
        .env("CI", "true")
        .env("TERM", "dumb")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;

    let start = std::time::Instant::now();
    loop {
        match child.try_wait().ok()? {
            Some(_) => {
                let output = child.wait_with_output().ok()?;
                if require_success && !output.status.success() {
                    return None;
                }
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                if !stdout.trim().is_empty() {
                    return Some(stdout);
                }
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                if !stderr.trim().is_empty() {
                    return Some(stderr);
                }
                return None;
            }
            None => {
                if start.elapsed() > PROBE_TIMEOUT {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}

/// Spec §4.2 probe sequence: `<bin> --help` -> `<bin> -h` -> `<bin> help` -> `man <bin>`.
pub fn crawl_command_help(binary: &str, subcommands: &[String]) -> Result<String, ChelpError> {
    let sub = subcommands.to_vec();

    let attempts: [(Vec<String>, bool); 4] = [
        ([sub.clone(), vec!["--help".to_string()]].concat(), false),
        ([sub.clone(), vec!["-h".to_string()]].concat(), false),
        ([vec!["help".to_string()], sub.clone()].concat(), false),
        (vec!["man".to_string(), binary.to_string()], true),
    ];

    for (args, require_success) in attempts {
        // `man <bin>` runs a different program than the binary under test.
        let target = if args.first().map(String::as_str) == Some("man") {
            "man"
        } else {
            binary
        };
        if let Some(out) = run_probe(target, &args, require_success) {
            return Ok(out);
        }
    }

    Err(ChelpError::Parser(format!(
        "No help output found for '{}'",
        binary
    )))
}

/// Resolves a binary to its on-disk path so its mtime can be tracked (spec §7.4).
pub fn resolve_binary(binary: &str) -> Option<PathBuf> {
    if binary.contains('/') || binary.contains('\\') {
        let p = PathBuf::from(binary);
        return p.is_file().then_some(p);
    }
    let paths = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&paths) {
        let candidate = dir.join(binary);
        if candidate.is_file() {
            return Some(candidate);
        }
        #[cfg(windows)]
        {
            let exe = dir.join(format!("{}.exe", binary));
            if exe.is_file() {
                return Some(exe);
            }
        }
    }
    None
}

/// Modification time of the binary in seconds since the epoch, `0` when unknown.
/// A change (e.g. `brew upgrade`) makes the daemon re-index the schema.
pub fn binary_mtime(binary: &str) -> u64 {
    let Some(path) = resolve_binary(binary) else {
        return 0;
    };
    let Ok(meta) = std::fs::metadata(&path) else {
        return 0;
    };
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Crawl + parse + stamp: the one entry point the daemon uses to index a binary.
pub fn crawl_schema(binary: &str, subcommands: &[String]) -> Result<CliCommandSchema, ChelpError> {
    let help = crawl_command_help(binary, subcommands)?;
    let mut schema = parse_help_output(binary, subcommands, &help)?;
    schema.binary_mtime = binary_mtime(binary);
    Ok(schema)
}
