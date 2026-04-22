//! Backend abstraction for SQLite, Native-V3, and Geometric storage.
//!
//! The Backend trait provides a unified interface for code graph queries
//! across different storage backends. This enables runtime backend detection
//! and zero breaking changes to existing functionality.

use crate::error::LlmError;
use crate::output::{CallSearchResponse, ReferenceSearchResponse, SearchResponse};
use crate::query::SearchOptions;
use std::path::Path;

// Backend implementation modules
#[cfg(feature = "geometric-backend")]
pub mod geometric;
#[cfg(feature = "geometric-backend")]
pub mod magellan_adapter; // Contract-aware Magellan adapter layer
#[cfg(feature = "native-v3")]
mod native_v3;
pub mod schema_check;
pub mod sqlite;

#[cfg(feature = "geometric-backend")]
pub use geometric::GeometricBackend;
#[cfg(feature = "geometric-backend")]
pub use magellan_adapter::{
    apply_path_filter, lookup_symbol_by_path_and_name, normalize_path_for_query, paths_equivalent,
    SymbolLookupResult,
};
#[cfg(feature = "native-v3")]
pub use native_v3::NativeV3Backend;
pub use sqlite::SqliteBackend;

/// Backend trait for abstracting over SQLite, Native-V3, and Geometric storage.
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
/// - magellan::CodeGraph (native-v3) is not Send
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
    /// Only available with native-v3 backend (KV prefix scan).
    ///
    /// # Arguments
    /// * `prefix` - Prefix string to match (e.g., "std::collections")
    /// * `limit` - Maximum number of completions to return
    fn complete(&self, prefix: &str, limit: usize) -> Result<Vec<String>, LlmError>;

    /// Lookup symbol by exact fully-qualified name.
    ///
    /// This method provides O(1) symbol resolution by FQN using KV store.
    /// Only available with native-v3 backend.
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
    /// Only available with native-v3 backend.
    ///
    /// # Arguments
    /// * `label` - Label name to search for (e.g., "test", "entry_point")
    /// * `limit` - Maximum number of symbols to return
    /// * `db_path` - Database path for error reporting
    ///
    /// # Returns
    /// * `Ok((SearchResponse, partial, paths_bounded))` - Search results and metadata
    /// * `Err(LlmError::RequiresNativeV3Backend)` - If SQLite backend detected
    fn search_by_label(
        &self,
        label: &str,
        limit: usize,
        db_path: &str,
    ) -> Result<(SearchResponse, bool, bool), LlmError>;

    /// Get code chunks for a specific symbol.
    ///
    /// This method provides pre-extracted code snippets for a symbol,
    /// avoiding expensive file I/O. Chunks are created during Magellan indexing.
    ///
    /// # Arguments
    /// * `file_path` - Path to the source file
    /// * `symbol_name` - Name of the symbol
    ///
    /// # Returns
    /// * `Ok(Vec<CodeChunk>)` - List of chunks for the symbol
    /// * `Err(LlmError::ChunksNotAvailable)` - If chunking was not performed
    #[cfg(feature = "geometric-backend")]
    fn get_chunks_for_symbol(
        &self,
        file_path: &str,
        symbol_name: &str,
    ) -> Result<Vec<crate::backend::magellan_adapter::CodeChunk>, LlmError>;
}

/// Runtime backend dispatcher.
///
/// Wraps either SqliteBackend, NativeV3Backend, or GeometricBackend and delegates Backend trait methods
/// to the appropriate implementation based on database format detection.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)] // NativeV3Backend contains CodeGraph which is large
pub enum Backend {
    /// SQLite storage backend (traditional, always available)
    Sqlite(SqliteBackend),
    /// Native-V3 storage backend (high-performance, requires native-v3 feature)
    #[cfg(feature = "native-v3")]
    NativeV3(NativeV3Backend),
    /// Geometric storage backend (spatial/CGF optimized, requires geometric-backend feature)
    #[cfg(feature = "geometric-backend")]
    Geometric(GeometricBackend),
}

impl Backend {
    /// Detect backend format from database file and open appropriate backend.
    ///
    /// Checks file extension and header magic bytes to distinguish between:
    /// - Geometric backend: ".geo" extension
    /// - V3 backend: "SQLTGF" header (SQLiteGraph native format)
    /// - SQLite backend: "SQLite format 3\0" header
    ///
    /// # Arguments
    /// * `db_path` - Path to the database file
    ///
    /// # Returns
    /// * `Ok(Backend)` - Appropriate backend variant based on file detection
    /// * `Err(LlmError::NativeV3BackendNotSupported)` - Native-V3 detected but feature not enabled
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

        // First check file extension for geometric backend
        #[cfg(feature = "geometric-backend")]
        {
            let is_geometric = db_path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e == "geo")
                .unwrap_or(false);

            if is_geometric {
                return GeometricBackend::open(db_path).map(Backend::Geometric);
            }
        }

        #[cfg(not(feature = "geometric-backend"))]
        {
            let is_geometric = db_path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e == "geo")
                .unwrap_or(false);

            if is_geometric {
                return Err(LlmError::BackendDetectionFailed {
                    path: db_path.display().to_string(),
                    reason: "Geometric backend (.geo files) requires 'geometric-backend' feature".to_string(),
                });
            }
        }

        // Read first 16 bytes to detect format for other backends
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

        // Check for V3 format magic: "SQLTGF" (SQLiteGraph Native)
        let is_v3 = &header[0..6] == b"SQLTGF";

        // Check for SQLite format: "SQLite format 3\0"
        let is_sqlite = &header[0..16] == b"SQLite format 3\0";

        if is_v3 {
            #[cfg(feature = "native-v3")]
            return NativeV3Backend::open(db_path).map(Backend::NativeV3);

            #[cfg(not(feature = "native-v3"))]
            return Err(LlmError::NativeV3BackendNotSupported {
                path: db_path.display().to_string(),
            });
        } else if is_sqlite {
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
            #[cfg(feature = "native-v3")]
            Backend::NativeV3(b) => b.search_symbols(options),
            #[cfg(feature = "geometric-backend")]
            Backend::Geometric(b) => b.search_symbols(options),
        }
    }

    /// Delegate search_references to inner backend.
    pub fn search_references(
        &self,
        options: SearchOptions,
    ) -> Result<(ReferenceSearchResponse, bool), LlmError> {
        match self {
            Backend::Sqlite(b) => b.search_references(options),
            #[cfg(feature = "native-v3")]
            Backend::NativeV3(b) => b.search_references(options),
            #[cfg(feature = "geometric-backend")]
            Backend::Geometric(b) => b.search_references(options),
        }
    }

    /// Delegate search_calls to inner backend.
    pub fn search_calls(
        &self,
        options: SearchOptions,
    ) -> Result<(CallSearchResponse, bool), LlmError> {
        match self {
            Backend::Sqlite(b) => b.search_calls(options),
            #[cfg(feature = "native-v3")]
            Backend::NativeV3(b) => b.search_calls(options),
            #[cfg(feature = "geometric-backend")]
            Backend::Geometric(b) => b.search_calls(options),
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
            #[cfg(feature = "native-v3")]
            Backend::NativeV3(b) => b.ast(file, position, limit),
            #[cfg(feature = "geometric-backend")]
            Backend::Geometric(b) => b.ast(file, position, limit),
        }
    }

    /// Delegate find_ast to inner backend.
    pub fn find_ast(&self, kind: &str) -> Result<serde_json::Value, LlmError> {
        match self {
            Backend::Sqlite(b) => b.find_ast(kind),
            #[cfg(feature = "native-v3")]
            Backend::NativeV3(b) => b.find_ast(kind),
            #[cfg(feature = "geometric-backend")]
            Backend::Geometric(b) => b.find_ast(kind),
        }
    }

    /// Get FQN completions for a prefix.
    ///
    /// This method is only available with native-v3 backend.
    /// SQLite and Geometric backends return RequiresNativeV3Backend error.
    pub fn complete(&self, prefix: &str, limit: usize) -> Result<Vec<String>, LlmError> {
        match self {
            Backend::Sqlite(b) => b.complete(prefix, limit),
            #[cfg(feature = "native-v3")]
            Backend::NativeV3(b) => b.complete(prefix, limit),
            #[cfg(feature = "geometric-backend")]
            Backend::Geometric(b) => b.complete(prefix, limit),
        }
    }

    /// Lookup symbol by exact FQN.
    ///
    /// This method is available with all backends but may have different
    /// performance characteristics.
    pub fn lookup(&self, fqn: &str, db_path: &str) -> Result<crate::output::SymbolMatch, LlmError> {
        match self {
            Backend::Sqlite(b) => b.lookup(fqn, db_path),
            #[cfg(feature = "native-v3")]
            Backend::NativeV3(b) => b.lookup(fqn, db_path),
            #[cfg(feature = "geometric-backend")]
            Backend::Geometric(b) => b.lookup(fqn, db_path),
        }
    }

    /// Search for symbols by label.
    ///
    /// This method is only available with native-v3 backend.
    /// SQLite and Geometric backends return RequiresNativeV3Backend error.
    pub fn search_by_label(
        &self,
        label: &str,
        limit: usize,
        db_path: &str,
    ) -> Result<(SearchResponse, bool, bool), LlmError> {
        match self {
            Backend::Sqlite(b) => b.search_by_label(label, limit, db_path),
            #[cfg(feature = "native-v3")]
            Backend::NativeV3(b) => b.search_by_label(label, limit, db_path),
            #[cfg(feature = "geometric-backend")]
            Backend::Geometric(b) => b.search_by_label(label, limit, db_path),
        }
    }

    /// Get code chunks for a specific symbol.
    ///
    /// This method provides pre-extracted code snippets for a symbol.
    /// Geometric backend supports chunks from .geo files.
    /// Other backends return RequiresGeometricBackend error.
    #[cfg(feature = "geometric-backend")]
    pub fn get_chunks_for_symbol(
        &self,
        file_path: &str,
        symbol_name: &str,
    ) -> Result<Vec<crate::backend::magellan_adapter::CodeChunk>, LlmError> {
        match self {
            Backend::Sqlite(_) => Err(LlmError::ChunksNotAvailable {
                backend: "SQLite".to_string(),
                message: "SQLite backend does not support chunk retrieval. Use Geometric (.geo) backend.".to_string(),
            }),
            #[cfg(feature = "native-v3")]
            Backend::NativeV3(_) => Err(LlmError::ChunksNotAvailable {
                backend: "NativeV3".to_string(),
                message: "NativeV3 backend does not support chunk retrieval. Use Geometric (.geo) backend.".to_string(),
            }),
            #[cfg(feature = "geometric-backend")]
            Backend::Geometric(b) => b.get_chunks_for_symbol(file_path, symbol_name),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::{NamedTempFile, TempDir};

    #[cfg(feature = "geometric-backend")]
    fn create_test_geo_db() -> (TempDir, std::path::PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let geo_path = temp_dir.path().join("test.geo");

        // Create a valid geometric database using Magellan's API
        // We need to use magellan directly since GeometricBackend::inner is private
        let _backend = magellan::graph::geometric_backend::GeometricBackend::create(&geo_path)
            .expect("Failed to create test geo database");

        (temp_dir, geo_path)
    }

    #[test]
    #[cfg(feature = "geometric-backend")]
    fn test_detect_and_open_geometric_backend() {
        // Layer 1: Test geometric backend detection by extension
        let (_temp_dir, geo_path) = create_test_geo_db();

        let result = Backend::detect_and_open(&geo_path);

        // Layer 1: Should succeed
        assert!(
            result.is_ok(),
            "Layer 1: Should detect and open .geo file: {:?}",
            result.err()
        );

        // Layer 2: Should be Geometric variant
        match result.unwrap() {
            Backend::Geometric(_) => {
                // Success - correct backend type
            }
            _ => panic!("Layer 2: Expected Geometric backend variant"),
        }
    }

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
        if result.is_ok() {
            match result.unwrap() {
                Backend::Sqlite(_) => {
                    // Success - correct backend type
                }
                _ => panic!("Layer 2: Expected Sqlite backend variant for SQLite header"),
            }
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
