//! SQLite backend implementation.
//!
//! SqliteBackend provides the Backend trait implementation for SQLite databases.
//! This is the traditional storage backend and is always available.

use crate::error::LlmError;
use crate::output::{
    CallSearchResponse, ReferenceSearchResponse, SearchResponse,
};
use crate::query::{
    SearchOptions,
    search_symbols_impl,
    search_references_impl,
    search_calls_impl,
};
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::process::Command;

/// SQLite backend implementation.
///
/// Wraps a rusqlite Connection and implements the Backend trait.
/// Stores db_path for magellan shell-out in ast/find_ast commands.
/// The actual SQL queries will be moved from query.rs in Phase 18.
#[derive(Debug)]
pub struct SqliteBackend {
    pub(crate) conn: Connection,
    db_path: PathBuf,
}

impl SqliteBackend {
    /// Open a SQLite database backend.
    ///
    /// # Arguments
    /// * `db_path` - Path to the SQLite database file
    pub fn open(db_path: &Path) -> Result<Self, LlmError> {
        let conn = Connection::open(db_path)?;
        Ok(Self {
            conn,
            db_path: db_path.to_path_buf(),
        })
    }

    /// Get the database path for magellan shell-out commands.
    pub(crate) fn db_path(&self) -> &Path {
        &self.db_path
    }
}

impl super::BackendTrait for SqliteBackend {
    fn search_symbols(
        &self,
        options: SearchOptions,
    ) -> Result<(SearchResponse, bool, bool), LlmError> {
        search_symbols_impl(&self.conn, &self.db_path, &options)
    }

    fn search_references(
        &self,
        options: SearchOptions,
    ) -> Result<(ReferenceSearchResponse, bool), LlmError> {
        search_references_impl(&self.conn, &options)
    }

    fn search_calls(
        &self,
        options: SearchOptions,
    ) -> Result<(CallSearchResponse, bool), LlmError> {
        search_calls_impl(&self.conn, &options)
    }

    fn ast(
        &self,
        file: &Path,
        position: Option<usize>,
        limit: usize,
    ) -> Result<serde_json::Value, LlmError> {
        let db_path = self.db_path.to_str()
            .ok_or_else(|| LlmError::SearchFailed {
                reason: format!("Database path {:?} is not valid UTF-8", self.db_path),
            })?;
        let file_path = file.to_str()
            .ok_or_else(|| LlmError::SearchFailed {
                reason: format!("File path {:?} is not valid UTF-8", file),
            })?;

        let mut cmd = Command::new("magellan");
        cmd.args(["ast", "--db", db_path, "--file", file_path]);
        cmd.args(["--output", "json"]);  // Explicitly request JSON output

        if let Some(pos) = position {
            cmd.args(["--position", &pos.to_string()]);
        }

        // Note: magellan ast command doesn't support --limit flag
        // We'll apply limit on the JSON result instead

        let output = cmd.output()
            .map_err(|e| LlmError::SearchFailed {
                reason: format!("Failed to execute magellan ast command: {}", e),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(LlmError::SearchFailed {
                reason: format!("magellan ast command failed: {}", stderr),
            });
        }

        // Parse JSON and apply limit if needed
        let mut value: serde_json::Value = serde_json::from_slice(&output.stdout)
            .map_err(|e| LlmError::JsonError(e))?;

        // Apply limit to array results
        if let Some(arr) = value.as_array_mut() {
            if arr.len() > limit {
                arr.truncate(limit);
            }
        }

        Ok(value)
    }

    fn find_ast(&self, kind: &str) -> Result<serde_json::Value, LlmError> {
        let db_path = self.db_path.to_str()
            .ok_or_else(|| LlmError::SearchFailed {
                reason: format!("Database path {:?} is not valid UTF-8", self.db_path),
            })?;

        let output = Command::new("magellan")
            .args(["find-ast", "--db", db_path, "--kind", kind, "--output", "json"])
            .output()
            .map_err(|e| LlmError::SearchFailed {
                reason: format!("Failed to execute magellan find-ast command: {}", e),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(LlmError::SearchFailed {
                reason: format!("magellan find-ast command failed: {}", stderr),
            });
        }

        serde_json::from_slice(&output.stdout)
            .map_err(|e| LlmError::JsonError(e))
    }

    fn complete(&self, _prefix: &str, _limit: usize) -> Result<Vec<String>, LlmError> {
        // SQLite backend cannot efficiently do prefix scans on FQNs
        Err(LlmError::RequiresNativeV2Backend {
            command: "complete".to_string(),
            path: self.db_path.display().to_string(),
        })
    }

    fn lookup(&self, fqn: &str, db_path: &str) -> Result<crate::output::SymbolMatch, LlmError> {
        // SQLite backend cannot efficiently do exact FQN lookups
        // Extract partial name from FQN for error message
        let partial = fqn.rsplit("::").next().unwrap_or(fqn);
        Err(LlmError::RequiresNativeV2Backend {
            command: "lookup".to_string(),
            path: db_path.to_string(),
        })
    }
}
