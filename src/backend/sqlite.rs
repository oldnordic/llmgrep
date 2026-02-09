//! SQLite backend implementation.
//!
//! SqliteBackend provides the Backend trait implementation for SQLite databases.
//! This is the traditional storage backend and is always available.

use crate::error::LlmError;
use crate::output::{
    CallSearchResponse, ReferenceSearchResponse, SearchResponse,
};
use crate::query::SearchOptions;
use rusqlite::Connection;
use std::path::Path;

/// SQLite backend implementation.
///
/// Wraps a rusqlite Connection and implements the Backend trait.
/// The actual SQL queries will be moved from query.rs in Phase 18.
pub struct SqliteBackend {
    pub(crate) conn: Connection,
}

impl SqliteBackend {
    /// Open a SQLite database backend.
    ///
    /// # Arguments
    /// * `db_path` - Path to the SQLite database file
    pub fn open(db_path: &Path) -> Result<Self, LlmError> {
        let conn = Connection::open(db_path)?;
        Ok(Self { conn })
    }
}

impl super::BackendTrait for SqliteBackend {
    fn search_symbols(
        &self,
        _options: SearchOptions,
    ) -> Result<(SearchResponse, bool, bool), LlmError> {
        // TODO: Implement in Phase 18 (move existing query.rs logic)
        Err(LlmError::SearchFailed {
            reason: "SqliteBackend::search_symbols not yet implemented".to_string(),
        })
    }

    fn search_references(
        &self,
        _options: SearchOptions,
    ) -> Result<(ReferenceSearchResponse, bool), LlmError> {
        // TODO: Implement in Phase 18
        Err(LlmError::SearchFailed {
            reason: "SqliteBackend::search_references not yet implemented".to_string(),
        })
    }

    fn search_calls(
        &self,
        _options: SearchOptions,
    ) -> Result<(CallSearchResponse, bool), LlmError> {
        // TODO: Implement in Phase 18
        Err(LlmError::SearchFailed {
            reason: "SqliteBackend::search_calls not yet implemented".to_string(),
        })
    }

    fn ast(
        &self,
        _file: &Path,
        _position: Option<usize>,
        _limit: usize,
    ) -> Result<serde_json::Value, LlmError> {
        // TODO: Implement in Phase 18 (shell out to magellan ast)
        Err(LlmError::SearchFailed {
            reason: "SqliteBackend::ast not yet implemented".to_string(),
        })
    }

    fn find_ast(&self, _kind: &str) -> Result<serde_json::Value, LlmError> {
        // TODO: Implement in Phase 18 (shell out to magellan find-ast)
        Err(LlmError::SearchFailed {
            reason: "SqliteBackend::find_ast not yet implemented".to_string(),
        })
    }
}
