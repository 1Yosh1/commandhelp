use serde::{Deserialize, Serialize};

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
