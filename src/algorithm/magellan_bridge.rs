use crate::error::LlmError;
use serde_json::Value;
use std::path::Path;
use std::process::Command;

use super::SymbolSet;

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
    // Use output() directly - it handles pipe management correctly
    // and waits for process completion before reading stdout/stderr.
    // The previous spawn() + try_wait() + wait_with_output() approach
    // had a race condition where stdout could be lost.
    let output = Command::new("magellan")
        .arg("--version")
        .output()
        .map_err(|e| match e.kind() {
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

    let major: u32 = parts[0]
        .parse()
        .map_err(|_| LlmError::MagellanExecutionFailed {
            algorithm: "version parsing".to_string(),
            stderr: format!("Invalid major version: {}", parts[0]),
        })?;
    let minor: u32 = parts[1]
        .parse()
        .map_err(|_| LlmError::MagellanExecutionFailed {
            algorithm: "version parsing".to_string(),
            stderr: format!("Invalid minor version: {}", parts[1]),
        })?;
    let patch: u32 = parts[2]
        .parse()
        .map_err(|_| LlmError::MagellanExecutionFailed {
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
/// let db_path = Path::new(".magellan/llmgrep.db");
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
            let symbols =
                parsed["result"]["symbols"]
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
            let dead_symbols = parsed["result"]["dead_symbols"].as_array().ok_or_else(|| {
                LlmError::InvalidQuery {
                    query: "Missing 'dead_symbols' array in dead-code output".to_string(),
                }
            })?;
            dead_symbols
                .iter()
                .filter_map(|s| s["symbol"]["symbol_id"].as_str().map(|id| id.to_string()))
                .collect()
        }
        "slice" => {
            let included_symbols =
                parsed["result"]["included_symbols"]
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
            let cycles =
                parsed["result"]["cycles"]
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
pub fn parse_condense_output(
    json: &str,
) -> Result<(Vec<String>, std::collections::HashMap<String, String>), LlmError> {
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

        let members =
            supernode["members"]
                .as_array()
                .ok_or_else(|| LlmError::MagellanExecutionFailed {
                    algorithm: "condense".to_string(),
                    stderr: "Supernode missing 'members' array".to_string(),
                })?;

        for member in members {
            let symbol_id =
                member["symbol_id"]
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
        let symbols =
            path["symbols"]
                .as_array()
                .ok_or_else(|| LlmError::MagellanExecutionFailed {
                    algorithm: "paths".to_string(),
                    stderr: "Path missing 'symbols' array".to_string(),
                })?;

        for symbol in symbols {
            let symbol_id =
                symbol["symbol_id"]
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
