//! Algorithm integration module for Magellan 2.0 graph algorithms.
//!
//! This module provides integration with Magellan's executable graph algorithms
//! through a shell-out pattern, and symbol resolution via the library API.
//! It supports:
//!
//! - Loading pre-computed SymbolSet files from JSON
//! - Running Magellan algorithms (reachable, dead-code, slice, cycles) and extracting SymbolIds
//! - Resolving simple symbol names to SymbolIds via magellan's `SymbolNavigator`
//!
//! The SymbolSet type is the core abstraction for algorithm-based filtering.
//! It represents a collection of SymbolIds (32-char BLAKE3 hashes) that can be
//! used to filter search results.

use crate::error::LlmError;
use magellan::CodeGraph;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

/// SymbolSet - a collection of SymbolIds for filtering search results.
///
/// This type represents a set of symbols identified by their 32-character
/// BLAKE3 hash SymbolIds. It can be loaded from JSON files or extracted
/// from Magellan algorithm outputs.
///
/// # JSON Format
///
/// ```json
/// {
///   "symbol_ids": [
///     "abc123def456789012345678901234ab",
///     "def456789012345678901234abcd1234",
///     ...
///   ]
/// }
/// ```
///
/// # Example
///
/// ```no_run
/// use llmgrep::algorithm::SymbolSet;
/// use std::path::Path;
///
/// let symbol_set = SymbolSet::from_file(Path::new("symbols.json"))?;
/// symbol_set.validate()?;
/// # Ok::<(), llmgrep::error::LlmError>(())
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolSet {
    /// List of SymbolIds (32-char BLAKE3 hashes)
    pub symbol_ids: Vec<String>,
}

impl SymbolSet {
    /// Load SymbolSet from a JSON file.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the JSON file containing SymbolSet data
    ///
    /// # Returns
    ///
    /// A validated SymbolSet if the file exists and contains valid JSON.
    ///
    /// # Errors
    ///
    /// Returns `LlmError::IoError` if the file cannot be read.
    /// Returns `LlmError::JsonError` if the JSON is invalid or malformed.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use llmgrep::algorithm::SymbolSet;
    /// use std::path::Path;
    ///
    /// let symbol_set = SymbolSet::from_file(Path::new("reachable.json"))?;
    /// # Ok::<(), llmgrep::error::LlmError>(())
    /// ```
    pub fn from_file(path: &Path) -> Result<Self, LlmError> {
        let content = std::fs::read_to_string(path).map_err(LlmError::IoError)?;
        serde_json::from_str(&content).map_err(LlmError::JsonError)
    }

    /// Validate that all SymbolIds are in the correct format (32 hex characters).
    ///
    /// Magellan SymbolIds are 32-character BLAKE3 hashes represented as lowercase
    /// hexadecimal strings. This method validates that format.
    ///
    /// # Returns
    ///
    /// `Ok(())` if all SymbolIds are valid.
    ///
    /// # Errors
    ///
    /// Returns `LlmError::InvalidQuery` if any SymbolId has an invalid format.
    ///
    /// # Example
    ///
    /// ```
    /// use llmgrep::algorithm::SymbolSet;
    ///
    /// let symbol_set = SymbolSet {
    ///     symbol_ids: vec![
    ///         "abc123def456789012345678901234ab".to_string(),
    ///     ],
    /// };
    /// assert!(symbol_set.validate().is_ok());
    /// ```
    pub fn validate(&self) -> Result<(), LlmError> {
        for symbol_id in &self.symbol_ids {
            if symbol_id.len() != 32 {
                return Err(LlmError::InvalidQuery {
                    query: format!(
                        "Invalid SymbolId format: '{}'. Expected 32 hex characters.",
                        symbol_id
                    ),
                });
            }
            if !symbol_id.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(LlmError::InvalidQuery {
                    query: format!(
                        "Invalid SymbolId format: '{}'. Expected only hexadecimal characters.",
                        symbol_id
                    ),
                });
            }
        }
        Ok(())
    }

    /// Check if the SymbolSet is empty.
    pub fn is_empty(&self) -> bool {
        self.symbol_ids.is_empty()
    }

    /// Return the number of SymbolIds in the set.
    pub fn len(&self) -> usize {
        self.symbol_ids.len()
    }
}

/// Algorithm-based filtering options for search
#[derive(Debug, Clone, Default)]
pub struct AlgorithmOptions<'a> {
    /// Load pre-computed SymbolSet from JSON file
    pub from_symbol_set: Option<&'a str>,
    /// One-shot: reachable from symbol (shell-out to magellan reachable)
    pub reachable_from: Option<&'a str>,
    /// One-shot: dead code from entry point (shell-out to magellan dead-code)
    pub dead_code_in: Option<&'a str>,
    /// One-shot: symbols in cycle (shell-out to magellan cycles)
    pub in_cycle: Option<&'a str>,
    /// One-shot: backward slice from target (shell-out to magellan slice)
    pub slice_backward_from: Option<&'a str>,
    /// One-shot: forward slice from target (shell-out to magellan slice)
    pub slice_forward_from: Option<&'a str>,
    /// One-shot: condense SCC detection (shell-out to magellan condense)
    pub condense: bool,
    /// One-shot: paths from start symbol (shell-out to magellan paths)
    pub paths_from: Option<&'a str>,
    /// Optional end symbol for path enumeration (shell-out to magellan paths --end)
    pub paths_to: Option<&'a str>,
}

impl<'a> AlgorithmOptions<'a> {
    /// Check if any algorithm filter is active
    pub fn is_active(&self) -> bool {
        self.from_symbol_set.is_some()
            || self.reachable_from.is_some()
            || self.dead_code_in.is_some()
            || self.in_cycle.is_some()
            || self.slice_backward_from.is_some()
            || self.slice_forward_from.is_some()
            || self.condense
            || self.paths_from.is_some()
    }

    /// Create empty AlgorithmOptions
    pub fn new() -> Self {
        Self::default()
    }
}

/// Result type for algorithm filtering operations.
///
/// Contains:
/// - `Vec<String>` of SymbolIds for filtering
/// - `HashMap<String, String>` mapping symbol_id -> supernode_id for decoration
/// - `bool` indicating if path enumeration hit bounds
pub type AlgorithmFilterResult = Result<(Vec<String>, HashMap<String, String>, bool), LlmError>;

/// Apply algorithm filters and return SymbolSet for search filtering
///
/// Handles:
/// - Pre-computed SymbolSet from file (--from-symbol-set)
/// - One-shot algorithm execution (--reachable-from, --dead-code-in, etc.)
/// - FQN resolution for simple names (resolves to SymbolId before shelling out)
///
/// Returns: (`Vec<String>` of SymbolIds, `HashMap<String, String>` of symbol_id -> supernode_id, `bool` paths_bounded)
///         All empty if no active filters
pub fn apply_algorithm_filters(
    db_path: &Path,
    options: &AlgorithmOptions<'_>,
) -> AlgorithmFilterResult {
    // Priority 1: Pre-computed SymbolSet from file
    if let Some(file_path) = options.from_symbol_set {
        let symbol_set = magellan_bridge::parse_symbol_set_file(Path::new(file_path))?;
        symbol_set.validate()?;
        return Ok((symbol_set.symbol_ids, HashMap::new(), false));
    }

    // Priority 2: One-shot algorithm execution (only one allowed)
    // Check for exactly one active one-shot filter
    let active_count = [
        options.reachable_from.is_some(),
        options.dead_code_in.is_some(),
        options.in_cycle.is_some(),
        options.slice_backward_from.is_some(),
        options.slice_forward_from.is_some(),
        options.condense,
        options.paths_from.is_some(),
    ]
    .iter()
    .filter(|&&x| x)
    .count();

    if active_count > 1 {
        return Err(LlmError::InvalidQuery {
            query: "Only one one-shot algorithm filter may be specified. Use --from-symbol-set for composed filters.".to_string(),
        });
    }

    // Execute the active one-shot algorithm
    if let Some(symbol) = options.reachable_from {
        let symbol_id = resolve_fqn_to_symbol_id(db_path, symbol)?;
        let args = ["--from", &symbol_id];
        return Ok((
            magellan_bridge::run_magellan_algorithm(db_path, "reachable", &args)?.symbol_ids,
            HashMap::new(),
            false,
        ));
    }

    if let Some(symbol) = options.dead_code_in {
        let symbol_id = resolve_fqn_to_symbol_id(db_path, symbol)?;
        let args = ["--entry", &symbol_id];
        return Ok((
            magellan_bridge::run_magellan_algorithm(db_path, "dead-code", &args)?.symbol_ids,
            HashMap::new(),
            false,
        ));
    }

    if let Some(symbol) = options.in_cycle {
        let symbol_id = resolve_fqn_to_symbol_id(db_path, symbol)?;
        let args = ["--symbol", &symbol_id];
        return Ok((
            magellan_bridge::run_magellan_algorithm(db_path, "cycles", &args)?.symbol_ids,
            HashMap::new(),
            false,
        ));
    }

    if let Some(symbol) = options.slice_backward_from {
        let symbol_id = resolve_fqn_to_symbol_id(db_path, symbol)?;
        let args = ["--target", &symbol_id, "--direction", "backward"];
        return Ok((
            magellan_bridge::run_magellan_algorithm(db_path, "slice", &args)?.symbol_ids,
            HashMap::new(),
            false,
        ));
    }

    if let Some(symbol) = options.slice_forward_from {
        let symbol_id = resolve_fqn_to_symbol_id(db_path, symbol)?;
        let args = ["--target", &symbol_id, "--direction", "forward"];
        return Ok((
            magellan_bridge::run_magellan_algorithm(db_path, "slice", &args)?.symbol_ids,
            HashMap::new(),
            false,
        ));
    }

    // Condense SCC detection
    if options.condense {
        let output = Command::new("magellan")
            .args([
                "condense",
                "--db",
                &db_path.to_string_lossy(),
                "--output",
                "json",
            ])
            .output()
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => LlmError::MagellanNotFound,
                _ => LlmError::MagellanExecutionFailed {
                    algorithm: "condense".to_string(),
                    stderr: format!(
                        "{}\n\nTry running: magellan condense --db {} for more details",
                        e,
                        db_path.to_string_lossy()
                    ),
                },
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(LlmError::MagellanExecutionFailed {
                algorithm: "condense".to_string(),
                stderr: format!(
                    "{}\n\nTry running: magellan condense --db {} for more details",
                    stderr,
                    db_path.to_string_lossy()
                ),
            });
        }

        let json = String::from_utf8_lossy(&output.stdout);
        let (symbol_ids, supernode_map) = magellan_bridge::parse_condense_output(&json)?;

        // Return both symbol_ids for filtering and supernode_map for output decoration
        // Condense doesn't have bounded_hit (always false)
        return Ok((symbol_ids, supernode_map, false));
    }

    // Path enumeration
    if let Some(start_symbol) = options.paths_from {
        let start_id = resolve_fqn_to_symbol_id(db_path, start_symbol)?;
        let db_path_str = db_path.to_string_lossy().to_string();

        // Build args: magellan paths --db <DB> --start <ID> [--end <ID>] --max-depth 100 --max-paths 1000 --output json
        let mut args = vec![
            "paths".to_string(),
            "--db".to_string(),
            db_path_str.clone(),
            "--start".to_string(),
            start_id.clone(),
            "--max-depth".to_string(),
            "100".to_string(),
            "--max-paths".to_string(),
            "1000".to_string(),
            "--output".to_string(),
            "json".to_string(),
        ];

        if let Some(end_symbol) = options.paths_to {
            let end_id = resolve_fqn_to_symbol_id(db_path, end_symbol)?;
            args.push("--end".to_string());
            args.push(end_id);
        }

        let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

        let output = Command::new("magellan")
            .args(&args_refs)
            .output()
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => LlmError::MagellanNotFound,
                _ => LlmError::MagellanExecutionFailed {
                    algorithm: "paths".to_string(),
                    stderr: format!(
                        "{}\n\nTry running: magellan paths --db {} --start {} for more details",
                        e, db_path_str, start_id
                    ),
                },
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(LlmError::MagellanExecutionFailed {
                algorithm: "paths".to_string(),
                stderr: format!(
                    "{}\n\nTry running: magellan paths --db {} --start {} for more details",
                    stderr, db_path_str, start_id
                ),
            });
        }

        let json = String::from_utf8_lossy(&output.stdout);
        let (symbol_ids, bounded_hit) = magellan_bridge::parse_paths_output(&json)?;

        // bounded_hit is propagated to main.rs for warning display
        // No decoration map needed for paths (unlike condense)
        return Ok((symbol_ids, HashMap::new(), bounded_hit));
    }

    // No active filters
    Ok((Vec::new(), HashMap::new(), false))
}

/// Threshold for using temporary table instead of IN clause
const SYMBOL_SET_TEMP_TABLE_THRESHOLD: usize = 1000;

/// Create temporary table for large SymbolSet filtering
///
/// For SymbolSets larger than SYMBOL_SET_TEMP_TABLE_THRESHOLD,
/// creates a temp table and returns table name for JOIN.
pub fn create_symbol_set_temp_table(
    conn: &Connection,
    symbol_ids: &[String],
) -> Result<String, LlmError> {
    let table_name = format!("symbol_set_filter_{}", std::process::id());

    // Create temporary table
    conn.execute(
        &format!(
            "CREATE TEMP TABLE {} (symbol_id TEXT PRIMARY KEY)",
            table_name
        ),
        [],
    )
    .map_err(LlmError::SqliteError)?;

    // Insert all symbol_ids
    let mut stmt = conn
        .prepare(&format!(
            "INSERT INTO {} (symbol_id) VALUES (?)",
            table_name
        ))
        .map_err(LlmError::SqliteError)?;

    for symbol_id in symbol_ids {
        stmt.execute([symbol_id]).map_err(LlmError::SqliteError)?;
    }

    Ok(table_name)
}

/// Get the appropriate filtering strategy for a SymbolSet
pub fn symbol_set_filter_strategy(symbol_ids: &[String]) -> SymbolSetStrategy {
    if symbol_ids.is_empty() {
        SymbolSetStrategy::None
    } else if symbol_ids.len() > SYMBOL_SET_TEMP_TABLE_THRESHOLD {
        SymbolSetStrategy::TempTable
    } else {
        SymbolSetStrategy::InClause
    }
}

/// Strategy for filtering by SymbolSet
#[derive(Debug, Clone, PartialEq)]
pub enum SymbolSetStrategy {
    /// No filtering needed
    None,
    /// Use SQL IN clause (for <= 1000 items)
    InClause,
    /// Use temporary table with JOIN (for > 1000 items)
    TempTable,
}

/// Resolve a simple symbol name to its SymbolId using `magellan find`.
///
/// When users provide a simple name (e.g., "main") instead of a full SymbolId,
/// this function queries Magellan's database to find matching symbols.
///
/// # Ambiguity Detection
///
/// If multiple symbols match the given name, this function returns
/// `LlmError::AmbiguousSymbolName` with the count of matches. The user must
/// then provide a more specific identifier (full SymbolId or path-qualified name).
///
/// # Arguments
///
/// * `db_path` - Path to the Magellan code graph database
/// * `name` - Simple symbol name to resolve (e.g., "main", "process_request")
///
/// # Returns
///
/// The SymbolId of the first matching symbol (if unique).
///
/// # Errors
///
/// Returns `LlmError::AmbiguousSymbolName` if multiple symbols match the name.
/// Returns `LlmError::InvalidQuery` (LLM-E011) if no symbols match.
/// Returns `LlmError::SearchFailed` if the database cannot be opened.
///
/// # Example
///
/// ```no_run
/// use llmgrep::algorithm::resolve_fqn_to_symbol_id;
/// use std::path::Path;
///
/// let db_path = Path::new(".magellan/llmgrep.db");
/// let symbol_id = resolve_fqn_to_symbol_id(db_path, "main")?;
/// println!("Resolved to: {}", symbol_id);
/// # Ok::<(), llmgrep::error::LlmError>(())
/// ```
pub fn resolve_fqn_to_symbol_id(db_path: &Path, name: &str) -> Result<String, LlmError> {
    let graph = CodeGraph::open(db_path).map_err(|e| LlmError::SearchFailed {
        reason: format!("Failed to open database: {}", e),
    })?;
    let nav = graph.navigator();

    let resolved = nav.resolve(name).map_err(|e| LlmError::SearchFailed {
        reason: format!("Symbol resolution failed: {}", e),
    })?;

    if resolved.is_empty() {
        return Err(LlmError::InvalidQuery {
            query: format!("Symbol '{}' not found in database", name),
        });
    }

    if resolved.len() > 1 {
        return Err(LlmError::AmbiguousSymbolName {
            name: name.to_string(),
            count: resolved.len(),
        });
    }

    Ok(resolved[0].id.to_string())
}

pub use magellan_bridge::{
    check_magellan_available, parse_condense_output, parse_paths_output, parse_symbol_set_file,
    run_magellan_algorithm,
};

mod magellan_bridge;
#[cfg(test)]
mod tests;
