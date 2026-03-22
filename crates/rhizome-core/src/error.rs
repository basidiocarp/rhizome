//! Error types for rhizome code intelligence operations.
//!
//! Provides typed errors for public APIs so consumers can match on specific
//! error variants rather than relying on error message strings.

use thiserror::Error;

/// ─────────────────────────────────────────────────────────────────────────
/// RhizomeError
/// ─────────────────────────────────────────────────────────────────────────
/// Comprehensive error type for all rhizome code intelligence operations.
#[derive(Debug, Error)]
pub enum RhizomeError {
    /// Unsupported or unrecognized programming language.
    #[error("unsupported language: {0}")]
    UnsupportedLanguage(String),

    /// File not found at the specified path.
    #[error("file not found: {0}")]
    FileNotFound(String),

    /// Parse error during code analysis.
    #[error("parse error: {0}")]
    ParseError(String),

    /// LSP (Language Server Protocol) related error.
    #[error("LSP error: {0}")]
    LspError(String),

    /// Configuration loading or validation error.
    #[error("config error: {0}")]
    Config(String),

    /// Symbol not found in the analyzed code.
    #[error("symbol not found: {0}")]
    SymbolNotFound(String),

    /// Backend operation not supported by the selected backend.
    #[error("operation not supported: {0}")]
    NotSupported(String),

    /// IO error during file operations.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON parsing error.
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    /// TOML parsing error.
    #[error("TOML parse error: {0}")]
    Toml(#[from] toml::de::Error),

    /// Generic error message.
    #[error("{0}")]
    Other(String),
}

/// ─────────────────────────────────────────────────────────────────────────
/// Result Type Alias
/// ─────────────────────────────────────────────────────────────────────────
/// Convenient alias for `Result<T, RhizomeError>`.
pub type Result<T> = std::result::Result<T, RhizomeError>;
