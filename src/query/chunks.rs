//! Code chunk querying from Magellan's code_chunks table.
//!
//! This module provides functions for querying pre-extracted code chunks
//! stored during Magellan indexing.

use crate::error::LlmError;
use rusqlite::Connection;

/// Code chunk from Magellan's code_chunks table.
///
/// Represents pre-extracted code content with SHA-256 hash for deduplication.
/// Chunks are created during Magellan indexing and provide faster snippet retrieval
/// than file I/O.
#[derive(Debug, Clone)]
pub struct CodeChunk {
    /// Database row ID
    pub id: i64,
    /// Absolute path to the source file
    pub file_path: String,
    /// Byte offset from file start (inclusive)
    pub byte_start: u64,
    /// Byte offset from file start (exclusive)
    pub byte_end: u64,
    /// The source code content
    pub content: String,
    /// SHA-256 hash of the content for deduplication
    pub content_hash: String,
    /// Name of the symbol this chunk belongs to (if available)
    pub symbol_name: Option<String>,
    /// Kind of the symbol (e.g., "function_item", "struct_item")
    pub symbol_kind: Option<String>,
}

/// Query the code_chunks table by symbol name
///
/// Searches for chunks associated with a specific symbol name.
/// Returns all matching chunks since a symbol may have multiple chunks.
pub fn search_chunks_by_symbol_name(
    conn: &Connection,
    symbol_name: &str,
) -> Result<Vec<CodeChunk>, LlmError> {
    let sql = r#"
        SELECT id, file_path, byte_start, byte_end, content, content_hash, symbol_name, symbol_kind
        FROM code_chunks
        WHERE symbol_name = ?
    "#;

    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([symbol_name], |row| {
        Ok(CodeChunk {
            id: row.get(0)?,
            file_path: row.get(1)?,
            byte_start: row.get(2)?,
            byte_end: row.get(3)?,
            content: row.get(4)?,
            content_hash: row.get(5)?,
            symbol_name: row.get(6)?,
            symbol_kind: row.get(7)?,
        })
    })?;

    let mut chunks = Vec::new();
    for row in rows {
        chunks.push(row?);
    }
    Ok(chunks)
}

/// Query the code_chunks table by span
///
/// Searches for a chunk at the exact file path and byte range.
/// Returns None if no chunk exists for this span.
pub fn search_chunks_by_span(
    conn: &Connection,
    file_path: &str,
    byte_start: u64,
    byte_end: u64,
) -> Result<Option<CodeChunk>, LlmError> {
    let sql = r#"
        SELECT id, file_path, byte_start, byte_end, content, content_hash, symbol_name, symbol_kind
        FROM code_chunks
        WHERE file_path = ? AND byte_start = ? AND byte_end = ?
        LIMIT 1
    "#;

    let mut stmt = conn.prepare(sql)?;
    let mut rows = stmt.query_map(
        rusqlite::params![file_path, byte_start as i64, byte_end as i64],
        |row| {
            Ok(CodeChunk {
                id: row.get(0)?,
                file_path: row.get(1)?,
                byte_start: row.get(2)?,
                byte_end: row.get(3)?,
                content: row.get(4)?,
                content_hash: row.get(5)?,
                symbol_name: row.get(6)?,
                symbol_kind: row.get(7)?,
            })
        },
    )?;

    match rows.next() {
        Some(Ok(chunk)) => Ok(Some(chunk)),
        Some(Err(e)) => Err(LlmError::from(e)),
        None => Ok(None),
    }
}
