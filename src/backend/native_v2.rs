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

impl NativeV2Backend {
    /// Open a Native-V2 database backend.
    ///
    /// # Arguments
    /// * `db_path` - Path to the Native-V2 database file
    pub fn open(db_path: &Path) -> Result<Self, LlmError> {
        let graph = CodeGraph::open(db_path)?;
        Ok(Self { graph })
    }
}

impl super::Backend for NativeV2Backend {
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
        _file: &Path,
        _position: Option<usize>,
        _limit: usize,
    ) -> Result<serde_json::Value, LlmError> {
        // TODO: Implement in Phase 19 using CodeGraph API
        Err(LlmError::SearchFailed {
            reason: "NativeV2Backend::ast not yet implemented".to_string(),
        })
    }

    fn find_ast(&self, _kind: &str) -> Result<serde_json::Value, LlmError> {
        // TODO: Implement in Phase 19 using CodeGraph API
        Err(LlmError::SearchFailed {
            reason: "NativeV2Backend::find_ast not yet implemented".to_string(),
        })
    }
}
