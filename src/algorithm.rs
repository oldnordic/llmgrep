//! Algorithm integration module for Magellan 2.0 graph algorithms.
//!
//! This module provides integration with Magellan's executable graph algorithms
//! through a shell-out pattern. It supports:
//!
//! - Loading pre-computed SymbolSet files from JSON
//! - Running Magellan algorithms (reachable, dead-code, slice, cycles) and extracting SymbolIds
//! - Resolving simple symbol names to SymbolIds via `magellan find`
//!
//! The SymbolSet type is the core abstraction for algorithm-based filtering.
//! It represents a collection of SymbolIds (32-char BLAKE3 hashes) that can be
//! used to filter search results.

use crate::error::LlmError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use rusqlite::Connection;

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

/// Check if Magellan CLI is available and at the correct version.
///
/// This function implements the fail-fast approach: check availability
/// before attempting any algorithm shell-out. Returns Ok(()) if Magellan
/// is available and version is acceptable, otherwise returns an error
/// with helpful installation instructions.
///
/// # Errors
///
/// Returns `LlmError::MagellanNotFound` if magellan CLI is not in PATH.
/// Returns `LlmError::MagellanVersionMismatch` if version is less than 2.1.0.
///
/// # Example
///
/// ```no_run
/// use llmgrep::algorithm::check_magellan_available;
///
/// check_magellan_available()?;
/// # Ok::<(), llmgrep::error::LlmError>(())
/// ```
pub fn check_magellan_available() -> Result<(), LlmError> {
    check_magellan_version()
}

/// Check Magellan version is 2.1.0 or higher.
///
/// Parses `magellan --version` output and enforces minimum version.
/// Magellan output format: "magellan VERSION (DATE) rustc RUSTC_VERSION"
fn check_magellan_version() -> Result<(), LlmError> {
    use std::thread;
    use std::time::Duration;

    let child = Command::new("magellan")
        .arg("--version")
        .spawn();

    let output = match child {
        Ok(mut child) => {
            let timeout = Duration::from_secs(5);
            let start = std::time::Instant::now();

            loop {
                if let Ok(status) = child.try_wait() {
                    match status {
                        Some(_status) => {
                            // Process has exited, get output
                            break child.wait_with_output();
                        }
                        None => {
                            // Still running, check timeout
                            if start.elapsed() >= timeout {
                                child.kill().ok();
                                return Err(LlmError::MagellanExecutionFailed {
                                    algorithm: "version check".to_string(),
                                    stderr: "Version check timed out after 5 seconds".to_string(),
                                });
                            }
                            thread::sleep(Duration::from_millis(100));
                        }
                    }
                }
            }
        }
        Err(e) => return Err(match e.kind() {
            std::io::ErrorKind::NotFound => LlmError::MagellanNotFound,
            _ => LlmError::MagellanExecutionFailed {
                algorithm: "version check".to_string(),
                stderr: e.to_string(),
            },
        }),
    };

    let output = output.map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => LlmError::MagellanNotFound,
        _ => LlmError::MagellanExecutionFailed {
            algorithm: "version check".to_string(),
            stderr: e.to_string(),
        },
    })?;

    if !output.status.success() {
        return Err(LlmError::MagellanExecutionFailed {
            algorithm: "version check".to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }

    // Parse version from output: "magellan 2.1.0 (2024-01-15) rustc 1.75.0"
    let version_str = String::from_utf8_lossy(&output.stdout);
    let version = parse_magellan_version(&version_str)?;

    // Require 2.1.0 or higher
    if version < (2, 1, 0) {
        return Err(LlmError::MagellanVersionMismatch {
            current: format!("{}.{}.{}", version.0, version.1, version.2),
            required: "2.1.0".to_string(),
        });
    }

    Ok(())
}

/// Parse Magellan version string into (major, minor, patch) tuple.
///
/// Expected format: "magellan X.Y.Z (DATE) rustc VERSION"
fn parse_magellan_version(output: &str) -> Result<(u32, u32, u32), LlmError> {
    // Extract the version number after "magellan"
    let first_line = output.lines().next().unwrap_or("");
    let version_part = first_line
        .strip_prefix("magellan")
        .unwrap_or("")
        .split_whitespace()
        .next()
        .unwrap_or("");

    // Parse X.Y.Z format
    let parts: Vec<&str> = version_part.split('.').collect();
    if parts.len() != 3 {
        return Err(LlmError::MagellanExecutionFailed {
            algorithm: "version parsing".to_string(),
            stderr: format!("Unable to parse version from: {}", first_line),
        });
    }

    let major: u32 = parts[0].parse().map_err(|_| LlmError::MagellanExecutionFailed {
        algorithm: "version parsing".to_string(),
        stderr: format!("Invalid major version: {}", parts[0]),
    })?;
    let minor: u32 = parts[1].parse().map_err(|_| LlmError::MagellanExecutionFailed {
        algorithm: "version parsing".to_string(),
        stderr: format!("Invalid minor version: {}", parts[1]),
    })?;
    let patch: u32 = parts[2].parse().map_err(|_| LlmError::MagellanExecutionFailed {
        algorithm: "version parsing".to_string(),
        stderr: format!("Invalid patch version: {}", parts[2]),
    })?;

    Ok((major, minor, patch))
}

/// Run a Magellan algorithm and extract SymbolIds from its JSON output.
///
/// This function shells out to the Magellan CLI, executes the specified algorithm,
/// and parses the JSON output to extract SymbolIds. The extraction logic handles
/// the different JSON structures returned by each algorithm.
///
/// # Arguments
///
/// * `db_path` - Path to the Magellan code graph database
/// * `algorithm` - Algorithm name: "reachable", "dead-code", "slice", "cycles"
/// * `args` - Algorithm-specific arguments (e.g., ["--symbol-id", "abc123..."])
///
/// # Returns
///
/// A `SymbolSet` containing the SymbolIds extracted from the algorithm output.
///
/// # Errors
///
/// Returns `LlmError::SearchFailed` if:
/// - Magellan CLI is not found in PATH
/// - The algorithm execution fails (non-zero exit code)
/// - The JSON output cannot be parsed
///
/// # Supported Algorithms
///
/// - **reachable**: Extracts from `result.symbols[].symbol_id`
/// - **dead-code**: Extracts from `result.dead_symbols[].symbol.symbol_id`
/// - **slice**: Extracts from `result.included_symbols[].symbol_id`
/// - **cycles**: Extracts from `result.cycles[].members[].symbol_id`
///
/// # Example
///
/// ```no_run
/// use llmgrep::algorithm::run_magellan_algorithm;
/// use std::path::Path;
///
/// let db_path = Path::new(".codemcp/codegraph.db");
/// let symbol_set = run_magellan_algorithm(
///     db_path,
///     "reachable",
///     &["--symbol-id", "abc123def456789012345678901234ab"],
/// )?;
/// # Ok::<(), llmgrep::error::LlmError>(())
/// ```
pub fn run_magellan_algorithm(
    db_path: &Path,
    algorithm: &str,
    args: &[&str],
) -> Result<SymbolSet, LlmError> {
    // Check Magellan availability and version
    check_magellan_available()?;

    // Build magellan command
    let mut cmd = Command::new("magellan");
    cmd.arg(algorithm)
        .arg("--db")
        .arg(db_path)
        .arg("--output")
        .arg("json")
        .args(args);

    // Execute and capture output
    let output = cmd.output().map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => LlmError::MagellanNotFound,
        _ => LlmError::MagellanExecutionFailed {
            algorithm: algorithm.to_string(),
            stderr: e.to_string(),
        },
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(LlmError::MagellanExecutionFailed {
            algorithm: algorithm.to_string(),
            stderr: stderr.to_string(),
        });
    }

    // Parse JSON response and extract SymbolIds
    let json_str = String::from_utf8_lossy(&output.stdout);
    extract_symbol_ids_from_magellan_json(&json_str, algorithm)
}

/// Extract SymbolIds from Magellan algorithm JSON output.
///
/// Each Magellan algorithm has a different JSON structure. This function
/// parses the algorithm-specific format and extracts all SymbolIds into
/// a unified SymbolSet.
///
/// # JSON Structures by Algorithm
///
/// - **reachable**: `{"result": {"symbols": [{"symbol_id": "..."}, ...]}}`
/// - **dead-code**: `{"result": {"dead_symbols": [{"symbol": {"symbol_id": "..."}}, ...]}}`
/// - **slice**: `{"result": {"included_symbols": [{"symbol_id": "..."}, ...]}}`
/// - **cycles**: `{"result": {"cycles": [{"members": [{"symbol_id": "..."}, ...]}]}}`
///
/// # Arguments
///
/// * `json` - The raw JSON string from Magellan's stdout
/// * `algorithm` - The algorithm name (determines which parser to use)
///
/// # Returns
///
/// A `SymbolSet` containing all extracted SymbolIds.
fn extract_symbol_ids_from_magellan_json(
    json: &str,
    algorithm: &str,
) -> Result<SymbolSet, LlmError> {
    let parsed: Value = serde_json::from_str(json).map_err(LlmError::JsonError)?;

    // Extract symbol_ids based on algorithm type
    let symbol_ids = match algorithm {
        "reachable" => {
            let symbols = parsed["result"]["symbols"]
                .as_array()
                .ok_or_else(|| LlmError::InvalidQuery {
                    query: "Missing 'symbols' array in reachable output".to_string(),
                })?;
            symbols
                .iter()
                .filter_map(|s| s["symbol_id"].as_str().map(|id| id.to_string()))
                .collect()
        }
        "dead-code" => {
            let dead_symbols = parsed["result"]["dead_symbols"]
                .as_array()
                .ok_or_else(|| LlmError::InvalidQuery {
                    query: "Missing 'dead_symbols' array in dead-code output".to_string(),
                })?;
            dead_symbols
                .iter()
                .filter_map(|s| s["symbol"]["symbol_id"].as_str().map(|id| id.to_string()))
                .collect()
        }
        "slice" => {
            let included_symbols = parsed["result"]["included_symbols"]
                .as_array()
                .ok_or_else(|| LlmError::InvalidQuery {
                    query: "Missing 'included_symbols' array in slice output".to_string(),
                })?;
            included_symbols
                .iter()
                .filter_map(|s| s["symbol_id"].as_str().map(|id| id.to_string()))
                .collect()
        }
        "cycles" => {
            let cycles = parsed["result"]["cycles"]
                .as_array()
                .ok_or_else(|| LlmError::InvalidQuery {
                    query: "Missing 'cycles' array in cycles output".to_string(),
                })?;
            let mut ids = Vec::new();
            for cycle in cycles {
                if let Some(members) = cycle["members"].as_array() {
                    for member in members {
                        if let Some(id) = member["symbol_id"].as_str() {
                            ids.push(id.to_string());
                        }
                    }
                }
            }
            ids
        }
        _ => {
            return Err(LlmError::InvalidQuery {
                query: format!("Unknown algorithm: {}", algorithm),
            });
        }
    };

    Ok(SymbolSet { symbol_ids })
}

/// Parse magellan condense output and extract SCC membership.
///
/// This function parses the JSON output from `magellan condense` and extracts:
/// - All symbol_ids from SCC members
/// - A mapping of symbol_id -> supernode_id for output decoration
///
/// # Condense JSON Structure
///
/// ```json
/// {
///   "supernodes": [
///     {
///       "id": "supernode_0",
///       "members": [
///         {"symbol_id": "abc123def456789012345678901234ab"},
///         {"symbol_id": "def456789012345678901234abcd1234"}
///       ]
///     },
///     ...
///   ]
/// }
/// ```
///
/// # Arguments
///
/// * `json` - The raw JSON string from Magellan's stdout
///
/// # Returns
///
/// A tuple of:
/// - `Vec<String>` of all symbol_ids from all SCC members
/// - `HashMap<String, String>` mapping symbol_id -> supernode_id
///
/// # Errors
///
/// Returns `LlmError::MagellanExecutionFailed` (LLM-E002) if JSON structure is invalid.
/// Returns empty Vec/HashMap if supernodes array is empty (not an error).
///
/// # Example
///
/// ```no_run
/// use llmgrep::algorithm::parse_condense_output;
///
/// let json = r#"{"supernodes": [{"id": "supernode_0", "members": [{"symbol_id": "abc123..."}]}]}"#;
/// let (symbol_ids, supernode_map) = parse_condense_output(json)?;
/// # Ok::<(), llmgrep::error::LlmError>(())
/// ```
pub fn parse_condense_output(json: &str) -> Result<(Vec<String>, std::collections::HashMap<String, String>), LlmError> {
    use std::collections::HashMap;

    let parsed: Value = serde_json::from_str(json).map_err(LlmError::JsonError)?;

    // Handle magellan wrapper structure: {"data": {"supernodes": [...]}}
    // Fall back to direct structure for backward compatibility
    let supernodes = if parsed["data"]["supernodes"].is_array() {
        parsed["data"]["supernodes"].as_array()
    } else {
        parsed["supernodes"].as_array()
    }
    .ok_or_else(|| LlmError::MagellanExecutionFailed {
        algorithm: "condense".to_string(),
        stderr: "Missing 'supernodes' array in condense output".to_string(),
    })?;

    let mut all_symbol_ids = Vec::new();
    let mut supernode_map = HashMap::new();

    // Extract members from each supernode
    for supernode in supernodes {
        // Handle both numeric and string IDs (magellan uses numeric)
        let supernode_id = if let Some(id_str) = supernode["id"].as_str() {
            id_str.to_string()
        } else if let Some(id_num) = supernode["id"].as_u64() {
            format!("supernode_{}", id_num)
        } else if let Some(id_i64) = supernode["id"].as_i64() {
            format!("supernode_{}", id_i64)
        } else {
            return Err(LlmError::MagellanExecutionFailed {
                algorithm: "condense".to_string(),
                stderr: "Supernode missing 'id' field".to_string(),
            });
        };

        let members = supernode["members"]
            .as_array()
            .ok_or_else(|| LlmError::MagellanExecutionFailed {
                algorithm: "condense".to_string(),
                stderr: "Supernode missing 'members' array".to_string(),
            })?;

        for member in members {
            let symbol_id = member["symbol_id"]
                .as_str()
                .ok_or_else(|| LlmError::MagellanExecutionFailed {
                    algorithm: "condense".to_string(),
                    stderr: "Member missing 'symbol_id' field".to_string(),
                })?;

            all_symbol_ids.push(symbol_id.to_string());
            supernode_map.insert(symbol_id.to_string(), supernode_id.to_string());
        }
    }

    Ok((all_symbol_ids, supernode_map))
}

/// Parse magellan paths output and extract execution path symbols.
///
/// This function parses the JSON output from `magellan paths` command and extracts
/// all unique SymbolIds from the returned execution paths. The paths command
/// enumerates all possible execution paths between symbols using bounded depth-first
/// search.
///
/// # Arguments
///
/// * `json` - The raw JSON string from Magellan's stdout
///
/// # Returns
///
/// A tuple of:
/// - `Vec<String>` of all unique symbol_ids from all paths
/// - `bool` indicating if bounded enumeration hit limits (bounded_hit)
///
/// # Errors
///
/// Returns `LlmError::MagellanExecutionFailed` (LLM-E002) if JSON structure is invalid.
/// Returns empty Vec and bounded_flag=false if paths array is empty (not an error).
///
/// # Example
///
/// ```no_run
/// use llmgrep::algorithm::parse_paths_output;
///
/// let json = r#"{"paths": [{"symbols": [{"symbol_id": "abc123..."}], "length": 1}], "bounded_hit": false}"#;
/// let (symbol_ids, bounded_hit) = parse_paths_output(json)?;
/// # Ok::<(), llmgrep::error::LlmError>(())
/// ```
pub fn parse_paths_output(json: &str) -> Result<(Vec<String>, bool), LlmError> {
    use std::collections::HashSet;

    let parsed: Value = serde_json::from_str(json).map_err(LlmError::JsonError)?;

    // Handle magellan wrapper structure: {"data": {"paths": [...]}}
    // Fall back to direct structure for backward compatibility
    let paths = if parsed["data"]["paths"].is_array() {
        parsed["data"]["paths"].as_array()
    } else {
        parsed["paths"].as_array()
    }
    .ok_or_else(|| LlmError::MagellanExecutionFailed {
        algorithm: "paths".to_string(),
        stderr: "Missing 'paths' array in output".to_string(),
    })?;

    let mut all_symbol_ids = HashSet::new();

    // Extract symbol_ids from all paths
    for path in paths {
        let symbols = path["symbols"]
            .as_array()
            .ok_or_else(|| LlmError::MagellanExecutionFailed {
                algorithm: "paths".to_string(),
                stderr: "Path missing 'symbols' array".to_string(),
            })?;

        for symbol in symbols {
            let symbol_id = symbol["symbol_id"]
                .as_str()
                .ok_or_else(|| LlmError::MagellanExecutionFailed {
                    algorithm: "paths".to_string(),
                    stderr: "Symbol missing 'symbol_id' field".to_string(),
                })?;

            all_symbol_ids.insert(symbol_id.to_string());
        }
    }

    // Extract bounded_hit flag from JSON (indicates if max-depth or max-paths was hit)
    let bounded_hit = parsed["bounded_hit"]
        .as_bool()
        .or_else(|| parsed["data"]["bounded_hit"].as_bool())
        .unwrap_or(false);

    // Convert HashSet to Vec for return
    Ok((all_symbol_ids.into_iter().collect(), bounded_hit))
}

/// Parse a SymbolSet from a JSON file and validate its format.
///
/// This is a convenience function that combines `SymbolSet::from_file`
/// and `SymbolSet::validate` into a single call.
///
/// # Arguments
///
/// * `path` - Path to the SymbolSet JSON file
///
/// # Returns
///
/// A validated `SymbolSet`.
///
/// # Errors
///
/// Returns `LlmError::IoError` if the file cannot be read.
/// Returns `LlmError::JsonError` if the JSON is invalid.
/// Returns `LlmError::InvalidQuery` if SymbolId format is invalid.
///
/// # Example
///
/// ```no_run
/// use llmgrep::algorithm::parse_symbol_set_file;
/// use std::path::Path;
///
/// let symbol_set = parse_symbol_set_file(Path::new("symbols.json"))?;
/// # Ok::<(), llmgrep::error::LlmError>(())
/// ```
pub fn parse_symbol_set_file(path: &Path) -> Result<SymbolSet, LlmError> {
    let symbol_set = SymbolSet::from_file(path)?;
    symbol_set.validate()?;
    Ok(symbol_set)
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
        let symbol_set = parse_symbol_set_file(Path::new(file_path))?;
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
        return Ok((run_magellan_algorithm(db_path, "reachable", &args)?.symbol_ids, HashMap::new(), false));
    }

    if let Some(symbol) = options.dead_code_in {
        let symbol_id = resolve_fqn_to_symbol_id(db_path, symbol)?;
        let args = ["--entry", &symbol_id];
        return Ok((run_magellan_algorithm(db_path, "dead-code", &args)?.symbol_ids, HashMap::new(), false));
    }

    if let Some(symbol) = options.in_cycle {
        let symbol_id = resolve_fqn_to_symbol_id(db_path, symbol)?;
        let args = ["--symbol", &symbol_id];
        return Ok((run_magellan_algorithm(db_path, "cycles", &args)?.symbol_ids, HashMap::new(), false));
    }

    if let Some(symbol) = options.slice_backward_from {
        let symbol_id = resolve_fqn_to_symbol_id(db_path, symbol)?;
        let args = ["--target", &symbol_id, "--direction", "backward"];
        return Ok((run_magellan_algorithm(db_path, "slice", &args)?.symbol_ids, HashMap::new(), false));
    }

    if let Some(symbol) = options.slice_forward_from {
        let symbol_id = resolve_fqn_to_symbol_id(db_path, symbol)?;
        let args = ["--target", &symbol_id, "--direction", "forward"];
        return Ok((run_magellan_algorithm(db_path, "slice", &args)?.symbol_ids, HashMap::new(), false));
    }

    // Condense SCC detection
    if options.condense {
        let output = Command::new("magellan")
            .args(["condense", "--db", &db_path.to_string_lossy(), "--output", "json"])
            .output()
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => LlmError::MagellanNotFound,
                _ => LlmError::MagellanExecutionFailed {
                    algorithm: "condense".to_string(),
                    stderr: format!("{}\n\nTry running: magellan condense --db {} for more details",
                        e, db_path.to_string_lossy()),
                },
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(LlmError::MagellanExecutionFailed {
                algorithm: "condense".to_string(),
                stderr: format!("{}\n\nTry running: magellan condense --db {} for more details",
                    stderr, db_path.to_string_lossy()),
            });
        }

        let json = String::from_utf8_lossy(&output.stdout);
        let (symbol_ids, supernode_map) = parse_condense_output(&json)?;

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
                    stderr: format!("{}\n\nTry running: magellan paths --db {} --start {} for more details",
                        e, db_path_str, start_id),
                },
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(LlmError::MagellanExecutionFailed {
                algorithm: "paths".to_string(),
                stderr: format!("{}\n\nTry running: magellan paths --db {} --start {} for more details",
                    stderr, db_path_str, start_id),
            });
        }

        let json = String::from_utf8_lossy(&output.stdout);
        let (symbol_ids, bounded_hit) = parse_paths_output(&json)?;

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
        &format!("CREATE TEMP TABLE {} (symbol_id TEXT PRIMARY KEY)", table_name),
        [],
    )
    .map_err(LlmError::SqliteError)?;

    // Insert all symbol_ids
    let mut stmt = conn
        .prepare(&format!("INSERT INTO {} (symbol_id) VALUES (?)", table_name))
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
/// Returns `LlmError::MagellanNotFound` if magellan CLI is not available.
/// Returns `LlmError::AmbiguousSymbolName` if multiple symbols match the name.
/// Returns `LlmError::InvalidQuery` (LLM-E011) if no symbols match.
/// Returns `LlmError::SearchFailed` if the `magellan find` command fails.
///
/// # Example
///
/// ```no_run
/// use llmgrep::algorithm::resolve_fqn_to_symbol_id;
/// use std::path::Path;
///
/// let db_path = Path::new(".codemcp/codegraph.db");
/// let symbol_id = resolve_fqn_to_symbol_id(db_path, "main")?;
/// println!("Resolved to: {}", symbol_id);
/// # Ok::<(), llmgrep::error::LlmError>(())
/// ```
pub fn resolve_fqn_to_symbol_id(db_path: &Path, name: &str) -> Result<String, LlmError> {
    // Check Magellan availability and version
    check_magellan_available()?;

    // Run magellan find
    let output = Command::new("magellan")
        .args(["find", "--db", &db_path.to_string_lossy(), "--name", name, "--output", "json"])
        .output()
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => LlmError::MagellanNotFound,
            _ => LlmError::SearchFailed {
                reason: format!("Failed to execute magellan find: {}", e),
            },
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(LlmError::SearchFailed {
            reason: format!("magellan find failed: {}", stderr),
        });
    }

    // Parse JSON response
    let json: Value = serde_json::from_slice(&output.stdout).map_err(LlmError::JsonError)?;

    // Check for matches array
    let matches = json["matches"]
        .as_array()
        .ok_or_else(|| LlmError::InvalidQuery {
            query: "Invalid magellan find output: missing 'matches' array".to_string(),
        })?;

    // Handle ambiguity
    if matches.len() > 1 {
        return Err(LlmError::AmbiguousSymbolName {
            name: name.to_string(),
            count: matches.len(),
        });
    }

    // Handle not found
    if matches.is_empty() {
        return Err(LlmError::InvalidQuery {
            query: format!("Symbol '{}' not found in database", name),
        });
    }

    // Extract symbol_id from first match
    matches[0]["symbol_id"]
        .as_str()
        .map(|id| id.to_string())
        .ok_or_else(|| LlmError::InvalidQuery {
            query: format!(
                "Symbol '{}' found but missing symbol_id in magellan output",
                name
            ),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_set_validation_valid() {
        let symbol_set = SymbolSet {
            symbol_ids: vec![
                "abc123def456789012345678901234ab".to_string(),
                "0123456789abcdef0123456789abcdef".to_string(),
                "ffffffffffffffffffffffffffffffff".to_string(),
            ],
        };
        assert!(symbol_set.validate().is_ok());
        assert_eq!(symbol_set.len(), 3);
        assert!(!symbol_set.is_empty());
    }

    #[test]
    fn test_symbol_set_validation_invalid_length() {
        let symbol_set = SymbolSet {
            symbol_ids: vec!["abc123".to_string()],
        };
        assert!(symbol_set.validate().is_err());
    }

    #[test]
    fn test_symbol_set_validation_invalid_chars() {
        let symbol_set = SymbolSet {
            symbol_ids: vec!["abc123def456789012345678901234g!".to_string()],
        };
        assert!(symbol_set.validate().is_err());
    }

    #[test]
    fn test_symbol_set_empty() {
        let symbol_set = SymbolSet {
            symbol_ids: vec![],
        };
        assert!(symbol_set.validate().is_ok());
        assert_eq!(symbol_set.len(), 0);
        assert!(symbol_set.is_empty());
    }

    #[test]
    fn test_symbol_set_json_deserialize() {
        let json = r#"{"symbol_ids": ["abc123def456789012345678901234ab"]}"#;
        let symbol_set: SymbolSet = serde_json::from_str(json).unwrap();
        assert_eq!(symbol_set.symbol_ids.len(), 1);
        assert_eq!(
            symbol_set.symbol_ids[0],
            "abc123def456789012345678901234ab"
        );
    }

    #[test]
    fn test_symbol_set_json_serialize() {
        let symbol_set = SymbolSet {
            symbol_ids: vec!["abc123def456789012345678901234ab".to_string()],
        };
        let json = serde_json::to_string(&symbol_set).unwrap();
        assert!(json.contains("symbol_ids"));
        assert!(json.contains("abc123def456789012345678901234ab"));
    }
}
