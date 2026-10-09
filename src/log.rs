// src/log.rs
use crate::config::{get_config_dir, get_log_path};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

/// Spec §2.3: a rolling diagnostic log at `~/.chelp/chelp.log`. The active log
/// is rotated to `chelp.log.1` once it passes this size.
const MAX_LOG_BYTES: u64 = 1_000_000;

/// Append one timestamped diagnostic line. Logging is best-effort: a failure
/// here must never break the shell hook or the daemon.
pub fn log(component: &str, message: &str) {
    let path = get_log_path();
    if let Ok(meta) = fs::metadata(&path) {
        if meta.len() > MAX_LOG_BYTES {
            let _ = fs::rename(&path, get_config_dir().join("chelp.log.1"));
        }
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or_default();
        let _ = writeln!(file, "{} [{}] {}", now, component, message);
    }
}
