//! Native-V2 backend implementation.
//!
//! NativeV2Backend provides the Backend trait implementation for Native-V2 databases.
//! This backend requires the native-v2 feature flag and uses the CodeGraph API.

#![cfg(feature = "native-v2")]

use crate::error::LlmError;
use crate::infer_language;
use crate::output::{
    CallSearchResponse, CallMatch, ReferenceSearchResponse, ReferenceMatch,
    SearchResponse, SymbolMatch, Span, SpanContext,
};
use crate::query::SearchOptions;
use magellan::common::extract_symbol_content_safe;
use magellan::graph::{query, SymbolNode};
use magellan::graph::metrics::schema::SymbolMetrics;
use magellan::CodeGraph;
use regex::Regex;
use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use sqlitegraph::SnapshotId;
use sqlitegraph::backend::KvValue;

/// File cache for context and snippet extraction
///
/// Caches file bytes and parsed lines to avoid re-reading files.
struct FileCache {
    bytes: Vec<u8>,
    lines: Vec<String>,
}

/// Load a file into cache, returning cached version if already loaded
fn load_file<'a>(path: &str, cache: &'a mut HashMap<String, FileCache>) -> Option<&'a FileCache> {
    if !cache.contains_key(path) {
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(_) => return None,
        };
        // Parse lines for context extraction
        let lines = String::from_utf8_lossy(&bytes)
            .lines()
            .map(|s| s.to_string())
            .collect();
        cache.insert(path.to_string(), FileCache { bytes, lines });
    }
    cache.get(path)
}

/// Extract snippet from file at specified byte range
///
/// # Arguments
/// * `file_path` - Path to source file
/// * `byte_start` - Start byte offset
/// * `byte_end` - End byte offset
/// * `max_bytes` - Maximum bytes to extract
/// * `cache` - File cache for performance
///
/// # Returns
/// * (snippet_content, truncated_flag) or (None, None) on failure
fn extract_snippet(
    file_path: &str,
    byte_start: u64,
    byte_end: u64,
    max_bytes: usize,
    cache: &mut HashMap<String, FileCache>,
) -> (Option<String>, Option<bool>) {
    if max_bytes == 0 {
        return (None, None);
    }
    let file = match load_file(file_path, cache) {
        Some(f) => f,
        None => return (None, None),
    };

    let start = byte_start as usize;
    let end = byte_end as usize;

    if start >= file.bytes.len() || end > file.bytes.len() || start >= end {
        return (None, None);
    }

    let capped_end = end.min(start + max_bytes);
    let truncated = capped_end < end;

    // Use safe UTF-8 extraction to handle multi-byte characters
    let snippet = match extract_symbol_content_safe(&file.bytes, start, capped_end) {
        Some(s) => s,
        None => {
            // Fallback to from_utf8_lossy if safe extraction fails
            String::from_utf8_lossy(&file.bytes[start..capped_end]).to_string()
        }
    };

    (Some(snippet), Some(truncated))
}

/// Extract context lines from file around a given line range
///
/// # Arguments
/// * `file_path` - Path to source file
/// * `start_line` - Start line number (1-based)
/// * `end_line` - End line number (1-based)
/// * `context_lines` - Number of context lines before and after
/// * `capped` - Whether the result was artificially capped
/// * `cache` - File cache for performance
///
/// # Returns
/// * SpanContext with before/selected/after lines, or None on failure
fn span_context_from_file(
    file_path: &str,
    start_line: u64,
    end_line: u64,
    context_lines: usize,
    capped: bool,
    cache: &mut HashMap<String, FileCache>,
) -> Option<SpanContext> {
    let file = load_file(file_path, cache)?;
    let line_count = file.lines.len() as u64;
    if line_count == 0 {
        return None;
    }
    let start_line = start_line.max(1).min(line_count);
    let end_line = end_line.max(start_line).min(line_count);
    let before_start = start_line.saturating_sub(context_lines as u64).max(1);
    let after_end = (end_line + context_lines as u64).min(line_count);

    let before = file.lines[(before_start - 1) as usize..(start_line - 1) as usize].to_vec();
    let selected = file.lines[(start_line - 1) as usize..end_line as usize].to_vec();
    let after = file.lines[end_line as usize..after_end as usize].to_vec();
    let truncated = capped
        || (context_lines > 0 && (before.len() < context_lines || after.len() < context_lines));

    Some(SpanContext {
        before,
        selected,
        after,
        truncated,
    })
}

/// Native-V2 backend implementation.
///
/// Wraps a CodeGraph and implements the Backend trait.
/// This backend is only available when the native-v2 feature is enabled.
///
/// Note: Native-v2 databases use sqlitegraph's GraphFile format, not standard SQLite.
/// All queries must go through the CodeGraph API.
///
/// # Safety
/// Uses UnsafeCell for interior mutability because CodeGraph methods require `&mut self`
/// but BackendTrait takes `&self`. This is safe because we own the CodeGraph exclusively
/// and only use one mutable reference at a time within each method.
pub struct NativeV2Backend {
    /// CodeGraph stored in UnsafeCell for interior mutability
    graph: UnsafeCell<CodeGraph>,
    /// Database path for error messages and direct SQL access
    #[allow(dead_code)] // Used for SQL queries but not directly read in current code
    db_path: PathBuf,
}

// # Safety
//
// NativeV2Backend owns the CodeGraph exclusively and only provides one mutable
// reference at a time within each method. We use UnsafeCell for interior mutability
// because CodeGraph methods require &mut self but BackendTrait takes &self.
//
// This is safe because:
// - We own the CodeGraph exclusively (it's not shared)
// - We never expose &mut references externally
// - Only one method call at a time can access the graph
// - Sync is NOT implemented, preventing concurrent access from multiple threads
//
// The Send implementation allows the backend to be moved between threads,
// but the lack of Sync ensures it cannot be shared concurrently.
unsafe impl Send for NativeV2Backend {}

impl std::fmt::Debug for NativeV2Backend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeV2Backend")
            .finish()
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
        Ok(Self {
            graph: UnsafeCell::new(graph),
            db_path: db_path.to_path_buf(),
        })
    }

    /// Get mutable reference to the CodeGraph
    ///
    /// # Safety
    ///
    /// The caller must ensure no other references to the graph exist during use.
    /// This method is only called from within BackendTrait methods, each of which:
    /// 1. Borrows &self (not &mut self)
    /// 2. Calls this method exactly once at the start
    /// 3. Does not store the returned reference beyond the method call
    /// 4. Does not call any other method that would access the graph concurrently
    ///
    /// This is safe because:
    /// - The UnsafeCell provides interior mutability
    /// - We own the CodeGraph exclusively
    /// - No reentrancy is possible (single-threaded method calls)
    /// - The returned reference's lifetime is scoped to the method
    #[inline]
    #[allow(clippy::mut_from_ref)] // Required for interior mutability via UnsafeCell
    unsafe fn graph(&self) -> &mut CodeGraph {
        &mut *self.graph.get()
    }

    /// Convert SymbolNode to SymbolMatch
    #[allow(clippy::too_many_arguments)] // All parameters are needed for complete symbol info
    fn symbol_node_to_match(
        &self,
        node: &SymbolNode,
        index: usize,
        snippet: Option<String>,
        snippet_truncated: Option<bool>,
        context: Option<SpanContext>,
        fan_in: Option<u64>,
        fan_out: Option<u64>,
        cyclomatic_complexity: Option<u64>,
    ) -> SymbolMatch {
        // Extract file path from canonical_fqn if available
        // Format: crate_name::file_path::kind symbol_name
        let file_path = node.canonical_fqn
            .as_ref()
            .and_then(|fqn| {
                // Try to extract file path from "crate_name::src/file.py::Function name"
                let parts: Vec<&str> = fqn.split("::").collect();
                if parts.len() >= 3 {
                    // Find the part that looks like a file path (contains '/' or has known extension)
                    parts.iter().find(|p| p.contains('/') || Self::has_known_extension(p))
                        .map(|s| s.to_string())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| String::from("<unknown>"));

        let span = Span {
            span_id: format!("{}:{}:{}", file_path, node.byte_start, node.byte_end),
            file_path: file_path.clone(),
            byte_start: node.byte_start as u64,
            byte_end: node.byte_end as u64,
            start_line: node.start_line as u64,
            start_col: node.start_col as u64,
            end_line: node.end_line as u64,
            end_col: node.end_col as u64,
            context,
        };

        SymbolMatch {
            match_id: format!("sym-{}", index),
            span,
            name: node.name.clone().unwrap_or_default(),
            kind: node.kind.clone(),
            parent: None,
            symbol_id: node.symbol_id.clone(),
            score: None,
            fqn: node.fqn.clone(),
            canonical_fqn: node.canonical_fqn.clone(),
            display_fqn: node.display_fqn.clone(),
            content_hash: None,
            symbol_kind_from_chunk: None,
            snippet,
            snippet_truncated,
            language: infer_language(&file_path).map(|s| s.to_string()),
            kind_normalized: node.kind_normalized.clone(),
            complexity_score: None,
            fan_in,
            fan_out,
            cyclomatic_complexity,
            ast_context: None,
            supernode_id: None,
        }
    }

    /// Get the backend reference (doesn't need mutable access)
    ///
    /// # Safety
    ///
    /// This method accesses the CodeGraph through the UnsafeCell to retrieve
    /// a reference to the underlying GraphBackend. This is safe because:
    /// - We're only reading the backend reference, not mutating the graph
    /// - The __backend_for_benchmarks() method returns a shared reference (Rc)
    /// - No mutable operations are performed on the graph
    /// - The returned Rc is a cheap clone that doesn't affect the graph state
    fn backend(&self) -> &std::rc::Rc<dyn sqlitegraph::GraphBackend> {
        // # Safety
        // We're only reading the backend reference, not mutating the graph.
        // The __backend_for_benchmarks() API returns a shared reference that
        // can be safely accessed without exclusive access.
        unsafe { (*self.graph.get()).__backend_for_benchmarks() }
    }

    /// Get metrics for a symbol from the KV store
    ///
    /// # Arguments
    /// * `entity_id` - The symbol's entity ID
    ///
    /// # Returns
    /// * Some((fan_in, fan_out, cyclomatic_complexity)) if metrics are available
    /// * None if metrics are not populated (graceful degradation for native-v2 databases)
    fn get_metrics(&self, entity_id: u64) -> Option<(u64, u64, u64)> {
        use sqlitegraph::backend::KvValue;

        // KV key format for symbol metrics
        let metrics_key = format!("sm:symbol:{}", entity_id);
        let snapshot = SnapshotId::current();
        let backend = self.backend();

        // Try to get metrics from KV store
        match backend.kv_get(snapshot, metrics_key.as_bytes()) {
            Ok(Some(KvValue::Bytes(data))) => {
                // Deserialize the SymbolMetrics struct
                match serde_json::from_slice::<SymbolMetrics>(&data) {
                    Ok(metrics) => Some((
                        metrics.fan_in.max(0) as u64,
                        metrics.fan_out.max(0) as u64,
                        metrics.cyclomatic_complexity.max(0) as u64,
                    )),
                    Err(_) => None, // Deserialization failed - return None gracefully
                }
            }
            _ => None, // Key not found or wrong type - return None gracefully
        }
    }

    /// Check if a path has a known source file extension
    ///
    /// This helps identify which part of an FQN is the file path.
    fn has_known_extension(path: &str) -> bool {
        path.ends_with(".rs") || path.ends_with(".py") || path.ends_with(".js")
            || path.ends_with(".ts") || path.ends_with(".tsx") || path.ends_with(".jsx")
            || path.ends_with(".c") || path.ends_with(".cpp") || path.ends_with(".cc")
            || path.ends_with(".cxx") || path.ends_with(".h") || path.ends_with(".hpp")
            || path.ends_with(".java") || path.ends_with(".go") || path.ends_with(".rb")
            || path.ends_with(".php") || path.ends_with(".swift") || path.ends_with(".kt")
            || path.ends_with(".kts") || path.ends_with(".scala") || path.ends_with(".lua")
            || path.ends_with(".r") || path.ends_with(".m") || path.ends_with(".cs")
    }

    /// Calculate relevance score for a search match
    ///
    /// This is a verbatim port of the score_match() function from src/query.rs
    /// to ensure consistent scoring across SQLite and Native-V2 backends.
    ///
    /// # Arguments
    /// * `query` - The search query string
    /// * `name` - Symbol name
    /// * `display_fqn` - Display fully qualified name
    /// * `fqn` - Fully qualified name
    /// * `regex` - Optional regex pattern for regex matching
    ///
    /// # Returns
    /// A score from 0-100 indicating match relevance
    fn score_match(
        query: &str,
        name: &str,
        display_fqn: &str,
        fqn: &str,
        regex: Option<&Regex>,
    ) -> u64 {
        let mut score = 0;

        if name == query {
            score = score.max(100);
        }
        if display_fqn == query {
            score = score.max(95);
        }
        if fqn == query {
            score = score.max(90);
        }

        if name.starts_with(query) {
            score = score.max(80);
        }
        if display_fqn.starts_with(query) {
            score = score.max(70);
        }
        if name.contains(query) {
            score = score.max(60);
        }
        if display_fqn.contains(query) {
            score = score.max(50);
        }
        if fqn.contains(query) {
            score = score.max(40);
        }

        if let Some(pattern) = regex {
            if pattern.is_match(name) {
                score = score.max(70);
            } else if pattern.is_match(display_fqn) {
                score = score.max(60);
            } else if pattern.is_match(fqn) {
                score = score.max(50);
            }
        }

        score
    }

}

impl super::BackendTrait for NativeV2Backend {
    fn search_symbols(
        &self,
        options: SearchOptions,
    ) -> Result<(SearchResponse, bool, bool), LlmError> {
        let mut results = Vec::new();
        let query_lower = options.query.to_lowercase();
        let mut file_cache = HashMap::new();

        // Compile regex pattern if using regex search
        let regex_pattern = if options.use_regex {
            Some(Regex::new(options.query).map_err(|e| LlmError::InvalidQuery {
                query: format!("Invalid regex: {}", e),
            })?)
        } else {
            None
        };

        // # Safety
        // We have exclusive access through &self because:
        // - This method borrows &self exclusively
        // - No other references to the graph exist during this call
        // - The returned reference's lifetime is scoped to this method
        let graph = unsafe { self.graph() };

        // Get all indexed files
        let file_nodes = graph.all_file_nodes()
            .map_err(|e| LlmError::SearchFailed {
                reason: format!("Failed to get file nodes: {}", e),
            })?;

        // Apply path filter if specified
        let files_to_search: Vec<_> = if let Some(path_filter) = options.path_filter {
            let path_str = path_filter.to_string_lossy().to_lowercase();
            file_nodes.keys()
                .filter(|p| p.to_lowercase().starts_with(&path_str))
                .cloned()
                .collect()
        } else {
            file_nodes.keys().cloned().collect()
        };

        // Search for symbols matching the query
        for file_path in files_to_search {
            if results.len() >= options.limit {
                break;
            }

            let entries = query::symbol_nodes_in_file_with_ids(graph, &file_path)
                .map_err(|e| LlmError::SearchFailed {
                    reason: format!("Failed to query symbols in {}: {}", file_path, e),
                })?;

            for (entity_id, symbol, _symbol_id) in entries {
                if results.len() >= options.limit {
                    break;
                }

                if let Some(ref name) = symbol.name {
                    let name_lower = name.to_lowercase();
                    if !name_lower.contains(&query_lower) {
                        continue;
                    }
                } else {
                    continue;
                }

                // Apply kind filter if specified
                if let Some(kind_filter) = options.kind_filter {
                    if symbol.kind_normalized != kind_filter {
                        continue;
                    }
                }

                // Get metrics for this symbol (graceful degradation if not available)
                let metrics = self.get_metrics(entity_id as u64);

                // Apply metrics filters if specified
                if let Some(min_cc) = options.metrics.min_complexity {
                    match metrics {
                        Some((_, _, cc)) => {
                            if cc < min_cc as u64 {
                                continue;
                            }
                        }
                        None => continue, // Filter out symbols without metrics when filter is active
                    }
                }
                if let Some(min_fi) = options.metrics.min_fan_in {
                    match metrics {
                        Some((fi, _, _)) => {
                            if fi < min_fi as u64 {
                                continue;
                            }
                        }
                        None => continue, // Filter out symbols without metrics when filter is active
                    }
                }
                if let Some(min_fo) = options.metrics.min_fan_out {
                    match metrics {
                        Some((_, fo, _)) => {
                            if fo < min_fo as u64 {
                                continue;
                            }
                        }
                        None => continue, // Filter out symbols without metrics when filter is active
                    }
                }

                // Extract snippet if requested
                let file_path_str = symbol.file_path.to_string_lossy().to_string();
                let (snippet, snippet_truncated) = if options.snippet.include {
                    extract_snippet(
                        &file_path_str,
                        symbol.byte_start as u64,
                        symbol.byte_end as u64,
                        options.snippet.max_bytes,
                        &mut file_cache,
                    )
                } else {
                    (None, None)
                };

                // Extract context if requested
                let context = if options.context.include {
                    span_context_from_file(
                        &file_path_str,
                        symbol.start_line as u64,
                        symbol.end_line as u64,
                        options.context.lines,
                        false,
                        &mut file_cache,
                    )
                } else {
                    None
                };

                // Calculate score if requested
                let name = symbol.name.as_deref().unwrap_or("");
                let display_fqn = symbol.display_fqn.as_deref().unwrap_or("");
                let fqn = symbol.fqn.as_deref().unwrap_or("");

                let score = if options.include_score {
                    Some(Self::score_match(
                        options.query,
                        name,
                        display_fqn,
                        fqn,
                        regex_pattern.as_ref(),
                    ))
                } else {
                    None
                };

                // Create SymbolMatch directly (SymbolFact, not SymbolNode)
                let span = Span {
                    span_id: format!("{}:{}:{}", file_path_str, symbol.byte_start, symbol.byte_end),
                    file_path: file_path_str.clone(),
                    byte_start: symbol.byte_start as u64,
                    byte_end: symbol.byte_end as u64,
                    start_line: symbol.start_line as u64,
                    start_col: symbol.start_col as u64,
                    end_line: symbol.end_line as u64,
                    end_col: symbol.end_col as u64,
                    context,
                };

                results.push(SymbolMatch {
                    match_id: format!("sym-{}", results.len()),
                    span,
                    name: symbol.name.clone().unwrap_or_default(),
                    kind: symbol.kind.normalized_key().to_string(),
                    parent: None,
                    symbol_id: None,
                    score,
                    fqn: symbol.fqn.clone(),
                    canonical_fqn: symbol.canonical_fqn.clone(),
                    display_fqn: symbol.display_fqn.clone(),
                    content_hash: None,
                    symbol_kind_from_chunk: None,
                    snippet,
                    snippet_truncated,
                    language: infer_language(&file_path_str).map(|s| s.to_string()),
                    kind_normalized: Some(symbol.kind_normalized.clone()),
                    complexity_score: None,
                    fan_in: metrics.map(|(fi, _, _)| fi),
                    fan_out: metrics.map(|(_, fo, _)| fo),
                    cyclomatic_complexity: metrics.map(|(_, _, cc)| cc),
                    ast_context: None,
                    supernode_id: None,
                });
            }
        }

        let total_count = results.len() as u64;
        let partial = total_count >= options.limit as u64;
        let paths_bounded = options.path_filter.is_some();

        let response = SearchResponse {
            results,
            query: options.query.to_string(),
            path_filter: options.path_filter.map(|p| p.to_string_lossy().to_string()),
            kind_filter: options.kind_filter.map(|s| s.to_string()),
            total_count,
            notice: None,
        };

        Ok((response, partial, paths_bounded))
    }

    fn search_references(
        &self,
        options: SearchOptions,
    ) -> Result<(ReferenceSearchResponse, bool), LlmError> {
        let mut results = Vec::new();
        let query_lower = options.query.to_lowercase();
        let mut file_cache = HashMap::new();

        // Compile regex pattern if using regex search
        let regex_pattern = if options.use_regex {
            Some(Regex::new(options.query).map_err(|e| LlmError::InvalidQuery {
                query: format!("Invalid regex: {}", e),
            })?)
        } else {
            None
        };

        // # Safety
        // We have exclusive access through &self because:
        // - This method borrows &self exclusively
        // - No other references to the graph exist during this call
        // - The returned reference's lifetime is scoped to this method
        let graph = unsafe { self.graph() };

        // Get all indexed files
        let file_nodes = graph.all_file_nodes()
            .map_err(|e| LlmError::SearchFailed {
                reason: format!("Failed to get file nodes: {}", e),
            })?;

        // Apply path filter if specified
        let files_to_search: Vec<_> = if let Some(path_filter) = options.path_filter {
            let path_str = path_filter.to_string_lossy().to_lowercase();
            file_nodes.keys()
                .filter(|p| p.to_lowercase().starts_with(&path_str))
                .cloned()
                .collect()
        } else {
            file_nodes.keys().cloned().collect()
        };

        // For references, we need to iterate through symbols and collect their references
        for file_path in files_to_search {
            if results.len() >= options.limit {
                break;
            }

            // Get all symbols in this file
            let symbols = graph.symbol_nodes_in_file(&file_path)
                .map_err(|e| LlmError::SearchFailed {
                    reason: format!("Failed to get symbols in {}: {}", file_path, e),
                })?;

            // Get references to each symbol
            for (entity_id, _symbol) in symbols {
                if results.len() >= options.limit {
                    break;
                }

                let references = graph.references_to_symbol(entity_id)
                    .map_err(|e| LlmError::SearchFailed {
                        reason: format!("Failed to get references for symbol {}: {}", entity_id, e),
                    })?;

                for reference in references {
                    if results.len() >= options.limit {
                        break;
                    }

                    let ref_name_lower = reference.referenced_symbol.to_lowercase();
                    if !ref_name_lower.contains(&query_lower) {
                        continue;
                    }

                    // Calculate score if requested
                    let score = if options.include_score {
                        Some(Self::score_match(
                            options.query,
                            &reference.referenced_symbol,
                            &reference.referenced_symbol,
                            "",
                            regex_pattern.as_ref(),
                        ))
                    } else {
                        None
                    };

                    let file_path = reference.file_path.to_string_lossy().to_string();

                    // Extract snippet if requested
                    let (snippet, snippet_truncated) = if options.snippet.include {
                        extract_snippet(
                            &file_path,
                            reference.byte_start as u64,
                            reference.byte_end as u64,
                            options.snippet.max_bytes,
                            &mut file_cache,
                        )
                    } else {
                        (None, None)
                    };

                    let span = Span {
                        span_id: format!("{}:{}:{}", file_path, reference.byte_start, reference.byte_end),
                        file_path,
                        byte_start: reference.byte_start as u64,
                        byte_end: reference.byte_end as u64,
                        start_line: reference.start_line as u64,
                        start_col: reference.start_col as u64,
                        end_line: reference.end_line as u64,
                        end_col: reference.end_col as u64,
                        context: None,
                    };

                    results.push(ReferenceMatch {
                        match_id: format!("ref-{}", results.len()),
                        span,
                        referenced_symbol: reference.referenced_symbol,
                        reference_kind: None,
                        target_symbol_id: None,
                        score,
                        content_hash: None,
                        symbol_kind_from_chunk: None,
                        snippet,
                        snippet_truncated,
                    });
                }
            }
        }

        let total_count = results.len() as u64;
        let partial = total_count >= options.limit as u64;

        let response = ReferenceSearchResponse {
            results,
            query: options.query.to_string(),
            path_filter: options.path_filter.map(|p| p.to_string_lossy().to_string()),
            total_count,
        };

        Ok((response, partial))
    }

    fn search_calls(
        &self,
        options: SearchOptions,
    ) -> Result<(CallSearchResponse, bool), LlmError> {
        let mut results = Vec::new();
        let query_lower = options.query.to_lowercase();
        let mut file_cache = HashMap::new();

        // Compile regex pattern if using regex search
        let regex_pattern = if options.use_regex {
            Some(Regex::new(options.query).map_err(|e| LlmError::InvalidQuery {
                query: format!("Invalid regex: {}", e),
            })?)
        } else {
            None
        };

        // # Safety
        // We have exclusive access through &self because:
        // - This method borrows &self exclusively
        // - No other references to the graph exist during this call
        // - The returned reference's lifetime is scoped to this method
        let graph = unsafe { self.graph() };

        // Get all indexed files
        let file_nodes = graph.all_file_nodes()
            .map_err(|e| LlmError::SearchFailed {
                reason: format!("Failed to get file nodes: {}", e),
            })?;

        // Apply path filter if specified
        let files_to_search: Vec<_> = if let Some(path_filter) = options.path_filter {
            let path_str = path_filter.to_string_lossy().to_lowercase();
            file_nodes.keys()
                .filter(|p| p.to_lowercase().starts_with(&path_str))
                .cloned()
                .collect()
        } else {
            file_nodes.keys().cloned().collect()
        };

        // For calls, we can use calls_from_symbol for each symbol in the file
        for file_path in files_to_search {
            if results.len() >= options.limit {
                break;
            }

            // Get all symbols in this file
            let symbols = graph.symbol_nodes_in_file(&file_path)
                .map_err(|e| LlmError::SearchFailed {
                    reason: format!("Failed to get symbols in {}: {}", file_path, e),
                })?;

            // Get calls from each symbol
            for (_entity_id, symbol) in symbols {
                if results.len() >= options.limit {
                    break;
                }

                let name = if let Some(n) = &symbol.name {
                    n.clone()
                } else {
                    continue;
                };

                let calls = graph.calls_from_symbol(&file_path, &name)
                    .map_err(|e| LlmError::SearchFailed {
                        reason: format!("Failed to get calls from {}::{}: {}", file_path, name, e),
                    })?;

                for call in calls {
                    if results.len() >= options.limit {
                        break;
                    }

                    let caller_lower = call.caller.to_lowercase();
                    let callee_lower = call.callee.to_lowercase();

                    if !caller_lower.contains(&query_lower) && !callee_lower.contains(&query_lower) {
                        continue;
                    }

                    // Calculate score if requested (use max of caller and callee scores)
                    let score = if options.include_score {
                        let caller_score = Self::score_match(
                            options.query,
                            &call.caller,
                            &call.caller,
                            "",
                            regex_pattern.as_ref(),
                        );
                        let callee_score = Self::score_match(
                            options.query,
                            &call.callee,
                            &call.callee,
                            "",
                            regex_pattern.as_ref(),
                        );
                        Some(caller_score.max(callee_score))
                    } else {
                        None
                    };

                    let file_path = call.file_path.to_string_lossy().to_string();

                    // Extract snippet if requested
                    let (snippet, snippet_truncated) = if options.snippet.include {
                        extract_snippet(
                            &file_path,
                            call.byte_start as u64,
                            call.byte_end as u64,
                            options.snippet.max_bytes,
                            &mut file_cache,
                        )
                    } else {
                        (None, None)
                    };

                    let span = Span {
                        span_id: format!("{}:{}:{}", file_path, call.byte_start, call.byte_end),
                        file_path,
                        byte_start: call.byte_start as u64,
                        byte_end: call.byte_end as u64,
                        start_line: call.start_line as u64,
                        start_col: call.start_col as u64,
                        end_line: call.end_line as u64,
                        end_col: call.end_col as u64,
                        context: None,
                    };

                    results.push(CallMatch {
                        match_id: format!("call-{}", results.len()),
                        span,
                        caller: call.caller,
                        callee: call.callee,
                        caller_symbol_id: call.caller_symbol_id,
                        callee_symbol_id: call.callee_symbol_id,
                        score,
                        content_hash: None,
                        symbol_kind_from_chunk: None,
                        snippet,
                        snippet_truncated,
                    });
                }
            }
        }

        let total_count = results.len() as u64;
        let partial = total_count >= options.limit as u64;

        let response = CallSearchResponse {
            results,
            query: options.query.to_string(),
            path_filter: options.path_filter.map(|p| p.to_string_lossy().to_string()),
            total_count,
        };

        Ok((response, partial))
    }

    fn ast(
        &self,
        file: &Path,
        position: Option<usize>,
        limit: usize,
    ) -> Result<serde_json::Value, LlmError> {
        let file_path = file.to_str()
            .ok_or_else(|| LlmError::SearchFailed {
                reason: format!("File path {:?} is not valid UTF-8", file),
            })?;

        // # Safety
        // We have exclusive access through &self because:
        // - This method borrows &self exclusively
        // - No other references to the graph exist during this call
        // - The returned reference's lifetime is scoped to this method
        let graph = unsafe { self.graph() };

        let nodes = graph.get_ast_nodes_by_file(file_path)
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

        serde_json::to_value(filtered).map_err(LlmError::JsonError)
    }

    fn find_ast(&self, kind: &str) -> Result<serde_json::Value, LlmError> {
        // # Safety
        // We have exclusive access through &self because:
        // - This method borrows &self exclusively
        // - No other references to the graph exist during this call
        // - The returned reference's lifetime is scoped to this method
        let graph = unsafe { self.graph() };

        let nodes = graph.get_ast_nodes_by_kind(kind)
            .map_err(|e| LlmError::SearchFailed {
                reason: format!("Failed to get AST nodes by kind: {}", e),
            })?;

        serde_json::to_value(nodes).map_err(LlmError::JsonError)
    }

    fn complete(&self, prefix: &str, limit: usize) -> Result<Vec<String>, LlmError> {
        use magellan::kv::keys::sym_fqn_key;

        let prefix_key = sym_fqn_key(prefix);
        let snapshot = SnapshotId::current();
        let backend = self.backend();

        let entries = backend.kv_prefix_scan(snapshot, &prefix_key)
            .map_err(|e| LlmError::SearchFailed {
                reason: format!("KV prefix scan failed: {}", e),
            })?;

        let completions: Vec<String> = entries
            .iter()
            .filter_map(|(key, _value)| {
                let key_str = String::from_utf8(key.clone()).ok()?;
                key_str.strip_prefix("sym:fqn:").map(|s| s.to_string())
            })
            .take(limit)
            .collect();

        Ok(completions)
    }

    fn lookup(&self, fqn: &str, db_path: &str) -> Result<crate::output::SymbolMatch, LlmError> {
        use magellan::kv::lookup_symbol_by_fqn;

        let partial = fqn.rsplit("::").next().unwrap_or(fqn);

        let backend = self.backend();
        let snapshot = SnapshotId::current();

        let entity_id = lookup_symbol_by_fqn(backend.as_ref(), fqn)
            .ok_or_else(|| LlmError::SymbolNotFound {
                fqn: fqn.to_string(),
                db: db_path.to_string(),
                partial: partial.to_string(),
            })?;

        let node = backend.get_node(snapshot, entity_id)
            .map_err(|e| LlmError::SearchFailed {
                reason: format!("Failed to get node for entity ID {}: {}", entity_id, e),
            })?;

        let symbol_node: SymbolNode = serde_json::from_value(node.data)
            .map_err(|e| LlmError::SearchFailed {
                reason: format!("Failed to parse SymbolNode: {}", e),
            })?;

        Ok(self.symbol_node_to_match(&symbol_node, 0, None, None, None, None, None, None))
    }

    fn search_by_label(
        &self,
        label: &str,
        limit: usize,
        _db_path: &str,
    ) -> Result<(SearchResponse, bool, bool), LlmError> {
        use magellan::kv::encoding::decode_symbol_ids;
        use magellan::kv::keys::label_key;

        let key = label_key(label);
        let snapshot = SnapshotId::current();
        let backend = self.backend();

        let symbol_ids = match backend.kv_get(snapshot, &key)
            .map_err(|e| LlmError::SearchFailed {
                reason: format!("KV lookup failed for label '{}': {}", label, e),
            })? {
            Some(kv_value) => {
                match kv_value {
                    KvValue::Bytes(data) => decode_symbol_ids(&data),
                    _ => {
                        return Ok((
                            SearchResponse {
                                results: vec![],
                                query: format!("label:{}", label),
                                path_filter: None,
                                kind_filter: None,
                                total_count: 0,
                                notice: Some(format!("Label '{}' exists but has invalid data type", label)),
                            },
                            false,
                            false,
                        ));
                    }
                }
            }
            None => {
                return Ok((
                    SearchResponse {
                        results: vec![],
                        query: format!("label:{}", label),
                        path_filter: None,
                        kind_filter: None,
                        total_count: 0,
                        notice: None,
                    },
                    false,
                    false,
                ));
            }
        };

        let mut results = Vec::new();
        for symbol_id in symbol_ids.into_iter().take(limit) {
            match backend.get_node(snapshot, symbol_id) {
                Ok(node) => {
                    if let Ok(symbol_node) = serde_json::from_value::<SymbolNode>(node.data) {
                        results.push(self.symbol_node_to_match(&symbol_node, results.len(), None, None, None, None, None, None));
                    }
                }
                Err(_) => continue,
            }
        }

        let total_count = results.len() as u64;
        let partial = total_count >= limit as u64;

        let response = SearchResponse {
            results,
            query: format!("label:{}", label),
            path_filter: None,
            kind_filter: None,
            total_count,
            notice: None,
        };

        Ok((response, partial, false))
    }
}
