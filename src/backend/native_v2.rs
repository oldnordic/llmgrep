//! Native-V2 backend implementation.
//!
//! NativeV2Backend provides the Backend trait implementation for Native-V2 databases.
//! This backend requires the native-v2 feature flag and uses the CodeGraph API.

#![cfg(feature = "native-v2")]

use crate::error::LlmError;
use crate::infer_language;
use crate::output::{
    CallSearchResponse, CallMatch, ReferenceSearchResponse, ReferenceMatch,
    SearchResponse, SymbolMatch, Span,
};
use crate::query::SearchOptions;
use magellan::graph::{query, SymbolNode};
use magellan::CodeGraph;
use regex::Regex;
use std::cell::UnsafeCell;
use std::path::{Path, PathBuf};
use sqlitegraph::SnapshotId;
use sqlitegraph::backend::KvValue;

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
    /// Database path for error messages
    db_path: PathBuf,
}

// SAFETY: NativeV2Backend owns the CodeGraph exclusively and only provides
// one mutable reference at a time within each method. Send/Sync are not implemented.
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
    /// Caller must ensure no other references to the graph exist during use
    #[inline]
    unsafe fn graph(&self) -> &mut CodeGraph {
        &mut *self.graph.get()
    }

    /// Convert SymbolNode to SymbolMatch
    fn symbol_node_to_match(&self, node: &SymbolNode, index: usize) -> SymbolMatch {
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
            context: None,
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
            snippet: None,
            snippet_truncated: None,
            language: infer_language(&file_path).map(|s| s.to_string()),
            kind_normalized: node.kind_normalized.clone(),
            complexity_score: None,
            fan_in: None,
            fan_out: None,
            cyclomatic_complexity: None,
            ast_context: None,
            supernode_id: None,
        }
    }

    /// Get the backend reference (doesn't need mutable access)
    fn backend(&self) -> &std::rc::Rc<dyn sqlitegraph::GraphBackend> {
        // SAFETY: We're only reading the backend, getting a shared reference
        unsafe { (*self.graph.get()).__backend_for_benchmarks() }
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

        // Compile regex pattern if using regex search
        let regex_pattern = if options.use_regex {
            Some(Regex::new(options.query).map_err(|e| LlmError::InvalidQuery {
                query: format!("Invalid regex: {}", e),
            })?)
        } else {
            None
        };

        // SAFETY: We have exclusive access through &self
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

            for (_node_id, symbol, _symbol_id) in entries {
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
                    if symbol.kind_normalized != kind_filter.to_string() {
                        continue;
                    }
                }

                // Convert SymbolFact to SymbolMatch
                let file_path_str = symbol.file_path.to_string_lossy().to_string();
                let name = symbol.name.as_deref().unwrap_or("");
                let display_fqn = symbol.display_fqn.as_deref().unwrap_or("");
                let fqn = symbol.fqn.as_deref().unwrap_or("");

                // Calculate score if requested
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

                let span = Span {
                    span_id: format!("{}:{}:{}", file_path_str, symbol.byte_start, symbol.byte_end),
                    file_path: file_path_str.clone(),
                    byte_start: symbol.byte_start as u64,
                    byte_end: symbol.byte_end as u64,
                    start_line: symbol.start_line as u64,
                    start_col: symbol.start_col as u64,
                    end_line: symbol.end_line as u64,
                    end_col: symbol.end_col as u64,
                    context: None,
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
                    snippet: None,
                    snippet_truncated: None,
                    language: infer_language(&file_path_str).map(|s| s.to_string()),
                    kind_normalized: Some(symbol.kind_normalized.clone()),
                    complexity_score: None,
                    fan_in: None,
                    fan_out: None,
                    cyclomatic_complexity: None,
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

        // Compile regex pattern if using regex search
        let regex_pattern = if options.use_regex {
            Some(Regex::new(options.query).map_err(|e| LlmError::InvalidQuery {
                query: format!("Invalid regex: {}", e),
            })?)
        } else {
            None
        };

        // SAFETY: We have exclusive access through &self
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
                        snippet: None,
                        snippet_truncated: None,
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

        // Compile regex pattern if using regex search
        let regex_pattern = if options.use_regex {
            Some(Regex::new(options.query).map_err(|e| LlmError::InvalidQuery {
                query: format!("Invalid regex: {}", e),
            })?)
        } else {
            None
        };

        // SAFETY: We have exclusive access through &self
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
                        snippet: None,
                        snippet_truncated: None,
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

        // SAFETY: We have exclusive access through &self
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
        // SAFETY: We have exclusive access through &self
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

        Ok(self.symbol_node_to_match(&symbol_node, 0))
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
                        results.push(self.symbol_node_to_match(&symbol_node, results.len()));
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
