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

/// Shorthand mappings for common AST node kind groups.
///
/// These shorthands allow users to query groups of related AST nodes
/// without having to list each kind individually. For example:
/// - `--ast-kind loops` expands to `for_expression,while_expression,loop_expression`
/// - `--ast-kind functions` expands to `function_item,closure_expression`
///
/// # Rust Shorthands
///
/// These are the node kinds for Rust code (tree-sitter-rust):
pub static AST_SHORTHANDS: &[(&str, &str)] = &[
    ("loops", "for_expression,while_expression,loop_expression"),
    ("conditionals", "if_expression,match_expression,match_arm"),
    ("functions", "function_item,closure_expression,async_function_item"),
    ("declarations", "struct_item,enum_item,let_declaration,const_item,static_item,type_alias_item"),
    ("unsafe", "unsafe_block"),
    ("types", "struct_item,enum_item,type_alias_item,union_item"),
    ("macros", "macro_invocation,macro_definition,macro_rule"),
    ("mods", "mod_item"),
    ("traits", "trait_item,trait_impl_item"),
    ("impls", "impl_item"),
];

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
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, u64>(1)?,
        ))
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
    get_ast_context_for_symbol_with_preference(conn, _file_path, byte_start, byte_end, include_enriched, &[])
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
    let (ast_id, parent_id, kind, ast_byte_start, ast_byte_end) =
        if !preferred_kinds.is_empty() {
            // First try to find a node matching one of the preferred kinds
            // Use direct SQL construction to avoid rusqlite parameter binding issues
            let kind_list = preferred_kinds.iter()
                .map(|k| format!("'{}'", k.replace('\'', "''")))
                .collect::<Vec<_>>()
                .join(",");
            let sql = format!(
                "SELECT id, parent_id, kind, byte_start, byte_end
                 FROM ast_nodes
                 WHERE byte_start <= {} AND byte_end >= {} AND kind IN ({})
                 ORDER BY ABS(byte_start - {}) + ABS(byte_end - {})
                 LIMIT 1",
                byte_end, byte_start, kind_list, byte_start, byte_end
            );

            match conn.query_row(
                &sql,
                [],
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
                        [byte_end as i64, byte_start as i64, byte_start as i64, byte_end as i64],
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
                },
                Err(e) => return Err(e.into()),
            }
        } else {
            // No preference, find the closest containing node
            // Prefer nodes that fully contain the symbol span, ordered by smallest containing span first
            let sql = format!(
                "SELECT id, parent_id, kind, byte_start, byte_end
                 FROM ast_nodes
                 WHERE byte_start <= {} AND byte_end >= {}
                 ORDER BY
                     CASE WHEN byte_start <= {} AND byte_end >= {} THEN 0 ELSE 1 END ASC,
                     (byte_end - byte_start) ASC
                 LIMIT 1",
                byte_end, byte_start, byte_start, byte_end
            );
            match conn.query_row(
                &sql,
                [],
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

/// Language-specific node kind mappings for shorthands.
///
/// Each language has its own set of AST node kinds from tree-sitter grammars.
/// This structure maps shorthand names like "loops", "functions", etc. to
/// language-specific node kinds.
#[derive(Debug, Clone)]
pub struct LanguageNodeKinds {
    /// Language identifier (rust, python, javascript, typescript)
    pub language: &'static str,
    /// Loop constructs
    pub loops: &'static [&'static str],
    /// Conditional/branching constructs
    pub conditionals: &'static [&'static str],
    /// Functions and callable definitions
    pub functions: &'static [&'static str],
    /// Type/declaration constructs
    pub declarations: &'static [&'static str],
}

/// Node kind mappings for Python (tree-sitter-python)
pub static PYTHON_NODE_KINDS: LanguageNodeKinds = LanguageNodeKinds {
    language: "python",
    loops: &["for_statement", "while_statement"],
    conditionals: &["if_statement", "match_statement"],
    functions: &["function_definition", "lambda", "async_function_definition"],
    declarations: &["class_definition", "type_alias_statement"],
};

/// Node kind mappings for JavaScript (tree-sitter-javascript)
pub static JAVASCRIPT_NODE_KINDS: LanguageNodeKinds = LanguageNodeKinds {
    language: "javascript",
    loops: &["for_statement", "for_in_statement", "for_of_statement", "while_statement", "do_statement"],
    conditionals: &["if_statement", "switch_statement", "catch_clause"],
    functions: &["function_declaration", "function_expression", "arrow_function", "generator_function_declaration", "generator_function_expression"],
    declarations: &["class_declaration", "class_expression", "variable_declaration", "type_alias_declaration"],
};

/// Node kind mappings for TypeScript (tree-sitter-typescript)
pub static TYPESCRIPT_NODE_KINDS: LanguageNodeKinds = LanguageNodeKinds {
    language: "typescript",
    loops: &["for_statement", "for_in_statement", "for_of_statement", "while_statement", "do_statement"],
    conditionals: &["if_statement", "switch_statement", "catch_clause"],
    functions: &["function_declaration", "function_expression", "arrow_function", "generator_function_declaration", "generator_function_expression"],
    declarations: &["class_declaration", "class_expression", "variable_declaration", "type_alias_declaration", "interface_declaration", "enum_declaration"],
};

/// Get all supported languages for AST node kind expansion.
///
/// Returns a slice of language identifiers that have specific node kind mappings.
pub fn get_supported_languages() -> &'static [&'static str] {
    &["rust", "python", "javascript", "typescript"]
}

/// Get node kinds for a specific language and shorthand category.
///
/// # Arguments
///
/// * `language` - Language identifier (rust, python, javascript, typescript)
/// * `category` - Shorthand category (loops, conditionals, functions, declarations)
///
/// # Returns
///
/// * `Some(kinds)` - Slice of node kind strings for the category
/// * `None` - Language or category not found
///
/// # Example
///
/// ```
/// use llmgrep::ast::get_node_kinds_for_language;
///
/// let python_funcs = get_node_kinds_for_language("python", "functions");
/// assert!(python_funcs.is_some());
/// assert!(python_funcs.unwrap().iter().any(|s| s == "function_definition"));
/// ```
pub fn get_node_kinds_for_language(
    language: &str,
    category: &str,
) -> Option<Vec<String>> {
    let kinds = match language.to_lowercase().as_str() {
        "python" => {
            let mapping = &PYTHON_NODE_KINDS;
            match category.to_lowercase().as_str() {
                "loops" => mapping.loops,
                "conditionals" => mapping.conditionals,
                "functions" => mapping.functions,
                "declarations" => mapping.declarations,
                _ => return None,
            }
        }
        "javascript" => {
            let mapping = &JAVASCRIPT_NODE_KINDS;
            match category.to_lowercase().as_str() {
                "loops" => mapping.loops,
                "conditionals" => mapping.conditionals,
                "functions" => mapping.functions,
                "declarations" => mapping.declarations,
                _ => return None,
            }
        }
        "typescript" => {
            let mapping = &TYPESCRIPT_NODE_KINDS;
            match category.to_lowercase().as_str() {
                "loops" => mapping.loops,
                "conditionals" => mapping.conditionals,
                "functions" => mapping.functions,
                "declarations" => mapping.declarations,
                _ => return None,
            }
        }
        _ => return None,
    };
    Some(kinds.iter().map(|s| s.to_string()).collect())
}

/// Expand a single shorthand to its full node kind list.
///
/// If the input is a known shorthand (like "loops", "functions"), returns
/// the expanded comma-separated list. Otherwise, returns the input as-is
/// (it might be a specific node kind like "function_item").
///
/// # Arguments
///
/// * `input` - Shorthand or specific node kind
///
/// # Returns
///
/// Expanded node kinds as a comma-separated string
///
/// # Example
///
/// ```
/// use llmgrep::ast::expand_shorthand;
///
/// assert_eq!(expand_shorthand("loops"), "for_expression,while_expression,loop_expression");
/// assert_eq!(expand_shorthand("function_item"), "function_item"); // Not a shorthand, passed through
/// ```
pub fn expand_shorthand(input: &str) -> String {
    let normalized = input.trim().to_lowercase();
    for &(shorthand, expansion) in AST_SHORTHANDS {
        if normalized == shorthand {
            return expansion.to_string();
        }
    }
    // Not a shorthand, return as-is (might be a specific node kind)
    input.to_string()
}

/// Expand multiple shorthands from a comma-separated input.
///
/// Splits the input by commas, expands each part, and returns a deduplicated
/// list of node kinds. This allows combining shorthands with specific kinds:
/// `loops,function_item` expands to all loop kinds plus `function_item`.
///
/// # Arguments
///
/// * `input` - Comma-separated shorthands and/or specific node kinds
///
/// # Returns
///
/// Deduplicated vector of expanded node kind strings
///
/// # Example
///
/// ```
/// use llmgrep::ast::expand_shorthands;
///
/// let kinds = expand_shorthands("loops,conditionals");
/// assert!(kinds.contains(&"for_expression".to_string()));
/// assert!(kinds.contains(&"if_expression".to_string()));
///
/// let mixed = expand_shorthands("loops,function_item");
/// assert!(mixed.contains(&"for_expression".to_string()));
/// assert!(mixed.contains(&"function_item".to_string()));
/// ```
pub fn expand_shorthands(input: &str) -> Vec<String> {
    let mut result = std::collections::HashSet::new();

    for part in input.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let expanded = expand_shorthand(part);
        // The expanded result might itself be comma-separated
        for kind in expanded.split(',') {
            let kind = kind.trim();
            if !kind.is_empty() {
                result.insert(kind.to_string());
            }
        }
    }

    // Convert to sorted Vec for consistent ordering
    let mut kinds: Vec<String> = result.into_iter().collect();
    kinds.sort();
    kinds
}

/// Expand a shorthand with language-aware node kind selection.
///
/// When a language is specified, returns language-specific node kinds
/// for the given shorthand. Falls back to Rust shorthands if language
/// is not recognized.
///
/// # Arguments
///
/// * `shorthand` - Shorthand name (loops, functions, etc.)
/// * `language` - Optional language identifier
///
/// # Returns
///
/// Vector of expanded node kind strings
pub fn expand_shorthand_with_language(
    shorthand: &str,
    language: Option<&str>,
) -> Vec<String> {
    let normalized = shorthand.trim().to_lowercase();

    if let Some(lang) = language {
        let lang_lower = lang.to_lowercase();

        // Check if this is a known category that has language-specific mappings
        if let Some(kinds) = get_node_kinds_for_language(&lang_lower, &normalized) {
            return kinds;
        }
    }

    // Fall back to Rust shorthands
    let expanded = expand_shorthand(&normalized);
    expanded.split(',').map(|s| s.trim().to_string()).collect()
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

    #[test]
    fn test_calculate_decision_depth() {
        use super::*;

        let conn = Connection::open_in_memory().unwrap();
        conn.execute(ast_nodes_table_schema(), []).unwrap();

        // Create a tree structure with decision points:
        // id=1: mod_item (parent_id=NULL) -> decision depth 0 (not a decision point)
        // id=2: function_item (parent_id=1) -> decision depth 0
        // id=3: if_expression (parent_id=2) -> decision depth 1
        // id=4: loop_expression (parent_id=3) -> decision depth 2
        // id=5: let_declaration (parent_id=4) -> decision depth 2 (not a decision point)
        // id=6: match_expression (parent_id=5) -> decision depth 3
        conn.execute(
            "INSERT INTO ast_nodes (id, parent_id, kind, byte_start, byte_end) VALUES
            (1, NULL, 'mod_item', 0, 1000),
            (2, 1, 'function_item', 100, 900),
            (3, 2, 'if_expression', 150, 800),
            (4, 3, 'loop_expression', 200, 700),
            (5, 4, 'let_declaration', 250, 600),
            (6, 5, 'match_expression', 300, 500)",
            [],
        )
        .unwrap();

        // Test decision depth calculation
        assert_eq!(
            calculate_decision_depth(&conn, 1).unwrap().unwrap(),
            0,
            "mod_item at root should have decision depth 0"
        );
        assert_eq!(
            calculate_decision_depth(&conn, 2).unwrap().unwrap(),
            0,
            "function_item (child of mod) should have decision depth 0"
        );
        assert_eq!(
            calculate_decision_depth(&conn, 3).unwrap().unwrap(),
            1,
            "if_expression should have decision depth 1"
        );
        assert_eq!(
            calculate_decision_depth(&conn, 4).unwrap().unwrap(),
            2,
            "loop_expression (child of if) should have decision depth 2"
        );
        assert_eq!(
            calculate_decision_depth(&conn, 5).unwrap().unwrap(),
            2,
            "let_declaration (child of loop) should have decision depth 2"
        );
        assert_eq!(
            calculate_decision_depth(&conn, 6).unwrap().unwrap(),
            3,
            "match_expression (child of let) should have decision depth 3"
        );
    }
}
