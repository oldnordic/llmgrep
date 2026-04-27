//! Watch command implementation for real-time query results
//!
//! This is an incomplete feature that requires unstable Magellan APIs.
//! Enable with the `unstable-watch` feature flag.

use anyhow::Result;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crate::backend::{Backend, BackendTrait};
use crate::error::LlmError;
use crate::output::{OutputFormat, SearchResponse, SymbolMatch};
use crate::query::SearchOptions;

/// Run the watch command with automatic backend detection.
///
/// This function detects the database format and uses file watching
/// with periodic polling for all backends.
///
/// # Arguments
/// * `db_path` - Path to the database file
/// * `options` - Search options for the query
/// * `output_format` - Output format for results
/// * `shutdown` - Atomic flag for graceful shutdown
///
/// # Returns
/// * `Ok(())` on successful shutdown
/// * `Err(LlmError)` on failure
pub fn run_watch<'a>(
    db_path: PathBuf,
    options: SearchOptions<'a>,
    output_format: OutputFormat,
    shutdown: Arc<AtomicBool>,
) -> Result<()> {
    // Detect backend format and open database
    let backend = Backend::detect_and_open(&db_path)?;

    match backend {
        Backend::Sqlite(inner) => {
            run_watch_with_filesystem(&inner, db_path, options, output_format, shutdown)
        }
        #[cfg(feature = "geometric-backend")]
        Backend::Geometric(_) => Err(LlmError::SearchFailed {
            reason: "Watch command not supported for geometric backend".to_string(),
        }
        .into()),
    }
}

/// Run watch mode with filesystem polling.
///
/// Note: This is a simplified implementation that polls the database
/// file for modifications. For production use, consider adding the
/// `notify` crate for proper file watching.
///
/// # Arguments
/// * `backend` - Any backend implementing BackendTrait
/// * `db_path` - Path to the database file
/// * `options` - Search options for the query
/// * `output_format` - Output format for results
/// * `shutdown` - Atomic flag for graceful shutdown
///
/// # Returns
/// * `Ok(())` on successful shutdown
/// * `Err(LlmError)` on failure
fn run_watch_with_filesystem<'a>(
    backend: &dyn BackendTrait,
    db_path: PathBuf,
    options: SearchOptions<'a>,
    output_format: OutputFormat,
    shutdown: Arc<AtomicBool>,
) -> Result<()> {
    // Run initial query and display results
    let (response, _partial, _paths_bounded) =
        backend
            .search_symbols(options.clone())
            .map_err(|e| LlmError::SearchFailed {
                reason: e.to_string(),
            })?;

    display_results(&response, &output_format)?;
    let mut previous_results = response.results;
    let mut last_modified = get_file_modification_time(&db_path)?;

    // Polling loop with 1 second interval
    const POLL_INTERVAL_MS: u64 = 1000;

    while !shutdown.load(Ordering::Relaxed) {
        std::thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));

        // Check if database file was modified
        if let Ok(current_modified) = get_file_modification_time(&db_path) {
            if current_modified > last_modified {
                last_modified = current_modified;

                // Re-run query
                match backend.search_symbols(options.clone()) {
                    Ok((current_response, _, _)) => {
                        // Display delta (only new/removed results)
                        format_delta(&previous_results, &current_response.results, &output_format)?;
                        previous_results = current_response.results;
                    }
                    Err(e) => {
                        eprintln!("Query failed: {}", e);
                        // Continue watching despite transient errors
                    }
                }
            }
        }
    }

    println!("SHUTDOWN");
    Ok(())
}

/// Get the last modification time of a file.
fn get_file_modification_time(path: &Path) -> Result<SystemTime, LlmError> {
    path.metadata()
        .and_then(|m| m.modified())
        .map_err(LlmError::IoError)
}

/// Display search results in the specified format.
fn display_results(response: &SearchResponse, output_format: &OutputFormat) -> Result<()> {
    match output_format {
        OutputFormat::Human => {
            // Display initial results header
            println!("Found {} results", response.total_count);
            for result in &response.results {
                println!("  {}", format_symbol_match(result));
            }
        }
        OutputFormat::Json | OutputFormat::Pretty => {
            // JSON output for initial results
            let json_output = serde_json::to_string_pretty(response)?;
            println!("{}", json_output);
        }
    }
    Ok(())
}

/// Format delta between previous and current results.
///
/// Shows only added and removed symbols, not the full result set.
///
/// # Arguments
/// * `previous` - Previous results
/// * `current` - Current results
/// * `output_format` - Output format for results
fn format_delta(
    previous: &[SymbolMatch],
    current: &[SymbolMatch],
    output_format: &OutputFormat,
) -> Result<()> {
    // Build HashSets for O(1) lookup instead of O(n×m) nested loops
    let previous_ids: HashSet<&str> = previous.iter().map(|p| p.match_id.as_str()).collect();
    let current_ids: HashSet<&str> = current.iter().map(|c| c.match_id.as_str()).collect();

    // Compute added: items in current but not previous
    let added: Vec<&SymbolMatch> = current
        .iter()
        .filter(|c| !previous_ids.contains(c.match_id.as_str()))
        .collect();

    // Compute removed: items in previous but not current
    let removed: Vec<&SymbolMatch> = previous
        .iter()
        .filter(|p| !current_ids.contains(p.match_id.as_str()))
        .collect();

    if added.is_empty() && removed.is_empty() {
        return Ok(()); // No changes, skip output
    }

    match output_format {
        OutputFormat::Human => {
            println!(
                "\n--- Changes: Added: {}, Removed: {} ---",
                added.len(),
                removed.len()
            );

            for result in &added {
                println!("+ {}", format_symbol_match(result));
            }

            for result in &removed {
                println!("- {}", format_symbol_match(result));
            }
        }
        OutputFormat::Json | OutputFormat::Pretty => {
            // For JSON output, just emit the notice with counts
            // Full result sets are emitted via direct JSON serialization
            let notice = format!("Added: {}, Removed: {}", added.len(), removed.len());
            println!("\n--- {} ---", notice);

            // Show added results
            for result in &added {
                let json_output = serde_json::to_string_pretty(result)?;
                println!("+ {}", json_output);
            }

            // Show removed results
            for result in &removed {
                let json_output = serde_json::to_string_pretty(result)?;
                println!("- {}", json_output);
            }
        }
    }
    Ok(())
}

/// Format a symbol match for human-readable output.
fn format_symbol_match(result: &SymbolMatch) -> String {
    let kind = result.kind.as_str();
    let name = result.name.as_str();
    let path = result.span.file_path.as_str();

    format!("[{}:{}] {} ({})", path, result.span.start_line, name, kind)
}
