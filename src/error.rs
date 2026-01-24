//! Error types for llmgrep.
//!
//! Error codes are organized by category:
//!
//! - **LLM-E001 to LLM-E099**: Database and file I/O errors
//! - **LLM-E100 to LLM-E199**: Query and parsing errors
//! - **LLM-E200 to LLM-E299**: Search execution errors
//! - **LLM-E300 to LLM-E399**: Path and argument validation errors
//! - **LLM-E900 to LLM-E999**: Internal and miscellaneous errors

use thiserror::Error;

/// Main error type for llmgrep operations.
#[derive(Error, Debug)]
pub enum LlmError {
    /// Database file not found at the specified path.
    #[error("Database not found: {path}")]
    DatabaseNotFound { path: String },

    /// Database file is corrupted or invalid.
    #[error("Database corrupted: {reason}")]
    DatabaseCorrupted { reason: String },

    /// Invalid query string provided.
    #[error("Invalid query: {query}")]
    InvalidQuery { query: String },

    /// Empty query string provided.
    #[error("Query cannot be empty")]
    EmptyQuery,

    /// Search operation failed.
    #[error("Search failed: {reason}")]
    SearchFailed { reason: String },

    /// Invalid file path provided.
    #[error("Invalid path: {path}")]
    InvalidPath { path: String },

    /// Invalid field name specified.
    #[error("Invalid field: {field}")]
    InvalidField { field: String },

    /// I/O error occurred.
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    /// SQLite error occurred.
    #[error("SQLite error: {0}")]
    SqliteError(#[from] rusqlite::Error),
}

impl LlmError {
    /// Returns the error code for this error.
    pub const fn error_code(&self) -> &'static str {
        match self {
            LlmError::DatabaseNotFound { .. } => "LLM-E001",
            LlmError::DatabaseCorrupted { .. } => "LLM-E002",
            LlmError::InvalidQuery { .. } => "LLM-E011",
            LlmError::EmptyQuery => "LLM-E012",
            LlmError::SearchFailed { .. } => "LLM-E021",
            LlmError::InvalidPath { .. } => "LLM-E031",
            LlmError::InvalidField { .. } => "LLM-E032",
            LlmError::IoError(_) => "LLM-E901",
            LlmError::SqliteError(_) => "LLM-E902",
        }
    }

    /// Returns the severity level for this error.
    pub const fn severity(&self) -> &'static str {
        match self {
            LlmError::InvalidField { .. } => "warning",
            _ => "error",
        }
    }

    /// Returns remediation hints for this error, if available.
    pub const fn remediation(&self) -> Option<&'static str> {
        match self {
            LlmError::DatabaseNotFound { .. } => {
                Some("Ensure the database path is correct and the file exists.")
            }
            LlmError::DatabaseCorrupted { .. } => {
                Some("The database file may be corrupted. Try reindexing your codebase.")
            }
            LlmError::InvalidQuery { .. } => {
                Some("Check that your query is properly formatted and valid.")
            }
            LlmError::EmptyQuery => {
                Some("Provide a non-empty query string using --query.")
            }
            LlmError::SearchFailed { .. } => {
                Some("Check that the database is valid and the query is supported.")
            }
            LlmError::InvalidPath { .. } => {
                Some("Ensure the path is valid and accessible.")
            }
            LlmError::InvalidField { .. } => {
                Some("Valid fields: context, snippet, score, fqn, canonical_fqn, display_fqn, all")
            }
            LlmError::IoError(_) => {
                Some("Check file permissions and disk space.")
            }
            LlmError::SqliteError(_) => {
                Some("The database may be locked or corrupted. Try reopening the database.")
            }
        }
    }
}
