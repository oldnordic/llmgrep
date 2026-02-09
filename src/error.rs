//! Error types for llmgrep.
//!
//! Error codes are organized by category:
//!
//! - **LLM-E001 to LLM-E099**: Database and file I/O errors
//! - **LLM-E100 to LLM-E199**: Query and parsing errors
//!   - LLM-E101: Regex pattern rejection (ReDoS prevention)
//!   - LLM-E102: Resource limit exceeded
//!   - LLM-E103: Path validation failure
//!   - LLM-E105: Magellan CLI not found
//!   - LLM-E106: Ambiguous symbol name
//!   - LLM-E107: Magellan version mismatch
//!   - LLM-E108: Magellan execution failed
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

    /// Regex pattern rejected for security reasons.
    #[error("Regex pattern rejected: {reason}")]
    RegexRejected { reason: String },

    /// Resource limit exceeded.
    #[error("Resource limit exceeded: {resource} (max: {limit}, provided: {provided})")]
    ResourceLimitExceeded {
        resource: String,
        limit: usize,
        provided: usize,
    },

    /// Path validation failed.
    #[error("Path validation failed: {path} - {reason}")]
    PathValidationFailed { path: String, reason: String },

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

    /// JSON serialization/deserialization error occurred.
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    /// Regex compilation error occurred.
    #[error("Regex error: {0}")]
    RegexError(#[from] regex::Error),

    /// Magellan CLI not found in PATH.
    #[error(
        "LLM-E105: Magellan CLI not found in PATH.\n\n\
         Magellan v2.1.0+ is required for algorithm features (--reachable-from, --dead-code-in, etc.).\n\n\
         Installation methods:\n\
         \x20  1. Cargo:     cargo install magellan\n\
         \x20  2. Binary:    cargo binstall magellan\n\
         \x20  3. Docs:      https://docs.rs/magellan\n\n\
         After installation, run: magellan --version"
    )]
    MagellanNotFound,

    /// Symbol name is ambiguous (multiple matches).
    #[error("Ambiguous symbol name '{name}': {count} matches. Use --symbol-id with full SymbolId.")]
    AmbiguousSymbolName { name: String, count: usize },

    /// Magellan version is incompatible.
    #[error("Magellan version {current} is incompatible. Required: {required}")]
    MagellanVersionMismatch { current: String, required: String },

    /// Magellan algorithm execution failed.
    #[error("Magellan {algorithm} execution failed: {stderr}")]
    MagellanExecutionFailed { algorithm: String, stderr: String },

    /// Native-V2 backend detected but llmgrep was built without native-v2 support.
    #[error(
        "LLM-E109: Native-V2 backend detected but llmgrep was built without native-v2 support.\n\n\
         Database: {path}\n\n\
         To enable Native-V2 support, rebuild llmgrep with:\n\
         \x20  cargo install llmgrep --features native-v2\n\
         \x20  or: cargo build --release --features native-v2\n\n\
         For more information, see: https://docs.rs/llmgrep/latest/llmgrep/"
    )]
    NativeV2BackendNotSupported { path: String },

    /// Backend detection failed for the database.
    #[error("LLM-E110: Backend detection failed for database: {path}\nReason: {reason}")]
    BackendDetectionFailed { path: String, reason: String },
}

impl LlmError {
    /// Returns the error code for this error.
    pub const fn error_code(&self) -> &'static str {
        match self {
            LlmError::DatabaseNotFound { .. } => "LLM-E001",
            LlmError::DatabaseCorrupted { .. } => "LLM-E002",
            LlmError::InvalidQuery { .. } => "LLM-E011",
            LlmError::EmptyQuery => "LLM-E012",
            LlmError::RegexRejected { .. } => "LLM-E101",
            LlmError::ResourceLimitExceeded { .. } => "LLM-E102",
            LlmError::PathValidationFailed { .. } => "LLM-E103",
            LlmError::SearchFailed { .. } => "LLM-E021",
            LlmError::InvalidPath { .. } => "LLM-E031",
            LlmError::InvalidField { .. } => "LLM-E032",
            LlmError::IoError(_) => "LLM-E901",
            LlmError::SqliteError(_) => "LLM-E902",
            LlmError::JsonError(_) => "LLM-E903",
            LlmError::RegexError(_) => "LLM-E904",
            LlmError::MagellanNotFound => "LLM-E105",
            LlmError::AmbiguousSymbolName { .. } => "LLM-E106",
            LlmError::MagellanVersionMismatch { .. } => "LLM-E107",
            LlmError::MagellanExecutionFailed { .. } => "LLM-E108",
            LlmError::NativeV2BackendNotSupported { .. } => "LLM-E109",
            LlmError::BackendDetectionFailed { .. } => "LLM-E110",
        }
    }

    /// Returns the severity level for this error.
    pub const fn severity(&self) -> &'static str {
        match self {
            LlmError::InvalidField { .. } => "warning",
            LlmError::JsonError(_) | LlmError::RegexError(_) => "error",
            LlmError::RegexRejected { .. } => "error",
            LlmError::ResourceLimitExceeded { .. } => "error",
            LlmError::PathValidationFailed { .. } => "error",
            LlmError::MagellanNotFound => "error",
            LlmError::AmbiguousSymbolName { .. } => "error",
            LlmError::MagellanVersionMismatch { .. } => "error",
            LlmError::MagellanExecutionFailed { .. } => "error",
            LlmError::NativeV2BackendNotSupported { .. } => "error",
            LlmError::BackendDetectionFailed { .. } => "error",
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
            LlmError::EmptyQuery => Some("Provide a non-empty query string using --query."),
            LlmError::SearchFailed { .. } => {
                Some("Check that the database is valid and the query is supported.")
            }
            LlmError::InvalidPath { .. } => Some("Ensure the path is valid and accessible."),
            LlmError::InvalidField { .. } => {
                Some("Valid fields: context, snippet, score, fqn, canonical_fqn, display_fqn, all")
            }
            LlmError::IoError(_) => Some("Check file permissions and disk space."),
            LlmError::SqliteError(_) => {
                Some("The database may be locked or corrupted. Try reopening the database.")
            }
            LlmError::JsonError(_) => {
                Some("JSON serialization error. This may indicate corrupted data.")
            }
            LlmError::RegexError(_) => Some("Invalid regular expression. Check your query syntax."),
            LlmError::RegexRejected { .. } => Some(
                "Simplify the regex pattern or avoid nested quantifiers and excessive alternation",
            ),
            LlmError::ResourceLimitExceeded { .. } => {
                Some("Reduce the resource value to within the allowed maximum")
            }
            LlmError::PathValidationFailed { .. } => {
                Some("Check that the path exists and is within the allowed directory structure")
            }
            LlmError::MagellanNotFound => {
                Some("Install magellan v2.1.0+: cargo install magellan")
            }
            LlmError::AmbiguousSymbolName { .. } => {
                Some("Use --symbol-id with full 32-character SymbolId for unambiguous reference.")
            }
            LlmError::MagellanVersionMismatch { .. } => {
                Some("Update magellan: cargo install magellan-cli --force")
            }
            LlmError::MagellanExecutionFailed { .. } => {
                Some("Check magellan --version and database compatibility.")
            }
            LlmError::NativeV2BackendNotSupported { .. } => {
                Some("Rebuild llmgrep with: cargo install llmgrep --features native-v2")
            }
            LlmError::BackendDetectionFailed { .. } => {
                Some("Check that the database file exists and is a valid Magellan database.")
            }
        }
    }
}
