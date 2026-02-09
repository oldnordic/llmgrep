//! Native-V2 backend implementation.
//!
//! NativeV2Backend provides the Backend trait implementation for Native-V2 databases.
//! This backend requires the native-v2 feature flag and uses the CodeGraph API.

#![cfg(feature = "native-v2")]

use crate::error::LlmError;
use crate::output::{
    CallSearchResponse, ReferenceSearchResponse, SearchResponse,
};
use crate::query::SearchOptions;
use magellan::CodeGraph;
use std::path::Path;

/// Native-V2 backend implementation.
///
/// Wraps a CodeGraph and implements the Backend trait.
/// This backend is only available when the native-v2 feature is enabled.
pub struct NativeV2Backend {
    #[allow(dead_code)]
    pub(crate) graph: CodeGraph,
}

impl std::fmt::Debug for NativeV2Backend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeV2Backend")
            .finish_non_exhaustive()
    }
}

impl NativeV2Backend {
    /// Open a Native-V2 database backend.
    ///
    /// # Arguments
    /// * `db_path` - Path to the Native-V2 database file
    pub fn open(db_path: &Path) -> Result<Self, LlmError> {
        let graph = CodeGraph::open(db_path).map_err(|e| LlmError::DatabaseCorrupted {
            reason: format!("Failed to open Native-V2 database: {}", e),
        })?;
        Ok(Self { graph })
    }
}

impl super::BackendTrait for NativeV2Backend {
    fn search_symbols(
        &self,
        _options: SearchOptions,
    ) -> Result<(SearchResponse, bool, bool), LlmError> {
        // TODO: Implement in Phase 19 using CodeGraph API
        Err(LlmError::SearchFailed {
            reason: "NativeV2Backend::search_symbols not yet implemented".to_string(),
        })
    }

    fn search_references(
        &self,
        _options: SearchOptions,
    ) -> Result<(ReferenceSearchResponse, bool), LlmError> {
        // TODO: Implement in Phase 19
        Err(LlmError::SearchFailed {
            reason: "NativeV2Backend::search_references not yet implemented".to_string(),
        })
    }

    fn search_calls(
        &self,
        _options: SearchOptions,
    ) -> Result<(CallSearchResponse, bool), LlmError> {
        // TODO: Implement in Phase 19
        Err(LlmError::SearchFailed {
            reason: "NativeV2Backend::search_calls not yet implemented".to_string(),
        })
    }

    fn ast(
        &self,
        file: &Path,
        position: Option<usize>,
        limit: usize,
    ) -> Result<serde_json::Value, LlmError> {
        // Convert Path to str for CodeGraph API
        let file_path = file.to_str()
            .ok_or_else(|| LlmError::SearchFailed {
                reason: format!("File path {:?} is not valid UTF-8", file),
            })?;

        // Delegate to CodeGraph API for AST nodes
        let nodes = self.graph.get_ast_nodes_by_file(file_path)
            .map_err(|e| LlmError::SearchFailed {
                reason: format!("Failed to get AST nodes for {}: {}", file_path, e),
            })?;

        // Apply position filter if specified
        let filtered: Vec<_> = if let Some(pos) = position {
            nodes.into_iter()
                .filter(|n| n.node.byte_start <= pos && pos < n.node.byte_end)
                .take(limit)
                .collect()
        } else {
            nodes.into_iter().take(limit).collect()
        };

        // Return as JSON
        serde_json::to_value(filtered).map_err(LlmError::JsonError)
    }

    fn find_ast(&self, kind: &str) -> Result<serde_json::Value, LlmError> {
        // Delegate to CodeGraph::get_ast_nodes_by_kind()
        let nodes = self.graph.get_ast_nodes_by_kind(kind)
            .map_err(|e| LlmError::SearchFailed {
                reason: format!("Failed to get AST nodes by kind: {}", e),
            })?;

        // Return JSON-serializable results
        serde_json::to_value(nodes).map_err(LlmError::JsonError)
    }
}
