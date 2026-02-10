//! Native-V2 backend implementation.
//!
//! NativeV2Backend provides the Backend trait implementation for Native-V2 databases.
//! This backend requires the native-v2 feature flag and uses the CodeGraph API.

#![cfg(feature = "native-v2")]

use crate::error::LlmError;
use crate::output::{
    CallSearchResponse, CallMatch, ReferenceSearchResponse, ReferenceMatch,
    SearchResponse, SymbolMatch, Span,
};
use crate::query::SearchOptions;
use magellan::CodeGraph;
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use sqlitegraph::SnapshotId;
use sqlitegraph::backend::KvValue;

/// Native-V2 backend implementation.
///
/// Wraps a CodeGraph and implements the Backend trait.
/// This backend is only available when the native-v2 feature is enabled.
///
/// Note: Native-v2 databases in Magellan 2.1.0 still use SQLite as the storage
/// layer (via sqlitegraph), so we can use SQL queries for symbol search.
pub struct NativeV2Backend {
    #[allow(dead_code)]
    pub(crate) graph: CodeGraph,
    /// Database path for direct SQL queries
    db_path: PathBuf,
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
        Ok(Self {
            graph,
            db_path: db_path.to_path_buf(),
        })
    }

    /// Get a database connection for SQL queries.
    fn connect(&self) -> Result<Connection, LlmError> {
        Connection::open(&self.db_path).map_err(LlmError::from)
    }
}

impl super::BackendTrait for NativeV2Backend {
    fn search_symbols(
        &self,
        options: SearchOptions,
    ) -> Result<(SearchResponse, bool, bool), LlmError> {
        // For native-v2, use SQL queries via the connection
        // The native-v2 feature in Magellan still uses SQLite as the storage layer
        let conn = self.connect()?;

        // Build a simple search query for symbols
        let mut query = String::from(
            "SELECT
                s.symbol_id,
                s.name,
                s.kind,
                s.fqn,
                s.canonical_fqn,
                s.display_fqn,
                s.file_path,
                s.byte_start,
                s.byte_end,
                s.start_line,
                s.start_col,
                s.end_line,
                s.end_col,
                s.language,
                s.parent_name
            FROM symbol_nodes s
            WHERE s.name LIKE ? ESCAPE '\\'"
        );

        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(format!("%{}%", options.query.replace("%", "\\%").replace("_", "\\_")))];

        // Add path filter if specified
        if let Some(path) = options.path_filter {
            query.push_str(" AND s.file_path LIKE ? ESCAPE '\\'");
            params.push(Box::new(format!("{}%", path.to_string_lossy())));
        }

        // Add kind filter if specified
        if let Some(kind) = options.kind_filter {
            query.push_str(" AND s.kind = ?");
            params.push(Box::new(kind.to_string()));
        }

        // Add limit
        query.push_str(" LIMIT ?");
        params.push(Box::new(options.limit as i64));

        let mut stmt = conn.prepare(&query)?;

        let mut results = Vec::new();
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let mut rows = stmt.query(param_refs.as_slice())?;

        while let Some(row) = rows.next()? {
            let file_path: String = row.get(6)?;
            let byte_start: u64 = row.get(7)?;
            let byte_end: u64 = row.get(8)?;
            let start_line: u64 = row.get(9)?;
            let start_col: u64 = row.get(10)?;
            let end_line: u64 = row.get(11)?;
            let end_col: u64 = row.get(12)?;

            let span = Span {
                span_id: format!("{}:{}:{}", file_path, byte_start, byte_end),
                file_path,
                byte_start,
                byte_end,
                start_line,
                start_col,
                end_line,
                end_col,
                context: None,
            };

            results.push(SymbolMatch {
                match_id: format!("sym-{}", results.len()),
                span,
                name: row.get(1)?,
                kind: row.get(2)?,
                parent: row.get(14)?,
                symbol_id: row.get(0)?,
                score: None,
                fqn: row.get(3)?,
                canonical_fqn: row.get(4)?,
                display_fqn: row.get(5)?,
                content_hash: None,
                symbol_kind_from_chunk: None,
                snippet: None,
                snippet_truncated: None,
                language: row.get(13)?,
                kind_normalized: None,
                complexity_score: None,
                fan_in: None,
                fan_out: None,
                cyclomatic_complexity: None,
                ast_context: None,
                supernode_id: None,
            });
        }

        let total_count = results.len() as u64;
        let response = SearchResponse {
            results,
            query: options.query.to_string(),
            path_filter: options.path_filter.map(|p| p.to_string_lossy().to_string()),
            kind_filter: options.kind_filter.map(|s| s.to_string()),
            total_count,
            notice: None,
        };

        // (response, partial, paths_bounded)
        // partial: true if results were truncated
        // paths_bounded: true if path filter was applied and limited results
        let partial = total_count >= options.limit as u64;
        let paths_bounded = options.path_filter.is_some();

        Ok((response, partial, paths_bounded))
    }

    fn search_references(
        &self,
        options: SearchOptions,
    ) -> Result<(ReferenceSearchResponse, bool), LlmError> {
        let conn = self.connect()?;

        // Build a search query for references
        let mut query = String::from(
            "SELECT DISTINCT
                r.file_path,
                r.referenced_symbol,
                r.byte_start,
                r.byte_end,
                r.start_line,
                r.start_col,
                r.end_line,
                r.end_col,
                s.symbol_id
            FROM reference_nodes r
            LEFT JOIN symbol_nodes s ON r.referenced_symbol = s.name
            WHERE r.referenced_symbol LIKE ? ESCAPE '\\'"
        );

        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(format!("%{}%", options.query.replace("%", "\\%").replace("_", "\\_")))];

        // Add path filter if specified
        if let Some(path) = options.path_filter {
            query.push_str(" AND r.file_path LIKE ? ESCAPE '\\'");
            params.push(Box::new(format!("{}%", path.to_string_lossy())));
        }

        // Add limit
        query.push_str(" LIMIT ?");
        params.push(Box::new(options.limit as i64));

        let mut stmt = conn.prepare(&query)?;

        let mut results = Vec::new();
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let mut rows = stmt.query(param_refs.as_slice())?;

        while let Some(row) = rows.next()? {
            let file_path: String = row.get(0)?;
            let byte_start: u64 = row.get(2)?;
            let byte_end: u64 = row.get(3)?;
            let start_line: u64 = row.get(4)?;
            let start_col: u64 = row.get(5)?;
            let end_line: u64 = row.get(6)?;
            let end_col: u64 = row.get(7)?;

            let span = Span {
                span_id: format!("{}:{}:{}", file_path, byte_start, byte_end),
                file_path,
                byte_start,
                byte_end,
                start_line,
                start_col,
                end_line,
                end_col,
                context: None,
            };

            results.push(ReferenceMatch {
                match_id: format!("ref-{}", results.len()),
                span,
                referenced_symbol: row.get(1)?,
                reference_kind: None,
                target_symbol_id: row.get(8)?,
                score: None,
                content_hash: None,
                symbol_kind_from_chunk: None,
                snippet: None,
                snippet_truncated: None,
            });
        }

        let total_count = results.len() as u64;
        let response = ReferenceSearchResponse {
            results,
            query: options.query.to_string(),
            path_filter: options.path_filter.map(|p| p.to_string_lossy().to_string()),
            total_count,
        };

        let partial = total_count >= options.limit as u64;
        Ok((response, partial))
    }

    fn search_calls(
        &self,
        options: SearchOptions,
    ) -> Result<(CallSearchResponse, bool), LlmError> {
        let conn = self.connect()?;

        // Build a search query for calls
        // Search for calls where either caller or callee matches the query
        let mut query = String::from(
            "SELECT DISTINCT
                c.file_path,
                c.caller,
                c.callee,
                c.caller_symbol_id,
                c.callee_symbol_id,
                c.byte_start,
                c.byte_end,
                c.start_line,
                c.start_col,
                c.end_line,
                c.end_col
            FROM call_nodes c
            WHERE (c.caller LIKE ? ESCAPE '\\' OR c.callee LIKE ? ESCAPE '\\')"
        );

        let query_pattern = format!("%{}%", options.query.replace("%", "\\%").replace("_", "\\_"));
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![
            Box::new(query_pattern.clone()),
            Box::new(query_pattern),
        ];

        // Add path filter if specified
        if let Some(path) = options.path_filter {
            query.push_str(" AND c.file_path LIKE ? ESCAPE '\\'");
            params.push(Box::new(format!("{}%", path.to_string_lossy())));
        }

        // Add limit
        query.push_str(" LIMIT ?");
        params.push(Box::new(options.limit as i64));

        let mut stmt = conn.prepare(&query)?;

        let mut results = Vec::new();
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let mut rows = stmt.query(param_refs.as_slice())?;

        while let Some(row) = rows.next()? {
            let file_path: String = row.get(0)?;
            let byte_start: u64 = row.get(5)?;
            let byte_end: u64 = row.get(6)?;
            let start_line: u64 = row.get(7)?;
            let start_col: u64 = row.get(8)?;
            let end_line: u64 = row.get(9)?;
            let end_col: u64 = row.get(10)?;

            let span = Span {
                span_id: format!("{}:{}:{}", file_path, byte_start, byte_end),
                file_path,
                byte_start,
                byte_end,
                start_line,
                start_col,
                end_line,
                end_col,
                context: None,
            };

            results.push(CallMatch {
                match_id: format!("call-{}", results.len()),
                span,
                caller: row.get(1)?,
                callee: row.get(2)?,
                caller_symbol_id: row.get(3)?,
                callee_symbol_id: row.get(4)?,
                score: None,
                content_hash: None,
                symbol_kind_from_chunk: None,
                snippet: None,
                snippet_truncated: None,
            });
        }

        let total_count = results.len() as u64;
        let response = CallSearchResponse {
            results,
            query: options.query.to_string(),
            path_filter: options.path_filter.map(|p| p.to_string_lossy().to_string()),
            total_count,
        };

        let partial = total_count >= options.limit as u64;
        Ok((response, partial))
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

    fn complete(&self, prefix: &str, limit: usize) -> Result<Vec<String>, LlmError> {
        use magellan::kv::keys::sym_fqn_key;

        // Construct prefix key for KV scan
        let prefix_key = sym_fqn_key(prefix);
        let snapshot = SnapshotId::current();

        // Perform KV prefix scan via graph backend
        let entries = self.graph.__backend_for_benchmarks().kv_prefix_scan(snapshot, &prefix_key)
            .map_err(|e| LlmError::SearchFailed {
                reason: format!("KV prefix scan failed: {}", e),
            })?;

        // Extract FQNs from keys by stripping "sym:fqn:" prefix
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

    fn lookup(&self, fqn: &str, db_path: &str) -> Result<SymbolMatch, LlmError> {
        use magellan::kv::lookup_symbol_by_fqn;

        // Extract partial name from FQN for error messages
        let partial = fqn.rsplit("::").next().unwrap_or(fqn);

        // O(1) lookup by FQN using KV store
        let symbol_id = lookup_symbol_by_fqn(self.graph.__backend_for_benchmarks().as_ref(), fqn)
            .ok_or_else(|| LlmError::SymbolNotFound {
                fqn: fqn.to_string(),
                db: db_path.to_string(),
                partial: partial.to_string(),
            })?;

        // Query symbol_nodes table for full details
        let conn = self.connect()?;

        let mut stmt = conn.prepare(
            "SELECT symbol_id, name, kind, fqn, canonical_fqn, display_fqn,
                    file_path, byte_start, byte_end, start_line, start_col,
                    end_line, end_col, language, parent_name
             FROM symbol_nodes WHERE id = ?"
        ).map_err(LlmError::from)?;

        let mut rows = stmt.query(rusqlite::params![symbol_id])?;

        if let Some(row) = rows.next()? {
            let file_path: String = row.get(6)?;
            let byte_start: u64 = row.get(7)?;
            let byte_end: u64 = row.get(8)?;
            let start_line: u64 = row.get(9)?;
            let start_col: u64 = row.get(10)?;
            let end_line: u64 = row.get(11)?;
            let end_col: u64 = row.get(12)?;

            let span = Span {
                span_id: format!("{}:{}:{}", file_path, byte_start, byte_end),
                file_path,
                byte_start,
                byte_end,
                start_line,
                start_col,
                end_line,
                end_col,
                context: None,
            };

            Ok(SymbolMatch {
                match_id: format!("sym-{}", symbol_id),
                span,
                name: row.get(1)?,
                kind: row.get(2)?,
                parent: row.get(14)?,
                symbol_id: row.get(0)?,
                score: None,
                fqn: row.get(3)?,
                canonical_fqn: row.get(4)?,
                display_fqn: row.get(5)?,
                content_hash: None,
                symbol_kind_from_chunk: None,
                snippet: None,
                snippet_truncated: None,
                language: row.get(13)?,
                kind_normalized: None,
                complexity_score: None,
                fan_in: None,
                fan_out: None,
                cyclomatic_complexity: None,
                ast_context: None,
                supernode_id: None,
            })
        } else {
            // Symbol ID was found in KV but not in symbol_nodes table
            // This is a data inconsistency
            Err(LlmError::SearchFailed {
                reason: format!(
                    "Symbol ID {} found in KV store but not in symbol_nodes table. Database may be corrupted.",
                    symbol_id
                ),
            })
        }
    }

    fn search_by_label(
        &self,
        label: &str,
        limit: usize,
        db_path: &str,
    ) -> Result<(SearchResponse, bool, bool), LlmError> {
        use magellan::kv::encoding::decode_symbol_ids;
        use magellan::kv::keys::label_key;

        // Construct label key for KV lookup
        let key = label_key(label);
        let snapshot = SnapshotId::current();

        // Look up label value from KV store
        let symbol_ids = match self.graph.__backend_for_benchmarks().kv_get(snapshot, &key)
            .map_err(|e| LlmError::SearchFailed {
                reason: format!("KV lookup failed for label '{}': {}", label, e),
            })? {
            Some(kv_value) => {
                match kv_value {
                    KvValue::Bytes(data) => decode_symbol_ids(&data),
                    _ => {
                        // Wrong value type - return empty results
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
                // Label not found - return empty results (not an error)
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

        // Query symbol_nodes table for full details
        let conn = self.connect()?;

        // Build placeholders string for IN clause
        let placeholders = symbol_ids.iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect::<Vec<_>>()
            .join(",");

        if symbol_ids.is_empty() {
            // No symbols for this label
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

        let mut query = format!(
            "SELECT
                s.symbol_id,
                s.name,
                s.kind,
                s.fqn,
                s.canonical_fqn,
                s.display_fqn,
                s.file_path,
                s.byte_start,
                s.byte_end,
                s.start_line,
                s.start_col,
                s.end_line,
                s.end_col,
                s.language,
                s.parent_name
            FROM symbol_nodes s
            WHERE s.id IN ({})
            LIMIT {}",
            placeholders, limit
        );

        let mut stmt = conn.prepare(&query)?;

        let mut results = Vec::new();
        let param_refs: Vec<&dyn rusqlite::ToSql> = symbol_ids.iter()
            .map(|id| id as &dyn rusqlite::ToSql)
            .collect();

        let mut rows = stmt.query(param_refs.as_slice())?;

        while let Some(row) = rows.next()? {
            let file_path: String = row.get(6)?;
            let byte_start: u64 = row.get(7)?;
            let byte_end: u64 = row.get(8)?;
            let start_line: u64 = row.get(9)?;
            let start_col: u64 = row.get(10)?;
            let end_line: u64 = row.get(11)?;
            let end_col: u64 = row.get(12)?;

            let span = Span {
                span_id: format!("{}:{}:{}", file_path, byte_start, byte_end),
                file_path,
                byte_start,
                byte_end,
                start_line,
                start_col,
                end_line,
                end_col,
                context: None,
            };

            results.push(SymbolMatch {
                match_id: format!("sym-{}", results.len()),
                span,
                name: row.get(1)?,
                kind: row.get(2)?,
                parent: row.get(14)?,
                symbol_id: row.get(0)?,
                score: None,
                fqn: row.get(3)?,
                canonical_fqn: row.get(4)?,
                display_fqn: row.get(5)?,
                content_hash: None,
                symbol_kind_from_chunk: None,
                snippet: None,
                snippet_truncated: None,
                language: row.get(13)?,
                kind_normalized: None,
                complexity_score: None,
                fan_in: None,
                fan_out: None,
                cyclomatic_complexity: None,
                ast_context: None,
                supernode_id: None,
            });
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
