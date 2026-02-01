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
    let mut stmt = conn.prepare(
        "SELECT name FROM sqlite_master WHERE type='table' AND name='ast_nodes'"
    )?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ast_context_serialization() {
        let ctx = AstContext {
            ast_id: 123,
            kind: "function_item".to_string(),
            parent_id: Some(122),
            byte_start: 100,
            byte_end: 200,
            depth: None,
            parent_kind: None,
            children_count_by_kind: None,
            decision_points: None,
        };

        let json = serde_json::to_string(&ctx).unwrap();
        assert!(json.contains("\"ast_id\":123"));
        assert!(json.contains("\"kind\":\"function_item\""));
        assert!(json.contains("\"parent_id\":122"));
        assert!(json.contains("\"byte_start\":100"));
        assert!(json.contains("\"byte_end\":200"));
        // Enriched fields should not appear in JSON when None (skip_serializing_if)
        assert!(!json.contains("depth"));
        assert!(!json.contains("parent_kind"));
        assert!(!json.contains("children_count_by_kind"));
        assert!(!json.contains("decision_points"));
    }

    #[test]
    fn test_ast_context_without_parent() {
        let ctx = AstContext {
            ast_id: 1,
            kind: "mod_item".to_string(),
            parent_id: None,
            byte_start: 0,
            byte_end: 50,
            depth: None,
            parent_kind: None,
            children_count_by_kind: None,
            decision_points: None,
        };

        let json = serde_json::to_string(&ctx).unwrap();
        assert!(json.contains("\"parent_id\":null"));
    }

    #[test]
    fn test_ast_context_enriched_serialization() {
        let mut children = HashMap::new();
        children.insert("let_declaration".to_string(), 3);
        children.insert("if_expression".to_string(), 2);

        let ctx = AstContext {
            ast_id: 42,
            kind: "function_item".to_string(),
            parent_id: None,
            byte_start: 1000,
            byte_end: 2000,
            depth: Some(0),
            parent_kind: None,
            children_count_by_kind: Some(children),
            decision_points: Some(2),
        };

        let json = serde_json::to_string(&ctx).unwrap();
        // Basic fields
        assert!(json.contains("\"ast_id\":42"));
        assert!(json.contains("\"kind\":\"function_item\""));
        // Enriched fields should appear when set
        assert!(json.contains("\"depth\":0"));
        assert!(json.contains("\"decision_points\":2"));
        assert!(json.contains("\"let_declaration\":3"));
        assert!(json.contains("\"if_expression\":2"));
        // parent_kind should not appear (None)
        assert!(!json.contains("parent_kind"));
    }

    #[test]
    fn test_ast_nodes_table_schema() {
        let schema = ast_nodes_table_schema();
        assert!(schema.contains("CREATE TABLE ast_nodes"));
        assert!(schema.contains("id INTEGER PRIMARY KEY"));
        assert!(schema.contains("parent_id"));
        assert!(schema.contains("kind TEXT NOT NULL"));
        assert!(schema.contains("byte_start INTEGER NOT NULL"));
        assert!(schema.contains("byte_end INTEGER NOT NULL"));
    }

    #[test]
    fn test_check_ast_table_exists_missing() {
        // Test with in-memory database (no tables)
        let conn = Connection::open_in_memory().unwrap();
        let result = check_ast_table_exists(&conn).unwrap();
        assert!(!result, "Should return false when table doesn't exist");
    }

    #[test]
    fn test_check_ast_table_exists_present() {
        // Create database with ast_nodes table
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(ast_nodes_table_schema(), []).unwrap();

        let result = check_ast_table_exists(&conn).unwrap();
        assert!(result, "Should return true when table exists");
    }

    #[test]
    fn test_check_ast_table_exists_with_other_tables() {
        // Create database with other tables but not ast_nodes
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE other_table (id INTEGER PRIMARY KEY)",
            [],
        )
        .unwrap();

        let result = check_ast_table_exists(&conn).unwrap();
        assert!(!result, "Should return false when only other tables exist");
    }
}
