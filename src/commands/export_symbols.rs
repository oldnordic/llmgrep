//! Export symbol map for O(1) symbol lookup.
//!
//! Provides `export-symbols` command that writes all symbols to JSON
//! for fast lookups without database queries.

use crate::cli::Cli;
use llmgrep::error::LlmError;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;

/// Symbol location for O(1) lookup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolLocation {
    /// Symbol ID
    pub id: i64,

    /// Symbol kind (fn, struct, mod, etc.)
    pub kind: String,

    /// Symbol name
    pub name: String,

    /// File path
    pub file_path: String,
}

/// Export all symbols from magellan database to JSON
///
/// Writes symbol map with symbol name → locations mapping.
/// Supports repo-root convention and custom output paths.
pub fn run_export_symbols(cli: &Cli, output_path: PathBuf) -> Result<(), LlmError> {
    use crate::cli::resolve_db_path;

    let db_path = resolve_db_path(cli)?;

    // Open magellan database
    let conn = Connection::open(&db_path)?;

    // Check schema version
    llmgrep::backend::schema_check::check_schema_version(&conn)
        .map_err(|e| LlmError::SchemaMismatch { reason: e })?;

    // Query all symbols
    let mut stmt = conn.prepare(
        "SELECT id, kind, name, file_path
         FROM graph_entities
         ORDER BY name, id",
    )?;

    // Build symbol map: name → list of locations
    let mut symbol_map: HashMap<String, Vec<SymbolLocation>> = HashMap::new();

    let mut rows = stmt.query([])?;

    while let Some(row) = rows.next()? {
        let id: i64 = row.get(0)?;
        let kind: String = row.get(1)?;
        let name: String = row.get(2)?;
        let file_path: String = row.get(3).unwrap_or_default();

        let location = SymbolLocation {
            id,
            kind,
            name: name.clone(),
            file_path,
        };

        symbol_map
            .entry(name)
            .or_insert_with(Vec::new)
            .push(location);
    }

    // Prepare output data
    let export_data = serde_json::json!({
        "symbols": symbol_map,
        "total_symbols": symbol_map.len(),
        "export_time": chrono::Utc::now().to_rfc3339()
    });

    // Write to file
    let file = File::create(&output_path)?;
    let writer = BufWriter::new(file);

    serde_json::to_writer_pretty(writer, &export_data)?;

    // Print summary
    let total_symbols: usize = symbol_map.values().map(|v| v.len()).sum();
    eprintln!(
        "Exported {} symbols ({} unique names) to {}",
        total_symbols,
        symbol_map.len(),
        output_path.display()
    );

    Ok(())
}
