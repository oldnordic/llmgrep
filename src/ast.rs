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

/// AST node context from Magellan's ast_nodes table.
///
/// Contains structural information about a code symbol's position
/// in the Abstract Syntax Tree.
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
        };

        let json = serde_json::to_string(&ctx).unwrap();
        assert!(json.contains("\"ast_id\":123"));
        assert!(json.contains("\"kind\":\"function_item\""));
        assert!(json.contains("\"parent_id\":122"));
        assert!(json.contains("\"byte_start\":100"));
        assert!(json.contains("\"byte_end\":200"));
    }

    #[test]
    fn test_ast_context_without_parent() {
        let ctx = AstContext {
            ast_id: 1,
            kind: "mod_item".to_string(),
            parent_id: None,
            byte_start: 0,
            byte_end: 50,
        };

        let json = serde_json::to_string(&ctx).unwrap();
        assert!(json.contains("\"parent_id\":null"));
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
