// src/config.rs
use crate::error::ChelpError;
use directories::ProjectDirs;
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

pub fn get_config_dir() -> PathBuf {
    if let Some(proj_dirs) = ProjectDirs::from("dev", "chelp", "chelp") {
        let dir = proj_dirs.config_dir();
        let _ = fs::create_dir_all(dir);
        dir.to_path_buf()
    } else {
        let dir = dirs_fallback();
        let _ = fs::create_dir_all(&dir);
        dir
    }
}

fn dirs_fallback() -> PathBuf {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".chelp")
}

pub fn get_config_path() -> PathBuf {
    // Check ~/.chelp/config.toml first
    let home_cfg = dirs_fallback().join("config.toml");
    if home_cfg.exists() {
        return home_cfg;
    }
    get_config_dir().join("config.toml")
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
            config.ai.model = Some("gemini-2.5-flash".to_string());
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
