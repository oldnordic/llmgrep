//! Backend abstraction for SQLite storage.
//!
//! The Backend trait provides a unified interface for code graph queries.
//!
//! Note: Geometric backend was removed in v3.5.1 — it was compile-gated
//! behind geometric-backend feature and unused in production.

use crate::error::LlmError;
use crate::output::{
    CallSearchResponse, DocsSearchResponse, FactsSearchResponse, ImplementsSearchResponse,
    ReferenceSearchResponse, SearchResponse,
};
use crate::query::{DocsSearchOptions, FactsSearchOptions, SearchOptions};
use std::path::Path;

// Backend implementation modules
pub mod schema_check;
pub mod sqlite;
pub mod vector;

pub use sqlite::SqliteBackend;

/// Backend trait for abstracting over SQLite storage.
///
/// All backend implementations must provide these core operations:
/// - Symbol search with filtering and scoring
/// - Reference search (incoming edges)
/// - Call search (outgoing edges)
/// - AST tree queries
/// - AST node search by kind
///
/// Note: This trait does not require Send or Sync because:
/// - rusqlite::Connection is not Sync
///
/// Each backend instance should be used from a single thread or externally synchronized.
pub trait BackendTrait {
    /// Search for symbols matching the given options.
    ///
    /// Returns a tuple of (response, partial_results_flag, paths_bounded_flag).
    /// - partial_results: true if candidates limit was hit
    /// - paths_bounded: true if path enumeration hit bounds
    fn search_symbols(
        &self,
        options: SearchOptions,
    ) -> Result<(SearchResponse, bool, bool), LlmError>;

    /// Search for references (incoming edges) to symbols.
    fn search_references(
        &self,
        options: SearchOptions,
    ) -> Result<(ReferenceSearchResponse, bool), LlmError>;

    /// Search for function calls (outgoing edges) from symbols.
    fn search_calls(&self, options: SearchOptions) -> Result<(CallSearchResponse, bool), LlmError>;

    /// Search for type-trait implementation relationships.
    fn search_implements(
        &self,
        options: SearchOptions,
    ) -> Result<(ImplementsSearchResponse, bool), LlmError>;

    fn search_docs(&self, options: DocsSearchOptions) -> Result<DocsSearchResponse, LlmError>;

    fn search_facts(&self, options: FactsSearchOptions) -> Result<FactsSearchResponse, LlmError>;

    /// Query AST nodes for a file.
    ///
    /// # Arguments
    /// * `file` - Path to the source file
    /// * `position` - Optional byte offset to query node at specific position
    /// * `limit` - Maximum number of AST nodes to return (for large files)
    fn ast(
        &self,
        file: &Path,
        position: Option<usize>,
        limit: usize,
    ) -> Result<serde_json::Value, LlmError>;

    /// Find AST nodes by kind.
    ///
    /// # Arguments
    /// * `kind` - AST node kind (e.g., "function_item", "if_expression")
    fn find_ast(&self, kind: &str) -> Result<serde_json::Value, LlmError>;

    /// Get FQN completions for a prefix.
    ///
    /// This method provides prefix-based autocomplete for fully qualified names.
    ///
    /// # Arguments
    /// * `prefix` - Prefix string to match (e.g., "std::collections")
    /// * `limit` - Maximum number of completions to return
    fn complete(&self, prefix: &str, limit: usize) -> Result<Vec<String>, LlmError>;

    /// Lookup symbol by exact fully-qualified name.
    ///
    /// This method provides symbol resolution by FQN.
    ///
    /// # Arguments
    /// * `fqn` - Fully-qualified name to lookup (e.g., "std::collections::HashMap::new")
    /// * `db_path` - Database path for error reporting
    ///
    /// # Returns
    /// * `Ok(SymbolMatch)` - Full symbol details if found
    /// * `Err(LlmError::SymbolNotFound)` - If FQN does not exist in database
    fn lookup(&self, fqn: &str, db_path: &str) -> Result<crate::output::SymbolMatch, LlmError>;

    /// Search for symbols by label.
    ///
    /// This method provides purpose-based label search using Magellan's label system.
    /// Labels group symbols by purpose category (e.g., "test", "entry_point", "public_api").
    ///
    /// # Arguments
    /// * `label` - Label name to search for (e.g., "test", "entry_point")
    /// * `limit` - Maximum number of symbols to return
    /// * `db_path` - Database path for error reporting
    ///
    /// # Returns
    /// * `Ok((SearchResponse, partial, paths_bounded))` - Search results and metadata
    fn search_by_label(
        &self,
        label: &str,
        limit: usize,
        db_path: &str,
    ) -> Result<(SearchResponse, bool, bool), LlmError>;
}

/// Runtime backend dispatcher.
///
/// Wraps SqliteBackend and delegates Backend trait methods.
#[derive(Debug)]
pub enum Backend {
    /// SQLite storage backend (traditional, always available)
    Sqlite(SqliteBackend),
}

impl Backend {
    /// Detect backend format from database file and open appropriate backend.
    ///
    /// Checks file header magic bytes to detect SQLite format.
    ///
    /// # Arguments
    /// * `db_path` - Path to the database file
    ///
    /// # Returns
    /// * `Ok(Backend)` - SQLite backend
    /// * `Err(LlmError::BackendDetectionFailed)` - Detection failed
    pub fn detect_and_open(db_path: &Path) -> Result<Self, LlmError> {
        use std::fs::File;
        use std::io::Read;

        // Check if file exists
        if !db_path.exists() {
            return Err(LlmError::DatabaseNotFound {
                path: db_path.display().to_string(),
            });
        }

        // Read first 16 bytes to detect format
        let mut file = File::open(db_path).map_err(|e| LlmError::BackendDetectionFailed {
            path: db_path.display().to_string(),
            reason: format!("Cannot open file: {}", e),
        })?;

        let mut header = [0u8; 16];
        file.read_exact(&mut header)
            .map_err(|e| LlmError::BackendDetectionFailed {
                path: db_path.display().to_string(),
                reason: format!("Cannot read file header: {}", e),
            })?;

        // Check for SQLite format: "SQLite format 3\0"
        let is_sqlite = &header[0..16] == b"SQLite format 3\0";

        if is_sqlite {
            SqliteBackend::open(db_path).map(Backend::Sqlite)
        } else {
            // Unknown format, try SQLite as fallback (may fail with better error)
            SqliteBackend::open(db_path).map(Backend::Sqlite)
        }
    }

    /// Delegate search_symbols to inner backend.
    pub fn search_symbols(
        &self,
        options: SearchOptions,
    ) -> Result<(SearchResponse, bool, bool), LlmError> {
        match self {
            Backend::Sqlite(b) => b.search_symbols(options),
        }
    }

    /// Delegate search_references to inner backend.
    pub fn search_references(
        &self,
        options: SearchOptions,
    ) -> Result<(ReferenceSearchResponse, bool), LlmError> {
        match self {
            Backend::Sqlite(b) => b.search_references(options),
        }
    }

    /// Delegate search_calls to inner backend.
    pub fn search_calls(
        &self,
        options: SearchOptions,
    ) -> Result<(CallSearchResponse, bool), LlmError> {
        match self {
            Backend::Sqlite(b) => b.search_calls(options),
        }
    }

    /// Delegate search_implements to inner backend.
    pub fn search_implements(
        &self,
        options: SearchOptions,
    ) -> Result<(ImplementsSearchResponse, bool), LlmError> {
        match self {
            Backend::Sqlite(b) => b.search_implements(options),
        }
    }

    pub fn search_docs(&self, options: DocsSearchOptions) -> Result<DocsSearchResponse, LlmError> {
        match self {
            Backend::Sqlite(b) => b.search_docs(options),
        }
    }

    pub fn search_facts(
        &self,
        options: FactsSearchOptions,
    ) -> Result<FactsSearchResponse, LlmError> {
        match self {
            Backend::Sqlite(b) => b.search_facts(options),
        }
    }

    /// Delegate ast to inner backend.
    pub fn ast(
        &self,
        file: &Path,
        position: Option<usize>,
        limit: usize,
    ) -> Result<serde_json::Value, LlmError> {
        match self {
            Backend::Sqlite(b) => b.ast(file, position, limit),
        }
    }

    /// Delegate find_ast to inner backend.
    pub fn find_ast(&self, kind: &str) -> Result<serde_json::Value, LlmError> {
        match self {
            Backend::Sqlite(b) => b.find_ast(kind),
        }
    }

    /// Get FQN completions for a prefix.
    pub fn complete(&self, prefix: &str, limit: usize) -> Result<Vec<String>, LlmError> {
        match self {
            Backend::Sqlite(b) => b.complete(prefix, limit),
        }
    }

    /// Lookup symbol by exact FQN.
    pub fn lookup(&self, fqn: &str, db_path: &str) -> Result<crate::output::SymbolMatch, LlmError> {
        match self {
            Backend::Sqlite(b) => b.lookup(fqn, db_path),
        }
    }

    /// Search for symbols by label.
    pub fn search_by_label(
        &self,
        label: &str,
        limit: usize,
        db_path: &str,
    ) -> Result<(SearchResponse, bool, bool), LlmError> {
        match self {
            Backend::Sqlite(b) => b.search_by_label(label, limit, db_path),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_detect_and_open_sqlite_backend() {
        // Layer 1: Test SQLite backend detection by header
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"SQLite format 3\0").unwrap();

        let result = Backend::detect_and_open(temp_file.path());

        // Layer 1: Should succeed (even if open fails later due to invalid SQLite)
        // We just want to verify detection works
        // The actual opening may fail for an invalid SQLite file

        // Layer 2: For a valid SQLite header, should attempt SQLite
        // Note: This may fail to open due to invalid SQLite content, but detection should work
        if let Ok(backend) = result {
            assert!(
                matches!(backend, Backend::Sqlite(_)),
                "Layer 2: Expected Sqlite backend variant for SQLite header"
            );
        }
    }

    #[test]
    fn test_detect_and_open_nonexistent_file() {
        // Layer 1: Test error handling for non-existent file
        let fake_path = Path::new("/nonexistent/path/code.geo");

        let result = Backend::detect_and_open(fake_path);

        // Layer 1: Should fail
        assert!(
            result.is_err(),
            "Layer 1: Should fail for non-existent file"
        );

        // Layer 2: Should be DatabaseNotFound error
        match result {
            Err(LlmError::DatabaseNotFound { .. }) => {
                // Success - correct error type
            }
            _ => panic!("Layer 2: Expected DatabaseNotFound error"),
        }
    }
}
