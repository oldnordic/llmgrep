//! SQLite backend implementation.
//!
//! SqliteBackend provides the Backend trait implementation for SQLite databases.
//! This is the traditional storage backend and is always available.

use crate::error::LlmError;
use crate::output::{CallSearchResponse, ReferenceSearchResponse, SearchResponse};
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
            .unwrap_or(false);

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
            .unwrap_or(false);

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

    fn complete(&self, _prefix: &str, _limit: usize) -> Result<Vec<String>, LlmError> {
        // SQLite backend cannot efficiently do prefix scans on FQNs
        Err(LlmError::RequiresNativeV3Backend {
            command: "complete".to_string(),
            path: self.db_path.display().to_string(),
        })
    }

    fn lookup(&self, fqn: &str, db_path: &str) -> Result<crate::output::SymbolMatch, LlmError> {
        // SQLite backend cannot efficiently do exact FQN lookups
        // Extract partial name from FQN for error message
        let _partial = fqn.rsplit("::").next().unwrap_or(fqn);
        Err(LlmError::RequiresNativeV3Backend {
            command: "lookup".to_string(),
            path: db_path.to_string(),
        })
    }

    fn search_by_label(
        &self,
        _label: &str,
        _limit: usize,
        db_path: &str,
    ) -> Result<(SearchResponse, bool, bool), LlmError> {
        // SQLite backend does not have label index
        Err(LlmError::RequiresNativeV3Backend {
            command: "search --mode label".to_string(),
            path: db_path.to_string(),
        })
    }

    #[cfg(feature = "geometric-backend")]
    fn get_chunks_for_symbol(
        &self,
        _file_path: &str,
        _symbol_name: &str,
    ) -> Result<Vec<crate::backend::magellan_adapter::CodeChunk>, LlmError> {
        Err(LlmError::ChunksNotAvailable {
            backend: "SQLite".to_string(),
            message: "SQLite backend does not support chunk retrieval. Use Geometric (.geo) backend with chunking enabled.".to_string(),
        })
    }
}
