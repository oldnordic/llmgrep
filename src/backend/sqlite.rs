//! SQLite backend implementation.
//!
//! SqliteBackend provides the Backend trait implementation for SQLite databases.
//! This is the traditional storage backend and is always available.

use crate::error::LlmError;
use crate::infer_language;
use crate::output::{
    CallSearchResponse, ReferenceSearchResponse, SearchResponse, Span, SymbolMatch,
};
use crate::query::{search_calls_impl, search_references_impl, search_symbols_impl, SearchOptions};
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};

/// SQLite backend implementation.
///
/// Wraps a rusqlite Connection and implements the Backend trait.
/// All operations now use direct SQL queries instead of shelling out to magellan CLI.
#[derive(Debug)]
pub struct SqliteBackend {
    pub conn: Connection,
    db_path: PathBuf,
}

impl SqliteBackend {
    /// Open a SQLite database backend.
    ///
    /// # Arguments
    /// * `db_path` - Path to the SQLite database file
    pub fn open(db_path: &Path) -> Result<Self, LlmError> {
        let conn = Connection::open(db_path)?;
        crate::backend::schema_check::check_schema_version(&conn)
            .map_err(|e| LlmError::SchemaMismatch { reason: e })?;
        Ok(Self {
            conn,
            db_path: db_path.to_path_buf(),
        })
    }
}

impl super::BackendTrait for SqliteBackend {
    fn search_symbols(
        &self,
        options: SearchOptions,
    ) -> Result<(SearchResponse, bool, bool), LlmError> {
        search_symbols_impl(&self.conn, &self.db_path, &options)
    }

    fn search_references(
        &self,
        options: SearchOptions,
    ) -> Result<(ReferenceSearchResponse, bool), LlmError> {
        search_references_impl(&self.conn, &options)
    }

    fn search_calls(&self, options: SearchOptions) -> Result<(CallSearchResponse, bool), LlmError> {
        search_calls_impl(&self.conn, &options)
    }

    fn ast(
        &self,
        file: &Path,
        position: Option<usize>,
        limit: usize,
    ) -> Result<serde_json::Value, LlmError> {
        let file_path = file.to_str().ok_or_else(|| LlmError::SearchFailed {
            reason: format!("File path {:?} is not valid UTF-8", file),
        })?;

        // Check if ast_nodes table exists
        let table_exists: bool = self
            .conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='ast_nodes'",
                [],
                |_| Ok(true),
            )
            .unwrap_or_else(|e| {
                eprintln!("Warning: Failed to check ast_nodes table existence: {}", e);
                false
            });

        if !table_exists {
            return Ok(serde_json::json!({
                "file_path": file_path,
                "count": 0,
                "nodes": [],
            }));
        }

        let nodes = if let Some(pos) = position {
            // Query for node at specific position
            // Join with graph_entities to filter by file path
            let mut stmt = self.conn.prepare(
                "SELECT an.id, an.parent_id, an.kind, an.byte_start, an.byte_end
                 FROM ast_nodes an
                 JOIN graph_entities f ON an.file_id = f.id AND f.kind = 'File'
                 WHERE f.name = ?1
                   AND an.byte_start <= ?2 AND an.byte_end > ?2
                 ORDER BY an.byte_start DESC
                 LIMIT ?3",
            )?;

            let rows = stmt.query_map(params![file_path, pos as i64, limit as i64], |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, i64>(0)?,
                    "parent_id": row.get::<_, Option<i64>>(1)?,
                    "kind": row.get::<_, String>(2)?,
                    "byte_start": row.get::<_, i64>(3)?,
                    "byte_end": row.get::<_, i64>(4)?,
                }))
            })?;

            rows.collect::<Result<Vec<_>, _>>()?
        } else {
            // Query all nodes for the file (join with graph_entities for file info)
            let mut stmt = self.conn.prepare(
                "SELECT an.id, an.parent_id, an.kind, an.byte_start, an.byte_end
                 FROM ast_nodes an
                 JOIN graph_entities f ON an.file_id = f.id AND f.kind = 'File'
                 WHERE f.name = ?1
                 ORDER BY an.byte_start
                 LIMIT ?2",
            )?;

            let rows = stmt.query_map(params![file_path, limit as i64], |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, i64>(0)?,
                    "parent_id": row.get::<_, Option<i64>>(1)?,
                    "kind": row.get::<_, String>(2)?,
                    "byte_start": row.get::<_, i64>(3)?,
                    "byte_end": row.get::<_, i64>(4)?,
                }))
            })?;

            rows.collect::<Result<Vec<_>, _>>()?
        };

        Ok(serde_json::json!({
            "file_path": file_path,
            "count": nodes.len(),
            "nodes": nodes,
        }))
    }

    fn find_ast(&self, kind: &str) -> Result<serde_json::Value, LlmError> {
        // Check if ast_nodes table exists
        let table_exists: bool = self
            .conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='ast_nodes'",
                [],
                |_| Ok(true),
            )
            .unwrap_or_else(|e| {
                eprintln!("Warning: Failed to check ast_nodes table existence: {}", e);
                false
            });

        if !table_exists {
            return Ok(serde_json::json!({
                "kind": kind,
                "count": 0,
                "nodes": [],
            }));
        }

        let mut stmt = self.conn.prepare(
            "SELECT an.id, an.parent_id, an.kind, an.byte_start, an.byte_end, f.name as file_path
             FROM ast_nodes an
             JOIN graph_entities f ON an.file_id = f.id AND f.kind = 'File'
             WHERE an.kind = ?1
             ORDER BY f.name, an.byte_start",
        )?;

        let rows = stmt.query_map(params![kind], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, i64>(0)?,
                "parent_id": row.get::<_, Option<i64>>(1)?,
                "kind": row.get::<_, String>(2)?,
                "byte_start": row.get::<_, i64>(3)?,
                "byte_end": row.get::<_, i64>(4)?,
                "file_path": row.get::<_, String>(5)?,
            }))
        })?;

        let nodes: Vec<_> = rows.collect::<Result<Vec<_>, _>>()?;

        Ok(serde_json::json!({
            "kind": kind,
            "count": nodes.len(),
            "nodes": nodes,
        }))
    }

    fn complete(&self, prefix: &str, limit: usize) -> Result<Vec<String>, LlmError> {
        let like_prefix = format!("{}%", prefix);
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT json_extract(data, '$.display_fqn') AS fqn
             FROM graph_entities
             WHERE kind = 'Symbol'
               AND (fqn LIKE ?1 ESCAPE '\\' OR json_extract(data, '$.fqn') LIKE ?1 ESCAPE '\\')
             ORDER BY fqn
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![like_prefix, limit as i64], |row| {
            row.get::<_, String>(0)
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| LlmError::SearchFailed {
                reason: format!("Failed to complete FQN: {}", e),
            })
    }

    fn lookup(&self, fqn: &str, db_path: &str) -> Result<SymbolMatch, LlmError> {
        let mut stmt = self.conn.prepare(
            "SELECT data, name
             FROM graph_entities
             WHERE kind = 'Symbol'
               AND (json_extract(data, '$.fqn') = ?1
                    OR json_extract(data, '$.canonical_fqn') = ?1
                    OR json_extract(data, '$.display_fqn') = ?1)
             LIMIT 1",
        )?;
        let row = stmt.query_row(params![fqn], |row| {
            let data: String = row.get(0)?;
            let name: String = row.get(1)?;
            Ok((data, name))
        });
        match row {
            Ok((data, _name)) => {
                let file_path: String = json_extract(&data, "file_path")
                    .or_else(|| json_extract(&data, "path"))
                    .unwrap_or_else(|| "<unknown>".to_string());
                let byte_start: u64 = json_extract(&data, "byte_start").unwrap_or(0);
                let byte_end: u64 = json_extract(&data, "byte_end").unwrap_or(0);
                let start_line: u64 = json_extract(&data, "start_line").unwrap_or(0);
                let start_col: u64 = json_extract(&data, "start_col").unwrap_or(0);
                let end_line: u64 = json_extract(&data, "end_line").unwrap_or(0);
                let end_col: u64 = json_extract(&data, "end_col").unwrap_or(0);
                let sym_name: String =
                    json_extract(&data, "name").unwrap_or_else(|| "<unknown>".to_string());
                let kind: String =
                    json_extract(&data, "kind").unwrap_or_else(|| "unknown".to_string());
                let kind_normalized: Option<String> = json_extract(&data, "kind_normalized");
                let symbol_id: Option<String> = json_extract(&data, "symbol_id");
                let sym_fqn: Option<String> = json_extract(&data, "fqn");
                let canonical_fqn: Option<String> = json_extract(&data, "canonical_fqn");
                let display_fqn: Option<String> = json_extract(&data, "display_fqn");

                Ok(SymbolMatch {
                    match_id: format!("sym-{}", symbol_id.as_deref().unwrap_or("unknown")),
                    span: Span {
                        span_id: format!("{}:{}:{}", file_path, byte_start, byte_end),
                        file_path: file_path.clone(),
                        byte_start,
                        byte_end,
                        start_line,
                        start_col,
                        end_line,
                        end_col,
                        context: None,
                    },
                    name: sym_name,
                    kind,
                    parent: None,
                    symbol_id,
                    score: None,
                    fqn: sym_fqn,
                    canonical_fqn,
                    display_fqn,
                    content_hash: None,
                    symbol_kind_from_chunk: None,
                    snippet: None,
                    snippet_truncated: None,
                    language: infer_language(&file_path).map(|s| s.to_string()),
                    kind_normalized,
                    complexity_score: None,
                    fan_in: None,
                    fan_out: None,
                    cyclomatic_complexity: None,
                    ast_context: None,
                    supernode_id: None,
                    coverage: None,
                })
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                let partial = fqn.rsplit("::").next().unwrap_or(fqn);
                Err(LlmError::SymbolNotFound {
                    fqn: fqn.to_string(),
                    db: db_path.to_string(),
                    partial: partial.to_string(),
                })
            }
            Err(e) => Err(LlmError::SearchFailed {
                reason: format!("Failed to lookup symbol: {}", e),
            }),
        }
    }

    fn search_by_label(
        &self,
        _label: &str,
        _limit: usize,
        _db_path: &str,
    ) -> Result<(SearchResponse, bool, bool), LlmError> {
        // Labels are stored in the geometric backend's binary format, not in SQLite tables.
        // Return empty results for SQLite-backed databases.
        Ok((
            SearchResponse {
                query: String::new(),
                total_count: 0,
                results: Vec::new(),
                path_filter: None,
                kind_filter: None,
                notice: None,
            },
            false,
            false,
        ))
    }

    #[cfg(feature = "geometric-backend")]
    fn get_chunks_for_symbol(
        &self,
        file_path: &str,
        symbol_name: &str,
    ) -> Result<Vec<crate::backend::magellan_adapter::CodeChunk>, LlmError> {
        // Check if code_chunks table exists
        let table_exists: bool = self
            .conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='code_chunks'",
                [],
                |_| Ok(true),
            )
            .unwrap_or(false);

        if !table_exists {
            return Err(LlmError::ChunksNotAvailable {
                backend: "SQLite".to_string(),
                message: "code_chunks table not found. Chunking was not enabled during indexing."
                    .to_string(),
            });
        }

        let mut stmt = self.conn.prepare(
            "SELECT id, file_path, byte_start, byte_end, content, content_hash, symbol_name, symbol_kind
             FROM code_chunks
             WHERE file_path = ?1 AND symbol_name = ?2",
        )?;
        let rows = stmt.query_map(params![file_path, symbol_name], |row| {
            Ok(crate::backend::magellan_adapter::CodeChunk {
                id: row.get(0)?,
                file_path: row.get(1)?,
                byte_start: row.get::<_, i64>(2)? as usize,
                byte_end: row.get::<_, i64>(3)? as usize,
                content: row.get(4)?,
                symbol_name: row.get(6)?,
                symbol_kind: row.get(7)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| LlmError::SearchFailed {
                reason: format!("Failed to get chunks: {}", e),
            })
    }
}

/// Extract a value from JSON string using serde_json.
fn json_extract<T: serde::de::DeserializeOwned>(json: &str, key: &str) -> Option<T> {
    serde_json::from_str::<serde_json::Value>(json)
        .ok()?
        .get(key)
        .and_then(|v| serde_json::from_value(v.clone()).ok())
}
