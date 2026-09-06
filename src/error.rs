use thiserror::Error;

#[derive(Error, Debug)]
pub enum ChelpError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("IPC error: {0}")]
    Ipc(String),
    #[error("Parser error: {0}")]
    Parser(String),
    #[error("AI provider error: {0}")]
    AiProvider(String),
    #[error("Configuration error: {0}")]
    Config(String),
}
