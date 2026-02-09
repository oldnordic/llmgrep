//! Backend abstraction for SQLite and Native-V2 storage.
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

/// Backend trait for abstracting over SQLite and Native-V2 storage.
///
/// All backend implementations must provide these core operations:
/// - Symbol search with filtering and scoring
/// - Reference search (incoming edges)
/// - Call search (outgoing edges)
/// - AST tree queries
/// - AST node search by kind
pub trait Backend: Send + Sync {
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
}
