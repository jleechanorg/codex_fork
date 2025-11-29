//! Error types for the extensions system

use std::path::PathBuf;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, ExtensionError>;

#[derive(Error, Debug)]
pub enum ExtensionError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("YAML parsing error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("Command not found: {0}")]
    CommandNotFound(String),

    #[error("Invalid command format in file {path}: {reason}")]
    InvalidCommandFormat { path: PathBuf, reason: String },

    #[error("Hook execution failed: {0}")]
    HookExecutionFailed(String),

    #[error("Hook timeout after {timeout_ms}ms")]
    HookTimeout { timeout_ms: u64 },

    #[error("Invalid hook configuration: {0}")]
    InvalidHookConfig(String),

    #[error("Settings file error at {path}: {reason}")]
    SettingsError { path: PathBuf, reason: String },

    #[error("Command file not found: {0}")]
    CommandFileNotFound(PathBuf),

    #[error("Hook script not found: {0}")]
    HookScriptNotFound(PathBuf),
}
