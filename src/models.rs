use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct CliFlag {
    pub short: Option<String>,
    pub long: Option<String>,
    pub takes_value: bool,
    pub value_hint: Option<String>,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct CliCommandSchema {
    pub binary: String,
    pub subcommand_path: Vec<String>,
    pub usage: String,
    pub description: String,
    pub flags: Vec<CliFlag>,
    pub subcommands: Vec<String>,
    pub binary_mtime: u64,
    pub last_indexed: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SafetyLevel {
    Safe,
    Caution,
    Destructive,
}

/// Single owner of the label shown for a safety level (modal, CLI output, logs).
impl fmt::Display for SafetyLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            SafetyLevel::Safe => "SAFE",
            SafetyLevel::Caution => "CAUTION",
            SafetyLevel::Destructive => "DESTRUCTIVE",
        };
        f.write_str(label)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct AiCommandResponse {
    pub command: String,
    pub explanation: String,
    pub safety_level: SafetyLevel,
    pub destructive_warning: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ShellContext {
    pub os: String,
    pub shell: String,
    pub cwd: String,
}
