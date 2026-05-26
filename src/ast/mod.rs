//! AST query module for structural code search.
//!
//! This module provides types and functions for working with Magellan's
//! `ast_nodes` table, which contains Abstract Syntax Tree node information
//! for indexed code.
//!
//! # AST Capabilities
//!
//! - Filter search results by AST node kind (function_item, block, etc.)
//! - Include AST context in results (parent_id, byte spans)
//! - Enriched context when --with-ast-context is enabled (depth, parent_kind, children_count, decision_points)
//! - Graceful degradation for databases without ast_nodes table
//!
//! # Table Schema
//!
//! The `ast_nodes` table in Magellan databases has the following schema:
//!
//! ```sql
//! CREATE TABLE ast_nodes (
//!     id INTEGER PRIMARY KEY,
//!     parent_id INTEGER,
//!     kind TEXT NOT NULL,
//!     byte_start INTEGER NOT NULL,
//!     byte_end INTEGER NOT NULL
//! );
//!
//! CREATE INDEX idx_ast_nodes_parent ON ast_nodes(parent_id);
//! CREATE INDEX idx_ast_nodes_kind ON ast_nodes(kind);
//! ```
//!
//! # Available Node Kinds
//!
//! Common AST node kinds include:
//! - `function_item` - Function definitions
//! - `block` - Code blocks { }
//! - `call_expression` - Function calls
//! - `let_declaration` - Variable declarations
//! - `expression_statement` - Expression statements
//! - `attribute_item` - Attributes/macros (#[derive])
//! - `mod_item` - Module declarations
//!
//! # Enriched AST Context
//!
//! When `--with-ast-context` flag is enabled, additional structural information is calculated:
//! - `depth`: Nesting depth from AST root (0 = top-level)
//! - `parent_kind`: Kind of parent AST node
//! - `children_count_by_kind`: Count of direct children grouped by kind
//! - `decision_points`: Number of branching control flow structures
//!
//! # Example
//!
//! ```no_run
//! use llmgrep::ast::{check_ast_table_exists, AstContext};
//! use rusqlite::Connection;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let conn = Connection::open("code.db")?;
//!
//! if check_ast_table_exists(&conn)? {
//!     println!("Database has AST support");
//! }
//! # Ok(())
//! # }
//! ```

use anyhow::Result;
use rusqlite::Connection;
use serde::Serialize;
use std::collections::HashMap;

/// AST node context from Magellan's ast_nodes table.
///
/// Contains structural information about a code symbol's position
/// in the Abstract Syntax Tree.
///
/// Basic fields are always populated when AST data is available.
/// Enriched fields (depth, parent_kind, children_count_by_kind, decision_points)
/// are only populated when `--with-ast-context` flag is used.
#[derive(Debug, Clone, Serialize)]
pub struct AstContext {
    /// AST node ID (matches symbol ID)
    pub ast_id: i64,
    /// Node kind (function_item, block, call_expression, etc.)
    pub kind: String,
    /// Parent AST node ID (None for root nodes)
    pub parent_id: Option<i64>,
    /// Byte start offset within source file
    pub byte_start: u64,
    /// Byte end offset within source file
    pub byte_end: u64,

    // Enriched fields (only populated when --with-ast-context is enabled)
    /// Nesting depth from AST root (0 = top-level, 1 = one level deep, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth: Option<u64>,

    /// Kind of parent AST node (None for root nodes)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_kind: Option<String>,

    /// Count of direct child nodes grouped by kind
    /// Example: {"let_declaration": 3, "if_expression": 2, "call_expression": 5}
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children_count_by_kind: Option<HashMap<String, u64>>,

    /// Number of decision points (branching control flow structures)
    /// Counts: if_expression, match_expression, while_expression, for_expression,
    ///         loop_expression, conditional_expression
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision_points: Option<u64>,
}

/// Check if the ast_nodes table exists in the database.
///
/// Queries sqlite_master to determine if the ast_nodes table is present.
/// This enables graceful degradation for databases created by older
/// versions of Magellan that don't include AST information.
///
/// # Arguments
///
/// * `conn` - SQLite connection
///
/// # Returns
///
/// * `Ok(true)` - Table exists
/// * `Ok(false)` - Table does not exist
/// * `Err(...)` - Database query error
///
/// # Example
///
/// ```no_run
/// use llmgrep::ast::check_ast_table_exists;
/// use rusqlite::Connection;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let conn = Connection::open("code.db")?;
///
/// if check_ast_table_exists(&conn)? {
///     println!("AST filtering available");
/// } else {
///     println!("No AST support in this database");
/// }
/// # Ok(())
/// # }
/// ```
pub fn check_ast_table_exists(conn: &Connection) -> Result<bool> {
    let mut stmt =
        conn.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='ast_nodes'")?;
    Ok(stmt.exists([])?)
}

/// Returns the SQL schema for the ast_nodes table.
///
/// This is provided for documentation and testing purposes.
/// The actual schema is defined by Magellan, not llmgrep.
///
/// # Returns
///
/// SQL CREATE TABLE statement for ast_nodes
pub const fn ast_nodes_table_schema() -> &'static str {
    "CREATE TABLE ast_nodes (
        id INTEGER PRIMARY KEY,
        parent_id INTEGER,
        kind TEXT NOT NULL,
        byte_start INTEGER NOT NULL,
        byte_end INTEGER NOT NULL
    )"
}

/// Calculate the nesting depth of an AST node using recursive CTE.
///
/// Depth is measured by counting ancestors from root nodes (parent_id IS NULL).
/// Root nodes have depth 0, their direct children have depth 1, etc.
///
/// # Arguments
///
/// * `conn` - SQLite connection
/// * `ast_id` - AST node ID to calculate depth for
///
/// # Returns
///
/// * `Ok(Some(depth))` - Depth of the node
/// * `Ok(None)` - Node not found
/// * `Err(...)` - Database error
///
/// # Example
///
/// ```no_run
/// use llmgrep::ast::calculate_ast_depth;
/// use rusqlite::Connection;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let conn = Connection::open("code.db")?;
/// if let Some(depth) = calculate_ast_depth(&conn, 42)? {
///     println!("Node 42 is at depth {}", depth);
/// }
/// # Ok(())
/// # }
/// ```
pub fn calculate_ast_depth(conn: &Connection, ast_id: i64) -> Result<Option<u64>> {
    let sql = r#"
        WITH RECURSIVE node_ancestry AS (
            -- Base case: root nodes (parent_id IS NULL)
            SELECT id, parent_id, 0 as depth
            FROM ast_nodes
            WHERE parent_id IS NULL
            UNION ALL
            -- Recursive case: add 1 to parent depth
            SELECT a.id, a.parent_id, na.depth + 1
            FROM ast_nodes a
            JOIN node_ancestry na ON a.parent_id = na.id
        )
        SELECT depth FROM node_ancestry WHERE id = ?
    "#;

    match conn.query_row(sql, [ast_id], |row| row.get::<_, u64>(0)) {
        Ok(depth) => Ok(Some(depth)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Calculate the decision point depth of an AST node.
///
/// Unlike `calculate_ast_depth` (which counts all ancestors), this counts
/// only decision points (control flow branching structures):
/// - if_expression
/// - match_expression
/// - for_expression
/// - while_expression
/// - loop_expression
///
/// Root level nodes have depth 0. Each decision point ancestor adds 1.
///
/// # Arguments
///
/// * `conn` - SQLite connection
/// * `ast_id` - AST node ID to calculate decision depth for
///
/// # Returns
///
/// * `Ok(Some(depth))` - Decision point depth of the node
/// * `Ok(None)` - Node not found
/// * `Err(...)` - Database error
///
/// # Example
///
/// ```no_run
/// use llmgrep::ast::calculate_decision_depth;
/// use rusqlite::Connection;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let conn = Connection::open("code.db")?;
/// if let Some(depth) = calculate_decision_depth(&conn, 42)? {
///     println!("Node 42 has decision depth {}", depth);
/// }
/// # Ok(())
/// # }
/// ```
pub fn calculate_decision_depth(conn: &Connection, ast_id: i64) -> Result<Option<u64>> {
    let sql = r#"
        WITH RECURSIVE decision_ancestry AS (
            -- Base case: start from the node itself, count 1 if it's a decision point
            SELECT id, parent_id,
                   CASE WHEN kind IN (
                       'if_expression', 'match_expression', 'for_expression',
                       'while_expression', 'loop_expression'
                   ) THEN 1 ELSE 0 END as depth
            FROM ast_nodes
            WHERE id = ?
            UNION ALL
            -- Recursive case: traverse to parent, add 1 if parent is a decision point
            SELECT a.id, a.parent_id,
                   da.depth + CASE WHEN a.kind IN (
                       'if_expression', 'match_expression', 'for_expression',
                       'while_expression', 'loop_expression'
                   ) THEN 1 ELSE 0 END
            FROM ast_nodes a
            JOIN decision_ancestry da ON a.id = da.parent_id
            WHERE a.parent_id IS NOT NULL
        )
        SELECT MAX(depth) FROM decision_ancestry
    "#;

    match conn.query_row(sql, [ast_id], |row| row.get::<_, u64>(0)) {
        Ok(depth) => Ok(Some(depth)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Get the kind of an AST node's parent.
///
/// # Arguments
///
/// * `conn` - SQLite connection
/// * `parent_id` - Parent node ID (None returns None immediately)
///
/// # Returns
///
/// * `Ok(Some(kind))` - Kind string of parent node
/// * `Ok(None)` - No parent or parent not found
/// * `Err(...)` - Database error
pub fn get_parent_kind(conn: &Connection, parent_id: Option<i64>) -> Result<Option<String>> {
    let Some(pid) = parent_id else {
        return Ok(None);
    };
    let sql = "SELECT kind FROM ast_nodes WHERE id = ?";

    match conn.query_row(sql, [pid], |row| row.get::<_, String>(0)) {
        Ok(kind) => Ok(Some(kind)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Count direct children of an AST node grouped by kind.
///
/// Returns a HashMap where keys are node kinds (e.g., "let_declaration",
/// "if_expression") and values are the count of children of that kind.
///
/// # Arguments
///
/// * `conn` - SQLite connection
/// * `ast_id` - AST node ID to count children for
///
/// # Returns
///
/// * `Ok(HashMap)` - Map of kind to count
/// * `Err(...)` - Database error
pub fn count_children_by_kind(conn: &Connection, ast_id: i64) -> Result<HashMap<String, u64>> {
    let sql = r#"
        SELECT kind, COUNT(*) as count
        FROM ast_nodes
        WHERE parent_id = ?
        GROUP BY kind
    "#;

    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([ast_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, u64>(1)?))
    })?;

    let mut counts = HashMap::new();
    for row in rows {
        let (kind, count) = row?;
        counts.insert(kind, count);
    }
    Ok(counts)
}

/// Count decision points within an AST node's direct children.
///
/// Decision points are branching control flow structures:
/// - if_expression
/// - match_expression
/// - while_expression
/// - for_expression
/// - loop_expression
/// - conditional_expression (ternary)
///
/// # Arguments
///
/// * `conn` - SQLite connection
/// * `ast_id` - AST node ID to count decision points for
///
/// # Returns
///
/// * `Ok(count)` - Number of decision point children
/// * `Err(...)` - Database error
pub fn count_decision_points(conn: &Connection, ast_id: i64) -> Result<u64> {
    let sql = r#"
        SELECT COUNT(*) FROM ast_nodes
        WHERE parent_id = ?
          AND kind IN (
              'if_expression', 'match_expression', 'while_expression',
              'for_expression', 'loop_expression', 'conditional_expression'
          )
    "#;

    conn.query_row(sql, [ast_id], |row| row.get(0))
        .map_err(Into::into)
}

/// Get full AST context for a symbol by finding its overlapping AST node.
///
/// This function finds the AST node that overlaps with the symbol's byte span
/// and optionally populates enriched fields (depth, parent_kind, children, decision_points).
///
/// # Arguments
///
/// * `conn` - SQLite connection
/// * `file_path` - Path to source file
/// * `byte_start` - Symbol's start byte offset
/// * `byte_end` - Symbol's end byte offset
/// * `include_enriched` - Whether to calculate enriched fields
///
/// # Returns
///
/// * `Ok(Some(ctx))` - AST context for the symbol
/// * `Ok(None)` - No matching AST node found
/// * `Err(...)` - Database error
///
/// # Finding Strategy
///
/// The function finds AST nodes where the symbol's span overlaps with the node's span.
/// When `preferred_kinds` is provided, nodes of those kinds are prioritized.
/// Otherwise, the node with minimal distance is selected.
pub fn get_ast_context_for_symbol(
    conn: &Connection,
    _file_path: &str,
    byte_start: u64,
    byte_end: u64,
    include_enriched: bool,
) -> Result<Option<AstContext>> {
    get_ast_context_for_symbol_with_preference(
        conn,
        _file_path,
        byte_start,
        byte_end,
        include_enriched,
        &[],
    )
}

/// Get AST context for a symbol with preferred kinds.
///
/// When `preferred_kinds` is non-empty, this function first looks for AST nodes
/// matching those kinds before falling back to any overlapping node.
pub fn get_ast_context_for_symbol_with_preference(
    conn: &Connection,
    _file_path: &str,
    byte_start: u64,
    byte_end: u64,
    include_enriched: bool,
    preferred_kinds: &[String],
) -> Result<Option<AstContext>> {
    // Overlap formula: intervals [s1, e1] and [s2, e2] overlap when: s1 < e2 AND s2 < e1
    let (ast_id, parent_id, kind, ast_byte_start, ast_byte_end) = if !preferred_kinds.is_empty() {
        // First try to find a node matching one of the preferred kinds
        let placeholders = preferred_kinds
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT id, parent_id, kind, byte_start, byte_end
                 FROM ast_nodes
                 WHERE byte_start <= ? AND byte_end >= ? AND kind IN ({})
                 ORDER BY ABS(byte_start - ?) + ABS(byte_end - ?)
                 LIMIT 1",
            placeholders
        );

        let byte_end_i64 = byte_end as i64;
        let byte_start_i64 = byte_start as i64;
        let mut params: Vec<&dyn rusqlite::ToSql> = vec![&byte_end_i64, &byte_start_i64];
        for kind in preferred_kinds {
            params.push(kind);
        }
        params.push(&byte_start_i64);
        params.push(&byte_end_i64);

        match conn.query_row(&sql, params.as_slice(), |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Option<i64>>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, u64>(3)?,
                row.get::<_, u64>(4)?,
            ))
        }) {
            Ok(result) => result,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                // No preferred kind found, fall back to any overlapping node
                let fallback_sql = r#"
                        SELECT id, parent_id, kind, byte_start, byte_end
                        FROM ast_nodes
                        WHERE byte_start <= ? AND byte_end >= ?
                        ORDER BY ABS(byte_start - ?) + ABS(byte_end - ?)
                        LIMIT 1
                    "#;
                match conn.query_row(
                    fallback_sql,
                    [
                        byte_end as i64,
                        byte_start as i64,
                        byte_start as i64,
                        byte_end as i64,
                    ],
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, Option<i64>>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, u64>(3)?,
                            row.get::<_, u64>(4)?,
                        ))
                    },
                ) {
                    Ok(result) => result,
                    Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
                    Err(e) => return Err(e.into()),
                }
            }
            Err(e) => return Err(e.into()),
        }
    } else {
        // No preference, find the closest containing node
        // Prefer nodes that fully contain the symbol span, ordered by smallest containing span first
        let sql = r#"
            SELECT id, parent_id, kind, byte_start, byte_end
            FROM ast_nodes
            WHERE byte_start <= ? AND byte_end >= ?
            ORDER BY
                CASE WHEN byte_start <= ? AND byte_end >= ? THEN 0 ELSE 1 END ASC,
                (byte_end - byte_start) ASC
            LIMIT 1
        "#;
        match conn.query_row(
            sql,
            [
                byte_end as i64,
                byte_start as i64,
                byte_start as i64,
                byte_end as i64,
            ],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, u64>(3)?,
                    row.get::<_, u64>(4)?,
                ))
            },
        ) {
            Ok(result) => result,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(e) => return Err(e.into()),
        }
    };

    let mut ctx = AstContext {
        ast_id,
        kind,
        parent_id,
        byte_start: ast_byte_start,
        byte_end: ast_byte_end,
        depth: None,
        parent_kind: None,
        children_count_by_kind: None,
        decision_points: None,
    };

    if include_enriched {
        // Populate enriched fields when requested
        ctx.depth = Some(calculate_ast_depth(conn, ast_id)?.unwrap_or(0));
        ctx.parent_kind = get_parent_kind(conn, parent_id)?;
        ctx.children_count_by_kind = Some(count_children_by_kind(conn, ast_id)?);
        ctx.decision_points = Some(count_decision_points(conn, ast_id)?);
    }

    Ok(Some(ctx))
}

pub use language::{
    expand_shorthand, expand_shorthand_with_language, expand_shorthands,
    get_node_kinds_for_language, get_supported_languages, LanguageNodeKinds, AST_SHORTHANDS,
    JAVASCRIPT_NODE_KINDS, PYTHON_NODE_KINDS, TYPESCRIPT_NODE_KINDS,
};

mod language;
#[cfg(test)]
mod tests;
