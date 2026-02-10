//! Backend abstraction for SQLite and Native-V2 storage.
//!
//! The Backend trait provides a unified interface for code graph queries
//! across different storage backends. This enables runtime backend detection
//! and zero breaking changes to existing functionality.

use magellan::migrate_backend_cmd::{detect_backend_format, BackendFormat as MagellanBackendFormat};

use crate::error::LlmError;
use crate::output::{
    CallSearchResponse, ReferenceSearchResponse, SearchResponse,
};
use crate::query::SearchOptions;
use std::path::Path;

// Backend implementation modules
mod sqlite;
mod native_v2;

use sqlite::SqliteBackend;
#[cfg(feature = "native-v2")]
use native_v2::NativeV2Backend;

/// Backend trait for abstracting over SQLite and Native-V2 storage.
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
/// - magellan::CodeGraph (native-v2) is not Send
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
    fn search_calls(
        &self,
        options: SearchOptions,
    ) -> Result<(CallSearchResponse, bool), LlmError>;

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
    /// Only available with native-v2 backend (KV prefix scan).
    ///
    /// # Arguments
    /// * `prefix` - Prefix string to match (e.g., "std::collections")
    /// * `limit` - Maximum number of completions to return
    fn complete(&self, prefix: &str, limit: usize) -> Result<Vec<String>, LlmError>;
}

/// Runtime backend dispatcher.
///
/// Wraps either SqliteBackend or NativeV2Backend and delegates Backend trait methods
/// to the appropriate implementation based on database format detection.
#[derive(Debug)]
pub enum Backend {
    /// SQLite storage backend (traditional, always available)
    Sqlite(SqliteBackend),
    /// Native-V2 storage backend (high-performance, requires native-v2 feature)
    #[cfg(feature = "native-v2")]
    NativeV2(NativeV2Backend),
}

impl Backend {
    /// Detect backend format from database file and open appropriate backend.
    ///
    /// # Arguments
    /// * `db_path` - Path to the database file
    ///
    /// # Returns
    /// * `Ok(Backend)` - Appropriate backend variant based on file header detection
    /// * `Err(LlmError::NativeV2BackendNotSupported)` - Native-V2 detected but feature not enabled
    /// * `Err(LlmError::BackendDetectionFailed)` - Detection failed
    pub fn detect_and_open(db_path: &Path) -> Result<Self, LlmError> {
        detect_backend_format(db_path)
            .map_err(|e| LlmError::BackendDetectionFailed {
                path: db_path.display().to_string(),
                reason: e.to_string(),
            })
            .and_then(|format| match format {
                MagellanBackendFormat::Sqlite => {
                    SqliteBackend::open(db_path).map(Backend::Sqlite)
                }
                #[cfg(feature = "native-v2")]
                MagellanBackendFormat::NativeV2 => {
                    NativeV2Backend::open(db_path).map(Backend::NativeV2)
                }
                #[cfg(not(feature = "native-v2"))]
                MagellanBackendFormat::NativeV2 => {
                    Err(LlmError::NativeV2BackendNotSupported {
                        path: db_path.display().to_string(),
                    })
                }
            })
    }

    /// Delegate search_symbols to inner backend.
    pub fn search_symbols(
        &self,
        options: SearchOptions,
    ) -> Result<(SearchResponse, bool, bool), LlmError> {
        match self {
            Backend::Sqlite(b) => b.search_symbols(options),
            #[cfg(feature = "native-v2")]
            Backend::NativeV2(b) => b.search_symbols(options),
        }
    }

    /// Delegate search_references to inner backend.
    pub fn search_references(
        &self,
        options: SearchOptions,
    ) -> Result<(ReferenceSearchResponse, bool), LlmError> {
        match self {
            Backend::Sqlite(b) => b.search_references(options),
            #[cfg(feature = "native-v2")]
            Backend::NativeV2(b) => b.search_references(options),
        }
    }

    /// Delegate search_calls to inner backend.
    pub fn search_calls(
        &self,
        options: SearchOptions,
    ) -> Result<(CallSearchResponse, bool), LlmError> {
        match self {
            Backend::Sqlite(b) => b.search_calls(options),
            #[cfg(feature = "native-v2")]
            Backend::NativeV2(b) => b.search_calls(options),
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
            #[cfg(feature = "native-v2")]
            Backend::NativeV2(b) => b.ast(file, position, limit),
        }
    }

    /// Delegate find_ast to inner backend.
    pub fn find_ast(&self, kind: &str) -> Result<serde_json::Value, LlmError> {
        match self {
            Backend::Sqlite(b) => b.find_ast(kind),
            #[cfg(feature = "native-v2")]
            Backend::NativeV2(b) => b.find_ast(kind),
        }
    }

    /// Get FQN completions for a prefix.
    ///
    /// This method is only available with native-v2 backend.
    /// SQLite backend returns RequiresNativeV2Backend error.
    pub fn complete(&self, prefix: &str, limit: usize) -> Result<Vec<String>, LlmError> {
        match self {
            Backend::Sqlite(b) => b.complete(prefix, limit),
            #[cfg(feature = "native-v2")]
            Backend::NativeV2(b) => b.complete(prefix, limit),
        }
    }
}
