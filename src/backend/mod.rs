//! Backend abstraction for SQLite and Native-V3 storage.
//!
//! The Backend trait provides a unified interface for code graph queries
//! across different storage backends. This enables runtime backend detection
//! and zero breaking changes to existing functionality.

use crate::error::LlmError;
use crate::output::{
    CallSearchResponse, ReferenceSearchResponse, SearchResponse,
};
use crate::query::SearchOptions;
use std::path::Path;

// Backend implementation modules
pub mod sqlite;
#[cfg(feature = "native-v3")]
mod native_v3;

pub use sqlite::SqliteBackend;
#[cfg(feature = "native-v3")]
pub use native_v3::NativeV3Backend;

/// Backend trait for abstracting over SQLite and Native-V3 storage.
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
    /// This method provides purpose-based semantic search using Magellan's label system.
    /// Labels group symbols by semantic category (e.g., "test", "entry_point", "public_api").
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
}

/// Runtime backend dispatcher.
///
/// Wraps either SqliteBackend or NativeV3Backend and delegates Backend trait methods
/// to the appropriate implementation based on database format detection.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)] // NativeV3Backend contains CodeGraph which is large
pub enum Backend {
    /// SQLite storage backend (traditional, always available)
    Sqlite(SqliteBackend),
    /// Native-V3 storage backend (high-performance, requires native-v3 feature)
    #[cfg(feature = "native-v3")]
    NativeV3(NativeV3Backend),
}

impl Backend {
    /// Detect backend format from database file and open appropriate backend.
    ///
    /// # Arguments
    /// * `db_path` - Path to the database file
    ///
    /// # Returns
    /// * `Ok(Backend)` - Appropriate backend variant based on file header detection
    /// * `Err(LlmError::NativeV3BackendNotSupported)` - Native-V3 detected but feature not enabled
    /// * `Err(LlmError::BackendDetectionFailed)` - Detection failed
    pub fn detect_and_open(db_path: &Path) -> Result<Self, LlmError> {
        // Check file extension to determine backend type
        let path_str = db_path.to_string_lossy();
        
        #[cfg(feature = "native-v3")]
        if path_str.ends_with(".v3") {
            return NativeV3Backend::open(db_path).map(Backend::NativeV3);
        }
        
        // Default to SQLite backend for .db files or unknown extensions
        SqliteBackend::open(db_path).map(Backend::Sqlite)
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
        }
    }

    /// Delegate find_ast to inner backend.
    pub fn find_ast(&self, kind: &str) -> Result<serde_json::Value, LlmError> {
        match self {
            Backend::Sqlite(b) => b.find_ast(kind),
            #[cfg(feature = "native-v3")]
            Backend::NativeV3(b) => b.find_ast(kind),
        }
    }

    /// Get FQN completions for a prefix.
    ///
    /// This method is only available with native-v3 backend.
    /// SQLite backend returns RequiresNativeV3Backend error.
    pub fn complete(&self, prefix: &str, limit: usize) -> Result<Vec<String>, LlmError> {
        match self {
            Backend::Sqlite(b) => b.complete(prefix, limit),
            #[cfg(feature = "native-v3")]
            Backend::NativeV3(b) => b.complete(prefix, limit),
        }
    }

    /// Lookup symbol by exact FQN.
    ///
    /// This method is only available with native-v3 backend.
    /// SQLite backend returns RequiresNativeV3Backend error.
    pub fn lookup(&self, fqn: &str, db_path: &str) -> Result<crate::output::SymbolMatch, LlmError> {
        match self {
            Backend::Sqlite(b) => b.lookup(fqn, db_path),
            #[cfg(feature = "native-v3")]
            Backend::NativeV3(b) => b.lookup(fqn, db_path),
        }
    }

    /// Search for symbols by label.
    ///
    /// This method is only available with native-v3 backend.
    /// SQLite backend returns RequiresNativeV3Backend error.
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
        }
    }
}
