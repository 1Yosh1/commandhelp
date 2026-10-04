// src/config.rs
use crate::error::ChelpError;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct ChelpConfig {
    #[serde(default)]
    pub ai: AiConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct AiConfig {
    pub provider: String,
    pub model: Option<String>,
    pub api_key: Option<String>,
    pub endpoint: Option<String>,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            provider: "gemini".to_string(),
            model: Some("gemini-2.5-flash".to_string()),
            api_key: None,
            endpoint: None,
        }
    }
}

impl Default for ChelpConfig {
    fn default() -> Self {
        Self {
            ai: AiConfig::default(),
        }
    }
}

fn home_dir() -> PathBuf {
    let raw = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(raw)
}

/// Single owner of every path chelp writes to: `$CHELP_HOME` when set (tests,
/// sandboxes), otherwise `~/.chelp` (spec §2.3).
pub fn get_config_dir() -> PathBuf {
    let dir = match std::env::var("CHELP_HOME") {
        Ok(custom) if !custom.trim().is_empty() => PathBuf::from(custom),
        _ => home_dir().join(".chelp"),
    };
    let _ = fs::create_dir_all(&dir);
    dir
}

pub fn get_config_path() -> PathBuf {
    get_config_dir().join("config.toml")
}

pub fn get_db_path() -> PathBuf {
    get_config_dir().join("data.db")
}

pub fn get_log_path() -> PathBuf {
    get_config_dir().join("chelp.log")
}

pub fn load_config() -> Result<ChelpConfig, ChelpError> {
    let path = get_config_path();
    if path.exists() {
        let content = fs::read_to_string(&path)?;
        let config: ChelpConfig = toml::from_str(&content)
            .map_err(|e| ChelpError::Config(format!("Failed to parse config: {}", e)))?;
        Ok(config)
    } else {
        // Auto-detect from environment variables
        let mut config = ChelpConfig::default();
        if let Ok(key) = std::env::var("GEMINI_API_KEY") {
            config.ai.provider = "gemini".to_string();
            config.ai.api_key = Some(key);
        } else if let Ok(key) = std::env::var("OPENAI_API_KEY") {
            config.ai.provider = "openai".to_string();
            config.ai.api_key = Some(key);
            config.ai.model = Some("gpt-4o-mini".to_string());
        } else if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
            config.ai.provider = "anthropic".to_string();
            config.ai.api_key = Some(key);
            config.ai.model = Some("claude-3-5-haiku-20241022".to_string());
        }
        Ok(config)
    }
}

pub fn save_config(config: &ChelpConfig) -> Result<PathBuf, ChelpError> {
    let path = get_config_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let content = toml::to_string_pretty(config)
        .map_err(|e| ChelpError::Config(format!("Failed to serialize config: {}", e)))?;
    fs::write(&path, content)?;
    Ok(path)
}
