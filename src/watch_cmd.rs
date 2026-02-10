//! Watch command implementation for real-time query results

use anyhow::Result;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crate::backend::Backend;
use crate::error::LlmError;
use crate::output::{OutputFormat, SearchResponse, SymbolMatch};
use crate::query::SearchOptions;

#[cfg(feature = "native-v2")]
use sqlitegraph::backend::{PubSubEvent, SubscriptionFilter};

/// Run the watch command with automatic backend detection.
///
/// This function detects the database format and uses the appropriate
/// watching strategy:
/// - Native-V2: Uses pub/sub events for real-time updates
/// - SQLite: Falls back to file watching (with warning)
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
pub fn run_watch(
    db_path: PathBuf,
    options: SearchOptions,
    output_format: OutputFormat,
    shutdown: Arc<AtomicBool>,
) -> Result<()> {
    // Detect backend format and open database
    let backend = Backend::detect_and_open(&db_path)?;

    match backend {
        #[cfg(feature = "native-v2")]
        Backend::NativeV2(inner) => {
            run_watch_with_pubsub(&inner, db_path, options, output_format, shutdown)
        }
        Backend::Sqlite(inner) => {
            // SQLite backend: warn and use file watching
            eprintln!("Warning: SQLite backend detected, file watching is not fully supported.");
            eprintln!("For real-time updates, reindex with native-v2 storage:");
            eprintln!("  magellan watch --root . --db {} --storage native-v2", db_path.display());
            run_watch_with_filesystem(&inner, db_path, options, output_format, shutdown)
        }
    }
}

/// Run watch mode with pub/sub events (Native-V2 backend only).
///
/// # Arguments
/// * `backend` - The NativeV2Backend reference
/// * `db_path` - Path to the database file
/// * `options` - Search options for the query
/// * `output_format` - Output format for results
/// * `shutdown` - Atomic flag for graceful shutdown
///
/// # Returns
/// * `Ok(())` on successful shutdown
/// * `Err(LlmError)` on failure
#[cfg(feature = "native-v2")]
fn run_watch_with_pubsub(
    backend: &crate::backend::native_v2::NativeV2Backend,
    db_path: PathBuf,
    options: SearchOptions,
    output_format: OutputFormat,
    shutdown: Arc<AtomicBool>,
) -> Result<()> {
    use magellan::CodeGraph;
    use std::sync::mpsc::Receiver;

    // Open the CodeGraph to access pub/sub
    let mut graph = CodeGraph::open(&db_path)
        .map_err(|e| LlmError::DatabaseCorrupted {
            reason: e.to_string(),
        })?;

    // Subscribe to ALL graph mutation events
    let (_sub_id, rx): (u64, Receiver<PubSubEvent>) = graph
        .subscribe(SubscriptionFilter::all())
        .map_err(|e| LlmError::BackendDetectionFailed {
            path: db_path.display().to_string(),
            reason: format!("Failed to subscribe to pub/sub: {}", e),
        })?;

    // Run initial query and display results
    let (response, _partial, _paths_bounded) = backend
        .search_symbols(options.clone())
        .map_err(|e| LlmError::SearchFailed {
            reason: e.to_string(),
        })?;

    display_results(&response, &output_format);
    let mut previous_results = response.results;

    // Event loop with 100ms timeout for shutdown flag checking
    const TIMEOUT_MS: u64 = 100;

    while !shutdown.load(Ordering::Relaxed) {
        match rx.recv_timeout(Duration::from_millis(TIMEOUT_MS)) {
            Ok(_event) => {
                // Re-run query on each event
                match backend.search_symbols(options.clone()) {
                    Ok((current_response, _, _)) => {
                        // Display delta (only new/removed results)
                        format_delta(&previous_results, &current_response.results, &output_format);
                        previous_results = current_response.results;
                    }
                    Err(e) => {
                        eprintln!("Query failed: {}", e);
                        // Continue watching despite transient errors
                    }
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // Timeout is expected - allows checking shutdown flag
                continue;
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                eprintln!("Backend disconnected, stopping watch");
                break;
            }
        }
    }

    println!("SHUTDOWN");
    Ok(())
}

/// Run watch mode with filesystem polling (SQLite backend fallback).
///
/// Note: This is a simplified implementation that polls the database
/// file for modifications. For production use, consider adding the
/// `notify` crate for proper file watching.
///
/// # Arguments
/// * `backend` - The SqliteBackend reference
/// * `db_path` - Path to the database file
/// * `options` - Search options for the query
/// * `output_format` - Output format for results
/// * `shutdown` - Atomic flag for graceful shutdown
///
/// # Returns
/// * `Ok(())` on successful shutdown
/// * `Err(LlmError)` on failure
fn run_watch_with_filesystem(
    backend: &crate::backend::sqlite::SqliteBackend,
    db_path: PathBuf,
    options: SearchOptions,
    output_format: OutputFormat,
    shutdown: Arc<AtomicBool>,
) -> Result<()> {
    use crate::backend::BackendTrait;

    // Run initial query and display results
    let (response, _partial, _paths_bounded) = backend
        .search_symbols(options.clone())
        .map_err(|e| LlmError::SearchFailed {
            reason: e.to_string(),
        })?;

    display_results(&response, &output_format);
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
                        format_delta(&previous_results, &current_response.results, &output_format);
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
fn get_file_modification_time(path: &PathBuf) -> Result<SystemTime, LlmError> {
    path.metadata()
        .and_then(|m| m.modified())
        .map_err(LlmError::IoError)
}

/// Display search results in the specified format.
fn display_results(response: &SearchResponse, output_format: &OutputFormat) {
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
            let json_output = serde_json::to_string_pretty(response).unwrap_or_default();
            println!("{}", json_output);
        }
    }
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
) {
    // Compute added: items in current but not previous
    let added: Vec<&SymbolMatch> = current
        .iter()
        .filter(|c| !previous.iter().any(|p| p.match_id == c.match_id))
        .collect();

    // Compute removed: items in previous but not current
    let removed: Vec<&SymbolMatch> = previous
        .iter()
        .filter(|p| !current.iter().any(|c| c.match_id == p.match_id))
        .collect();

    if added.is_empty() && removed.is_empty() {
        return; // No changes, skip output
    }

    match output_format {
        OutputFormat::Human => {
            println!("\n--- Changes: Added: {}, Removed: {} ---", added.len(), removed.len());

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
                let json_output = serde_json::to_string_pretty(result).unwrap_or_default();
                println!("+ {}", json_output);
            }

            // Show removed results
            for result in &removed {
                let json_output = serde_json::to_string_pretty(result).unwrap_or_default();
                println!("- {}", json_output);
            }
        }
    }
}

/// Format a symbol match for human-readable output.
fn format_symbol_match(result: &SymbolMatch) -> String {
    let kind = result.kind.as_str();
    let name = result.name.as_str();
    let path = result.span.file_path.as_str();

    format!("[{}:{}] {} ({})", path, result.span.start_line, name, kind)
}
