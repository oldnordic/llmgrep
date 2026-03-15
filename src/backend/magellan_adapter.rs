//! Magellan backend adapter with contract-aware integration
//!
//! This module provides a small adapter layer for interacting with Magellan's
//! .geo backend according to the updated contract (v3.0.2+). It handles:
//! - Path normalization before queries
//! - Explicit ambiguity error handling
//! - Code chunk retrieval
//! - Backend-neutral operations

use magellan::validation::normalize_path;
use std::path::Path;

/// Result type for symbol lookup that may have ambiguity
#[derive(Debug, Clone)]
pub enum SymbolLookupResult {
    /// Exactly one symbol found
    Unique(magellan::graph::geometric_backend::SymbolInfo),
    /// Multiple symbols match - explicit ambiguity with candidates
    Ambiguous {
        path: String,
        name: String,
        candidates: Vec<magellan::graph::geometric_backend::SymbolInfo>,
    },
    /// No symbols found
    NotFound,
}

/// Result type for search operations that may have ambiguity
///
/// This type provides explicit ambiguity information for search results,
/// allowing callers to distinguish between unique matches, ambiguous matches,
/// and not-found cases.
#[derive(Debug, Clone)]
pub enum SearchResult {
    /// Exactly one symbol match
    Unique(magellan::graph::geometric_backend::SymbolInfo),
    /// Multiple symbols match with ambiguity details
    Ambiguous {
        query: String,
        candidates: Vec<magellan::graph::geometric_backend::SymbolInfo>,
        /// Path filter that was applied (if any)
        path_filter: Option<String>,
    },
    /// No symbols found
    NotFound {
        query: String,
        path_filter: Option<String>,
    },
}

impl SearchResult {
    /// Convert from a vector of symbol info
    pub fn from_symbols(
        symbols: Vec<magellan::graph::geometric_backend::SymbolInfo>,
        query: String,
        path_filter: Option<String>,
    ) -> Self {
        if symbols.is_empty() {
            SearchResult::NotFound { query, path_filter }
        } else if symbols.len() == 1 {
            SearchResult::Unique(symbols.into_iter().next().unwrap())
        } else {
            SearchResult::Ambiguous {
                query,
                candidates: symbols,
                path_filter,
            }
        }
    }

    /// Get all symbols as a vec, regardless of ambiguity
    pub fn into_symbols(self) -> Vec<magellan::graph::geometric_backend::SymbolInfo> {
        match self {
            SearchResult::Unique(sym) => vec![sym],
            SearchResult::Ambiguous { candidates, .. } => candidates,
            SearchResult::NotFound { .. } => vec![],
        }
    }

    /// Check if the result has any symbols
    pub fn has_symbols(&self) -> bool {
        !matches!(self, SearchResult::NotFound { .. })
    }

    /// Get the count of symbols
    pub fn count(&self) -> usize {
        match self {
            SearchResult::Unique(_) => 1,
            SearchResult::Ambiguous { candidates, .. } => candidates.len(),
            SearchResult::NotFound { .. } => 0,
        }
    }
}

/// Code chunk with content from .geo backend
///
/// Represents a pre-extracted code snippet that provides fast access
/// to source content without file I/O.
#[derive(Debug, Clone)]
pub struct CodeChunk {
    /// Unique identifier
    pub id: Option<i64>,
    /// File path containing this chunk
    pub file_path: String,
    /// Byte offset where chunk starts
    pub byte_start: usize,
    /// Byte offset where chunk ends
    pub byte_end: usize,
    /// Source code content
    pub content: String,
    /// Name of symbol this chunk belongs to (if available)
    pub symbol_name: Option<String>,
    /// Kind of symbol (e.g., "function_item", "struct_item")
    pub symbol_kind: Option<String>,
}

/// Result type for chunk retrieval
#[derive(Debug, Clone)]
pub enum ChunkLookupResult {
    /// Chunks found
    Found(Vec<CodeChunk>),
    /// No chunks available (not an error - chunking may not have been run)
    NotAvailable,
    /// Error occurred during lookup
    Error(String),
}

impl From<magellan::generation::schema::CodeChunk> for CodeChunk {
    fn from(chunk: magellan::generation::schema::CodeChunk) -> Self {
        CodeChunk {
            id: chunk.id,
            file_path: chunk.file_path,
            byte_start: chunk.byte_start,
            byte_end: chunk.byte_end,
            content: chunk.content,
            symbol_name: chunk.symbol_name,
            symbol_kind: chunk.symbol_kind,
        }
    }
}

/// Normalize a file path for Mag queries
///
/// This helper ensures consistent path handling across all llmgrep queries.
/// Paths are normalized before being passed to Magellan, ensuring that:
/// - `./src/x.rs` and `src/x.rs` resolve consistently
/// - `src\\x.rs` converts to `src/x.rs`
/// - Redundant separators are removed
///
/// # Arguments
/// * `path` - File path to normalize
///
/// # Returns
/// Normalized path string, or original if normalization fails
pub fn normalize_path_for_query(path: &str) -> String {
    use std::path::Path;
    // Pre-process to handle double slashes and backslashes
    let preprocessed = path.replace("//", "/").replace('\\', "/");
    match normalize_path(Path::new(&preprocessed)) {
        Ok(normalized) => normalized,
        Err(_) => {
            // Fallback to preprocessed path if normalization fails
            // This preserves functionality for edge cases
            preprocessed
        }
    }
}

/// Check if two paths refer to the same file using normalized comparison
///
/// # Arguments
/// * `path1` - First path
/// * `path2` - Second path
///
/// # Returns
/// true if paths refer to the same file
pub fn paths_equivalent(path1: &str, path2: &str) -> bool {
    let norm1 = normalize_path_for_query(path1);
    let norm2 = normalize_path_for_query(path2);
    norm1 == norm2
}

/// Wrapper for symbol lookup with path normalization
///
/// This function handles the common pattern of looking up symbols by path and name,
/// with proper path normalization and ambiguity handling.
///
/// # Arguments
/// * `backend` - The Geometric backend instance
/// * `path` - File path (will be normalized)
/// * `name` - Symbol name
///
/// # Returns
/// SymbolLookupResult indicating unique match, ambiguity, or not found
pub fn lookup_symbol_by_path_and_name(
    backend: &magellan::graph::geometric_backend::GeometricBackend,
    path: &str,
    name: &str,
) -> SymbolLookupResult {
    // Normalize path first
    let normalized_path = normalize_path_for_query(path);

    // Use find_symbol_id_by_name_and_path that returns Option<u64>
    // Returns Some(id) if exactly one match found, None if no match or ambiguous
    match backend.find_symbol_id_by_name_and_path(name, &normalized_path) {
        Some(id) => {
            // Unique match found - get the symbol info
            match backend.find_symbol_by_id_info(id) {
                Some(info) => SymbolLookupResult::Unique(info),
                None => SymbolLookupResult::NotFound,
            }
        }
        None => {
            // Not found or ambiguous - check if there are multiple candidates
            let all_symbols = backend
                .symbols_in_file(&normalized_path)
                .unwrap_or_default();
            let matching: Vec<_> = all_symbols.into_iter().filter(|s| s.name == name).collect();

            if matching.len() > 1 {
                // Truly ambiguous
                SymbolLookupResult::Ambiguous {
                    path: normalized_path,
                    name: name.to_string(),
                    candidates: matching,
                }
            } else {
                SymbolLookupResult::NotFound
            }
        }
    }
}

/// Apply path filter to symbols with normalization
///
/// This helper ensures that path filtering works consistently regardless of
/// path variations (relative vs absolute, separators, etc.).
///
/// # Arguments
/// * `symbols` - Symbols to filter
/// * `path_filter` - Path filter to apply
///
/// # Returns
/// Filtered symbols
pub fn apply_path_filter(
    symbols: Vec<magellan::graph::geometric_backend::SymbolInfo>,
    path_filter: &Path,
) -> Vec<magellan::graph::geometric_backend::SymbolInfo> {
    let normalized_filter = normalize_path_for_query(path_filter.to_str().unwrap_or(""));
    symbols
        .into_iter()
        .filter(|info| {
            let normalized_symbol_path = normalize_path_for_query(&info.file_path);
            normalized_symbol_path.contains(&normalized_filter)
                || normalized_filter.contains(&normalized_symbol_path)
        })
        .collect()
}

/// Get code chunks for a specific symbol
///
/// This is the preferred way to retrieve code content for a symbol.
/// It uses the pre-extracted chunks stored during Magellan indexing,
/// avoiding expensive file I/O.
///
/// # Arguments
/// * `backend` - The Geometric backend instance
/// * `file_path` - Path to the source file (will be normalized)
/// * `symbol_name` - Name of the symbol
///
/// # Returns
/// ChunkLookupResult indicating success with chunks, unavailability, or error
pub fn get_chunks_for_symbol(
    backend: &magellan::graph::geometric_backend::GeometricBackend,
    file_path: &str,
    symbol_name: &str,
) -> ChunkLookupResult {
    let normalized_path = normalize_path_for_query(file_path);

    match backend.get_code_chunks_for_symbol(&normalized_path, symbol_name) {
        Ok(chunks) => {
            if chunks.is_empty() {
                ChunkLookupResult::NotAvailable
            } else {
                let converted: Vec<CodeChunk> = chunks.into_iter().map(|c| c.into()).collect();
                ChunkLookupResult::Found(converted)
            }
        }
        Err(e) => ChunkLookupResult::Error(format!("Failed to get chunks: {}", e)),
    }
}

/// Get all code chunks for a file
///
/// Returns all chunks associated with a file, ordered by byte_start.
///
/// # Arguments
/// * `backend` - The Geometric backend instance
/// * `file_path` - Path to the source file (will be normalized)
///
/// # Returns
/// ChunkLookupResult indicating success with chunks, unavailability, or error
pub fn get_chunks_for_file(
    backend: &magellan::graph::geometric_backend::GeometricBackend,
    file_path: &str,
) -> ChunkLookupResult {
    let normalized_path = normalize_path_for_query(file_path);

    match backend.get_code_chunks(&normalized_path) {
        Ok(chunks) => {
            if chunks.is_empty() {
                ChunkLookupResult::NotAvailable
            } else {
                let converted: Vec<CodeChunk> = chunks.into_iter().map(|c| c.into()).collect();
                ChunkLookupResult::Found(converted)
            }
        }
        Err(e) => ChunkLookupResult::Error(format!("Failed to get chunks: {}", e)),
    }
}

/// Get a code chunk by exact byte span
///
/// Returns the chunk at the exact file path and byte range if it exists.
///
/// # Arguments
/// * `backend` - The Geometric backend instance
/// * `file_path` - Path to the source file (will be normalized)
/// * `byte_start` - Starting byte offset
/// * `byte_end` - Ending byte offset
///
/// # Returns
/// ChunkLookupResult indicating success with chunk, unavailability, or error
pub fn get_chunk_by_span(
    backend: &magellan::graph::geometric_backend::GeometricBackend,
    file_path: &str,
    byte_start: usize,
    byte_end: usize,
) -> ChunkLookupResult {
    let normalized_path = normalize_path_for_query(file_path);

    match backend.get_code_chunk_by_span(&normalized_path, byte_start, byte_end) {
        Ok(Some(chunk)) => ChunkLookupResult::Found(vec![chunk.into()]),
        Ok(None) => ChunkLookupResult::NotAvailable,
        Err(e) => ChunkLookupResult::Error(format!("Failed to get chunk: {}", e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_path_for_query() {
        // Test that ./ prefix is handled (may canonicalize to absolute path if file exists)
        let result = normalize_path_for_query("./src/lib.rs");
        // If src/lib.rs exists, it will be canonicalized to absolute path
        // Otherwise, ./ prefix is stripped
        assert!(result.ends_with("src/lib.rs") || result == "src/lib.rs");

        // Test that double slashes are removed (non-existent path won't canonicalize)
        let result = normalize_path_for_query("nonexistent//lib.rs");
        assert!(!result.contains("//"));

        // Test that backslash is converted
        let result = normalize_path_for_query("nonexistent\\lib.rs");
        assert!(!result.contains("\\"));
    }

    #[test]
    fn test_paths_equivalent() {
        // Use non-existent paths to avoid canonicalization to absolute paths
        assert!(paths_equivalent(
            "./nonexistent/lib.rs",
            "nonexistent/lib.rs"
        ));
        assert!(paths_equivalent(
            "nonexistent//lib.rs",
            "nonexistent/lib.rs"
        ));
    }

    #[test]
    fn test_normalize_path_fallback() {
        // Invalid paths should still return something (the original)
        let result = normalize_path_for_query("");
        // Either normalization succeeds or returns original
        assert!(result.is_empty() || result == "");
    }
}
