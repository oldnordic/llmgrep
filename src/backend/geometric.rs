//! Geometric backend implementation for .geo database files.
//!
//! This backend provides integration with Magellan's geometric backend,
//! which stores code graphs in a specialized format optimized for
//! spatial queries and CFG analysis.

#![cfg(feature = "geometric-backend")]

use crate::backend::magellan_adapter::normalize_path_for_query;
use crate::error::LlmError;
use crate::output::{
    CallMatch, CallSearchResponse, ReferenceMatch, ReferenceSearchResponse, SearchResponse, Span,
    SymbolMatch,
};
use crate::query::util::score_match;
use crate::query::SearchOptions;
use magellan::graph::geometric_backend::SymbolInfo;
use magellan::graph::GeometricBackend as MagellanGeometricBackend;
use regex::Regex;
use std::path::Path;

/// Geometric backend for .geo database files.
///
/// This backend wraps Magellan's GeometricBackend and provides
/// a unified interface for symbol search and navigation.
pub struct GeometricBackend {
    inner: MagellanGeometricBackend,
    db_path: std::path::PathBuf,
}

impl std::fmt::Debug for GeometricBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GeometricBackend")
            .field("db_path", &self.db_path)
            .finish_non_exhaustive()
    }
}
impl GeometricBackend {
    /// Open a geometric database file.
    ///
    /// # Arguments
    /// * `db_path` - Path to the .geo database file
    ///
    /// # Returns
    /// * `Ok(GeometricBackend)` - Backend instance
    /// * `Err(LlmError)` - If database cannot be opened
    pub fn open(db_path: &Path) -> Result<Self, LlmError> {
        // Validate the file exists
        if !db_path.exists() {
            return Err(LlmError::DatabaseNotFound {
                path: db_path.display().to_string(),
            });
        }

        // Open the magellan geometric backend
        let inner = MagellanGeometricBackend::open(db_path).map_err(|e| {
            LlmError::BackendDetectionFailed {
                path: db_path.display().to_string(),
                reason: format!("Failed to open geometric backend: {}", e),
            }
        })?;

        // Validate Magellan schema version if the file is also a valid SQLite database
        // (some .geo files may be SQLite-based or have companion SQLite metadata)
        if let Ok(conn) = rusqlite::Connection::open(db_path) {
            if let Err(e) = crate::backend::schema_check::check_schema_version(&conn) {
                // Only fail on actual schema version mismatch, not on "not a database"
                if !e.to_lowercase().contains("not a database") {
                    return Err(LlmError::SchemaMismatch {
                        reason: format!("Schema version check failed: {}", e),
                    });
                }
            }
        }

        Ok(Self {
            inner,
            db_path: db_path.to_path_buf(),
        })
    }

    /// Convert Magellan's SymbolInfo to llmgrep's SymbolMatch with metrics
    fn symbol_info_to_match(
        &self,
        info: SymbolInfo,
        query: &str,
        regex: Option<&Regex>,
    ) -> SymbolMatch {
        // Get complexity if it's a function
        let mut complexity = None;
        if matches!(
            info.kind,
            magellan::ingest::SymbolKind::Function | magellan::ingest::SymbolKind::Method
        ) {
            let res = self.inner.calculate_complexity(info.id as i64);
            complexity = Some(res.cyclomatic_complexity as u64);
        }

        let score = score_match(query, &info.name, &info.fqn, &info.fqn, regex);

        SymbolMatch {
            match_id: format!("match_{}", info.id),
            span: Span {
                span_id: format!("span_{}", info.id),
                file_path: info.file_path.clone(),
                byte_start: info.byte_start,
                byte_end: info.byte_end,
                start_line: info.start_line,
                start_col: info.start_col,
                end_line: info.end_line,
                end_col: info.end_col,
                context: None,
            },
            name: info.name.clone(),
            kind: format!("{:?}", info.kind),
            parent: None,
            symbol_id: Some(format!("{:016x}", info.id)),
            score: if score > 0 { Some(score) } else { None },
            fqn: Some(info.fqn.clone()),
            canonical_fqn: Some(info.fqn.clone()),
            display_fqn: Some(info.fqn.clone()),
            content_hash: None,
            symbol_kind_from_chunk: None,
            snippet: None,
            snippet_truncated: None,
            language: Some(format!("{:?}", info.language).to_lowercase()),
            kind_normalized: Some(format!("{:?}", info.kind).to_lowercase()),
            complexity_score: complexity,
            fan_in: Some(self.inner.get_callers(info.id).len() as u64),
            fan_out: Some(self.inner.get_callees(info.id).len() as u64),
            cyclomatic_complexity: complexity.map(|c| c as u64),
            ast_context: None,
            supernode_id: None,
            coverage: None,
        }
    }
}

impl crate::backend::BackendTrait for GeometricBackend {
    fn search_symbols(
        &self,
        options: SearchOptions,
    ) -> Result<(SearchResponse, bool, bool), LlmError> {
        // 1. Get all symbols (search_symbols in magellan is currently pattern-only)
        let all_symbols = self
            .inner
            .get_all_symbols()
            .map_err(|e| LlmError::SearchFailed {
                reason: format!("Failed to get all symbols: {}", e),
            })?;

        // 2. Filter by regex or pattern
        let pattern_lower = options.query.to_lowercase();
        let regex = if options.use_regex {
            Some(
                Regex::new(options.query).map_err(|e| LlmError::SearchFailed {
                    reason: format!("Invalid regex: {}", e),
                })?,
            )
        } else {
            None
        };

        let filtered: Vec<SymbolInfo> = all_symbols
            .into_iter()
            .filter(|info| {
                // Pattern/Regex match
                let is_match = if let Some(ref re) = regex {
                    re.is_match(&info.fqn) || re.is_match(&info.name)
                } else {
                    info.fqn.to_lowercase().contains(&pattern_lower)
                        || info.name.to_lowercase().contains(&pattern_lower)
                };
                if !is_match {
                    return false;
                }

                // Path filter - use normalized comparison for contract compliance
                if let Some(ref path_filter) = options.path_filter {
                    let filter_str = path_filter.to_string_lossy();
                    let normalized_filter = normalize_path_for_query(&filter_str);
                    let normalized_symbol_path = normalize_path_for_query(&info.file_path);
                    // Check if normalized paths have containment relationship
                    if !normalized_symbol_path.contains(&normalized_filter)
                        && !normalized_filter.contains(&normalized_symbol_path)
                    {
                        return false;
                    }
                }

                // Kind filter - compare SymbolKind enum as strings
                if let Some(ref kind_filter) = options.kind_filter {
                    let kind_str = format!("{:?}", info.kind).to_lowercase();
                    let allowed: Vec<String> = kind_filter
                        .split(',')
                        .map(|s| s.trim().to_lowercase())
                        .collect();
                    if !allowed.iter().any(|k| kind_str.contains(k)) {
                        return false;
                    }
                }

                true
            })
            .collect();

        // Apply limit
        let total_count = filtered.len() as u64;
        let partial = filtered.len() > options.limit;

        // Collect into a vec first to avoid move issues
        let filtered_vec: Vec<_> = filtered.into_iter().collect();

        let matches: Vec<SymbolMatch> = filtered_vec
            .iter()
            .take(options.limit)
            .map(|info| {
                // Get full metadata for the match
                if let Some(full_info) = self.inner.find_symbol_by_id_info(info.id) {
                    self.symbol_info_to_match(full_info, options.query, regex.as_ref())
                } else {
                    // Fallback to basic info if metadata lookup fails
                    let score = score_match(
                        options.query,
                        &info.name,
                        &info.fqn,
                        &info.fqn,
                        regex.as_ref(),
                    );
                    SymbolMatch {
                        match_id: format!("match_{}", info.id),
                        span: Span {
                            span_id: format!("span_{}", info.id),
                            file_path: info.file_path.clone(),
                            byte_start: 0,
                            byte_end: 0,
                            start_line: info.start_line,
                            start_col: 0,
                            end_line: info.end_line,
                            end_col: 0,
                            context: None,
                        },
                        name: info.name.clone(),
                        kind: format!("{:?}", info.kind),
                        parent: None,
                        symbol_id: Some(format!("{:016x}", info.id)),
                        score: if score > 0 { Some(score) } else { None },
                        fqn: Some(info.fqn.clone()),
                        canonical_fqn: Some(info.fqn.clone()),
                        display_fqn: Some(info.fqn.clone()),
                        content_hash: None,
                        symbol_kind_from_chunk: None,
                        snippet: None,
                        snippet_truncated: None,
                        language: None,
                        kind_normalized: Some(format!("{:?}", info.kind).to_lowercase()),
                        complexity_score: None,
                        fan_in: None,
                        fan_out: None,
                        cyclomatic_complexity: None,
                        ast_context: None,
                        supernode_id: None,
                        coverage: None,
                    }
                }
            })
            .collect();

        // Detect ambiguity for explicit notice
        // Check if there are multiple symbols with the same name across different files
        let mut unique_names: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut unique_files: std::collections::HashSet<String> = std::collections::HashSet::new();
        for info in &filtered_vec {
            unique_names.insert(info.name.clone());
            unique_files.insert(info.file_path.clone());
        }

        let notice = if unique_names.len() < unique_files.len() && unique_names.len() > 1 {
            // Multiple symbols with the same name in different files (ambiguous)
            let ambiguous_names: Vec<_> = unique_names.into_iter().collect();
            Some(format!(
                "Ambiguous: Found {} symbol(s) with name(s) {:?} across {} files. Use --path-filter to disambiguate.",
                ambiguous_names.len(),
                ambiguous_names,
                unique_files.len()
            ))
        } else if partial {
            Some(format!(
                "Results truncated to {} of {} matches. Use --limit to increase.",
                options.limit, total_count
            ))
        } else {
            None
        };

        let response = SearchResponse {
            query: options.query.to_string(),
            total_count,
            results: matches,
            path_filter: options.path_filter.map(|p| p.to_string_lossy().to_string()),
            kind_filter: options.kind_filter.map(|k| k.to_string()),
            notice,
        };

        Ok((response, partial, false))
    }

    fn search_references(
        &self,
        options: SearchOptions,
    ) -> Result<(ReferenceSearchResponse, bool), LlmError> {
        // 1. Find the target symbol by name/fqn
        let symbols = self.inner.find_symbols_by_name_info(options.query);

        if symbols.is_empty() {
            // Fallback: try FQN lookup
            if let Some(info) = self.inner.find_symbol_by_fqn_info(options.query) {
                return self.get_references_for_symbol(&info, options);
            }

            return Ok((
                ReferenceSearchResponse {
                    query: options.query.to_string(),
                    total_count: 0,
                    results: Vec::new(),
                    path_filter: options.path_filter.map(|p| p.to_string_lossy().to_string()),
                },
                false,
            ));
        }

        // CONTRACT FIX: Handle ambiguity explicitly
        // If multiple symbols match and no path filter, return results for all matches
        // If path_filter is provided, try to find unique match in that path
        let target_symbols: Vec<_> = if let Some(ref path_filter) = options.path_filter {
            let filter_str = path_filter.to_string_lossy();
            let normalized_filter = normalize_path_for_query(&filter_str);

            // Filter symbols by path using normalized comparison
            let filtered: Vec<_> = symbols
                .iter()
                .filter(|info| {
                    let normalized_path = normalize_path_for_query(&info.file_path);
                    normalized_path.contains(&normalized_filter)
                        || normalized_filter.contains(&normalized_path)
                })
                .cloned()
                .collect();

            if filtered.is_empty() {
                // Path filter didn't match anything, use all symbols
                symbols
            } else {
                // Use filtered results (may be unique or still ambiguous)
                filtered
            }
        } else {
            symbols
        };

        // Aggregate references for all target symbols
        let mut all_results = Vec::new();
        let mut total_count = 0u64;
        let mut partial = false;

        for target in target_symbols {
            let (response, is_partial) =
                self.get_references_for_symbol(&target, options.clone())?;
            all_results.extend(response.results);
            total_count += response.total_count;
            partial = partial || is_partial;
        }

        // Apply limit
        let truncated = all_results.len() > options.limit;
        all_results.truncate(options.limit);

        Ok((
            ReferenceSearchResponse {
                query: options.query.to_string(),
                total_count,
                results: all_results,
                path_filter: options.path_filter.map(|p| p.to_string_lossy().to_string()),
            },
            truncated || partial,
        ))
    }

    fn search_calls(&self, options: SearchOptions) -> Result<(CallSearchResponse, bool), LlmError> {
        // 1. Find the caller symbol by name/fqn
        let symbols = self.inner.find_symbols_by_name_info(options.query);

        if symbols.is_empty() {
            // Fallback: try FQN lookup
            if let Some(info) = self.inner.find_symbol_by_fqn_info(options.query) {
                return self.get_calls_from_symbol(&info, options);
            }

            return Ok((
                CallSearchResponse {
                    query: options.query.to_string(),
                    total_count: 0,
                    results: Vec::new(),
                    path_filter: options.path_filter.map(|p| p.to_string_lossy().to_string()),
                },
                false,
            ));
        }

        // CONTRACT FIX: Handle ambiguity explicitly
        let target_symbols: Vec<_> = if let Some(ref path_filter) = options.path_filter {
            let filter_str = path_filter.to_string_lossy();
            let normalized_filter = normalize_path_for_query(&filter_str);

            let filtered: Vec<_> = symbols
                .iter()
                .filter(|info| {
                    let normalized_path = normalize_path_for_query(&info.file_path);
                    normalized_path.contains(&normalized_filter)
                        || normalized_filter.contains(&normalized_path)
                })
                .cloned()
                .collect();

            if filtered.is_empty() {
                symbols
            } else {
                filtered
            }
        } else {
            symbols
        };

        // Aggregate calls for all target symbols
        let mut all_results = Vec::new();
        let mut total_count = 0u64;
        let mut partial = false;

        for target in target_symbols {
            let (response, is_partial) = self.get_calls_from_symbol(&target, options.clone())?;
            all_results.extend(response.results);
            total_count += response.total_count;
            partial = partial || is_partial;
        }

        // Apply limit
        let truncated = all_results.len() > options.limit;
        all_results.truncate(options.limit);

        Ok((
            CallSearchResponse {
                query: options.query.to_string(),
                total_count,
                results: all_results,
                path_filter: options.path_filter.map(|p| p.to_string_lossy().to_string()),
            },
            truncated || partial,
        ))
    }

    fn ast(
        &self,
        file: &Path,
        position: Option<usize>,
        limit: usize,
    ) -> Result<serde_json::Value, LlmError> {
        let file_path = file.to_string_lossy().to_string();
        let normalized_file_path = normalize_path_for_query(&file_path);

        // Try to get symbols with the provided path first
        let symbols =
            self.inner
                .symbols_in_file(&file_path)
                .map_err(|e| LlmError::SearchFailed {
                    reason: format!("Failed to get AST nodes: {}", e),
                })?;

        // CONTRACT FIX: If no symbols found, use normalized path comparison
        // Geometric backend may store paths in normalized form, while llmgrep passes unnormalized paths
        let fallback_symbols: Vec<SymbolInfo> = if symbols.is_empty() {
            // Get all symbols and filter by normalized path comparison
            let all: Vec<SymbolInfo> = self
                .inner
                .get_all_symbols()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|info| self.inner.find_symbol_by_id_info(info.id))
                .collect();

            all.into_iter()
                .filter(|info| {
                    // Use normalized path comparison for contract compliance
                    let normalized_info_path = normalize_path_for_query(&info.file_path);
                    normalized_info_path == normalized_file_path
                        || normalized_info_path.contains(&normalized_file_path)
                        || normalized_file_path.contains(&normalized_info_path)
                })
                .collect()
        } else {
            Vec::new()
        };

        // Use fallback if primary search returned empty
        let symbols_to_use = if symbols.is_empty() {
            fallback_symbols
        } else {
            symbols
        };

        let nodes: Vec<serde_json::Value> = symbols_to_use
            .into_iter()
            .take(limit)
            .map(|info| {
                serde_json::json!({
                    "id": info.id,
                    "kind": format!("{:?}", info.kind),
                    "name": info.name,
                    "fqn": info.fqn,
                    "file_path": info.file_path,
                    "byte_start": info.byte_start,
                    "byte_end": info.byte_end,
                    "start_line": info.start_line,
                    "start_col": info.start_col,
                    "end_line": info.end_line,
                    "end_col": info.end_col,
                })
            })
            .collect();

        Ok(serde_json::json!({
            "file": file_path,
            "position": position,
            "count": nodes.len(),
            "nodes": nodes,
        }))
    }

    fn find_ast(&self, kind: &str) -> Result<serde_json::Value, LlmError> {
        let all_symbols = self.inner.get_all_symbols().unwrap_or_default();

        // Default limit to prevent output size issues (1000 nodes max)
        const DEFAULT_LIMIT: usize = 1000;

        let filtered: Vec<_> = all_symbols
            .into_iter()
            .filter(|info| {
                let kind_str = format!("{:?}", info.kind).to_lowercase();
                kind_str.contains(&kind.to_lowercase())
            })
            .collect();

        let total = filtered.len();
        let truncated = total > DEFAULT_LIMIT;

        let nodes: Vec<serde_json::Value> = filtered
            .into_iter()
            .take(DEFAULT_LIMIT)
            .map(|info| {
                serde_json::json!({
                    "id": info.id,
                    "kind": info.kind,
                    "name": info.name,
                    "fqn": info.fqn,
                    "file_path": info.file_path,
                    "start_line": info.start_line,
                    "end_line": info.end_line,
                })
            })
            .collect();

        Ok(serde_json::json!({
            "kind": kind,
            "count": nodes.len(),
            "total": total,
            "truncated": truncated,
            "limit": DEFAULT_LIMIT,
            "nodes": nodes,
        }))
    }

    fn complete(&self, prefix: &str, limit: usize) -> Result<Vec<String>, LlmError> {
        let completions = self.inner.complete_fqn_prefix(prefix, limit);
        Ok(completions)
    }

    fn lookup(&self, fqn: &str, db_path: &str) -> Result<SymbolMatch, LlmError> {
        match self.inner.find_symbol_by_fqn_info(fqn) {
            Some(info) => Ok(self.symbol_info_to_match(info, fqn, None)),
            None => Err(LlmError::SymbolNotFound {
                fqn: fqn.to_string(),
                db: db_path.to_string(),
                partial: fqn.split("::").last().unwrap().to_string(), // SAFETY: split always yields >=1 element
            }),
        }
    }

    fn search_by_label(
        &self,
        _label: &str,
        _limit: usize,
        db_path: &str,
    ) -> Result<(SearchResponse, bool, bool), LlmError> {
        Err(LlmError::FeatureNotAvailable {
            feature: "search_by_label".to_string(),
            backend: "Geometric".to_string(),
            message: format!("Label search is not supported for Geometric backend. Use SQLite backend for label-based queries. Database: {}", db_path),
        })
    }

    fn get_chunks_for_symbol(
        &self,
        file_path: &str,
        symbol_name: &str,
    ) -> Result<Vec<crate::backend::magellan_adapter::CodeChunk>, LlmError> {
        use crate::backend::magellan_adapter::{
            get_chunks_for_symbol as adapter_get_chunks, ChunkLookupResult,
        };

        match adapter_get_chunks(&self.inner, file_path, symbol_name) {
            ChunkLookupResult::Found(chunks) => Ok(chunks),
            ChunkLookupResult::NotAvailable => Err(LlmError::ChunksNotAvailable {
                backend: "Geometric".to_string(),
                message: format!("No chunks found for symbol '{}' in file '{}'. Chunking may not have been enabled during indexing.", symbol_name, file_path),
            }),
            ChunkLookupResult::Error(msg) => Err(LlmError::SearchFailed {
                reason: msg,
            }),
        }
    }
}

impl GeometricBackend {
    fn get_references_for_symbol(
        &self,
        symbol: &SymbolInfo,
        options: SearchOptions,
    ) -> Result<(ReferenceSearchResponse, bool), LlmError> {
        let (callers, _callees): (Vec<u64>, Vec<u64>) =
            self.inner.get_references_bidirectional(symbol.id);

        let mut results = Vec::new();
        let ref_score = score_match(options.query, &symbol.name, "", "", None);

        // Structured references from DB
        for caller_id in callers.iter() {
            if let Some(info) = self.inner.find_symbol_by_id_info(*caller_id) {
                // Apply path filter with normalized comparison
                if let Some(ref path_filter) = options.path_filter {
                    let filter_str = path_filter.to_string_lossy();
                    let normalized_filter = normalize_path_for_query(&filter_str);
                    let normalized_path = normalize_path_for_query(&info.file_path);
                    if !normalized_path.contains(&normalized_filter)
                        && !normalized_filter.contains(&normalized_path)
                    {
                        continue;
                    }
                }

                results.push(ReferenceMatch {
                    match_id: format!("ref_{}_{}", symbol.id, caller_id),
                    span: Span {
                        span_id: format!("span_{}", caller_id),
                        file_path: info.file_path.clone(),
                        byte_start: info.byte_start,
                        byte_end: info.byte_end,
                        start_line: info.start_line,
                        start_col: info.start_col,
                        end_line: info.end_line,
                        end_col: info.end_col,
                        context: None,
                    },
                    referenced_symbol: symbol.name.clone(),
                    reference_kind: Some("structured".to_string()),
                    target_symbol_id: Some(format!("{:016x}", symbol.id)),
                    score: if ref_score > 0 { Some(ref_score) } else { None },
                    content_hash: None,
                    symbol_kind_from_chunk: None,
                    snippet: None,
                    snippet_truncated: None,
                });
            }
            if results.len() >= options.limit {
                break;
            }
        }

        // 2. Textual Fallback: If no structured refs, search by name
        if results.is_empty() {
            let all_symbols = self.inner.get_all_symbols().unwrap_or_default();
            for sym in all_symbols {
                if sym.id == symbol.id {
                    continue;
                }

                // Apply path filter with normalized comparison
                if let Some(ref path_filter) = options.path_filter {
                    let filter_str = path_filter.to_string_lossy();
                    let normalized_filter = normalize_path_for_query(&filter_str);
                    let normalized_path = normalize_path_for_query(&sym.file_path);
                    if !normalized_path.contains(&normalized_filter)
                        && !normalized_filter.contains(&normalized_path)
                    {
                        continue;
                    }
                }

                // Read file content and search for symbol name
                if let Ok(content) = std::fs::read_to_string(&sym.file_path) {
                    if content.contains(&symbol.name) {
                        let text_score = score_match(options.query, &symbol.name, "", "", None);
                        results.push(ReferenceMatch {
                            match_id: format!("textref_{}_{}", symbol.id, sym.id),
                            span: Span {
                                span_id: format!("span_{}", sym.id),
                                file_path: sym.file_path.clone(),
                                byte_start: 0,
                                byte_end: 0,
                                start_line: sym.start_line,
                                start_col: 0,
                                end_line: sym.end_line,
                                end_col: 0,
                                context: None,
                            },
                            referenced_symbol: symbol.name.clone(),
                            reference_kind: Some("textual".to_string()),
                            target_symbol_id: Some(format!("{:016x}", symbol.id)),
                            score: if text_score > 0 {
                                Some(text_score)
                            } else {
                                None
                            },
                            content_hash: None,
                            symbol_kind_from_chunk: None,
                            snippet: None,
                            snippet_truncated: None,
                        });
                    }
                }
                if results.len() >= options.limit {
                    break;
                }
            }
        }

        let total_count = results.len() as u64;
        let partial = results.len() >= options.limit;

        Ok((
            ReferenceSearchResponse {
                query: options.query.to_string(),
                total_count,
                results,
                path_filter: options.path_filter.map(|p| p.to_string_lossy().to_string()),
            },
            partial,
        ))
    }

    fn get_calls_from_symbol(
        &self,
        symbol: &SymbolInfo,
        options: SearchOptions,
    ) -> Result<(CallSearchResponse, bool), LlmError> {
        let callee_ids = self.inner.get_callees(symbol.id);
        let mut results = Vec::new();

        // 1. Structured calls
        for callee_id in callee_ids.iter() {
            if let Some(info) = self.inner.find_symbol_by_id_info(*callee_id) {
                // Apply path filter with normalized comparison
                if let Some(ref path_filter) = options.path_filter {
                    let filter_str = path_filter.to_string_lossy();
                    let normalized_filter = normalize_path_for_query(&filter_str);
                    let normalized_path = normalize_path_for_query(&info.file_path);
                    if !normalized_path.contains(&normalized_filter)
                        && !normalized_filter.contains(&normalized_path)
                    {
                        continue;
                    }
                }

                let call_score = score_match(options.query, &info.name, "", "", None);
                results.push(CallMatch {
                    match_id: format!("call_{}_{}", symbol.id, callee_id),
                    span: Span {
                        span_id: format!("span_{}", symbol.id),
                        file_path: symbol.file_path.clone(),
                        byte_start: symbol.byte_start,
                        byte_end: symbol.byte_end,
                        start_line: symbol.start_line,
                        start_col: symbol.start_col,
                        end_line: symbol.end_line,
                        end_col: symbol.end_col,
                        context: None,
                    },
                    caller: symbol.name.clone(),
                    callee: info.name.clone(),
                    caller_symbol_id: Some(format!("{:016x}", symbol.id)),
                    callee_symbol_id: Some(format!("{:016x}", callee_id)),
                    score: if call_score > 0 {
                        Some(call_score)
                    } else {
                        None
                    },
                    content_hash: None,
                    symbol_kind_from_chunk: None,
                    snippet: None,
                    snippet_truncated: None,
                });
            }
            if results.len() >= options.limit {
                break;
            }
        }

        // 2. Textual Fallback: Search for other symbol names in this function's source
        if results.is_empty() {
            if let Ok(content) = std::fs::read_to_string(&symbol.file_path) {
                if symbol.byte_start < content.len() as u64
                    && symbol.byte_end <= content.len() as u64
                {
                    let func_src = &content[symbol.byte_start as usize..symbol.byte_end as usize];
                    let all_symbols = self.inner.get_all_symbols().unwrap_or_default();

                    for other in all_symbols {
                        if other.id == symbol.id {
                            continue;
                        }

                        // Apply path filter with normalized comparison
                        if let Some(ref path_filter) = options.path_filter {
                            let filter_str = path_filter.to_string_lossy();
                            let normalized_filter = normalize_path_for_query(&filter_str);
                            let normalized_path = normalize_path_for_query(&other.file_path);
                            if !normalized_path.contains(&normalized_filter)
                                && !normalized_filter.contains(&normalized_path)
                            {
                                continue;
                            }
                        }

                        if func_src.contains(&other.name) {
                            let text_score = score_match(options.query, &other.name, "", "", None);
                            results.push(CallMatch {
                                match_id: format!("textcall_{}_{}", symbol.id, other.id),
                                span: Span {
                                    span_id: format!("span_{}", symbol.id),
                                    file_path: symbol.file_path.clone(),
                                    byte_start: symbol.byte_start,
                                    byte_end: symbol.byte_end,
                                    start_line: symbol.start_line,
                                    start_col: symbol.start_col,
                                    end_line: symbol.end_line,
                                    end_col: symbol.end_col,
                                    context: None,
                                },
                                caller: symbol.name.clone(),
                                callee: other.name.clone(),
                                caller_symbol_id: Some(format!("{:016x}", symbol.id)),
                                callee_symbol_id: Some(format!("{:016x}", other.id)),
                                score: if text_score > 0 {
                                    Some(text_score)
                                } else {
                                    None
                                },
                                content_hash: None,
                                symbol_kind_from_chunk: None,
                                snippet: None,
                                snippet_truncated: None,
                            });
                        }
                        if results.len() >= options.limit {
                            break;
                        }
                    }
                }
            }
        }

        let total_count = results.len() as u64;
        let partial = results.len() >= options.limit;

        Ok((
            CallSearchResponse {
                query: options.query.to_string(),
                total_count,
                results,
                path_filter: options.path_filter.map(|p| p.to_string_lossy().to_string()),
            },
            partial,
        ))
    }
}
