use crate::error::ChelpError;
use std::process::{Command, Stdio};
use std::time::Duration;

pub fn crawl_command_help(binary: &str, subcommands: &[String]) -> Result<String, ChelpError> {
    let mut args = subcommands.to_vec();
    args.push("--help".to_string());

    let mut child = Command::new(binary)
        .args(&args)
        .env("PAGER", "cat")
        .env("MANPAGER", "cat")
        .env("CI", "true")
        .env("TERM", "dumb")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| ChelpError::Parser(format!("Failed to execute '{}': {}", binary, e)))?;

    let start = std::time::Instant::now();
    loop {
        match child.try_wait()? {
            Some(_status) => {
                let output = child.wait_with_output()?;
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                if !stdout.is_empty() {
                    return Ok(stdout);
                }
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                return Ok(stderr);
            }
            None => {
                if start.elapsed() > Duration::from_millis(1500) {
                    let _ = child.kill();
                    return Err(ChelpError::Parser(format!("Timed out reading help from '{}'", binary)));
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}