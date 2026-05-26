use crate::cli::{resolve_db_path, Cli};
use llmgrep::error::LlmError;
use llmgrep::output::{json_response, OutputFormat};

pub fn run_stats_cmd(cli: &Cli) -> Result<(), LlmError> {
    let db_path = resolve_db_path(cli)?;
    let conn = rusqlite::Connection::open(&db_path)?;
    llmgrep::backend::schema_check::check_schema_version(&conn)
        .map_err(|e| LlmError::SchemaMismatch { reason: e })?;

    let response = llmgrep::query::run_stats(&conn, &db_path)?;
    let wants_json = matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty);

    if wants_json {
        let wrapped = json_response(&response);
        let json_str = if matches!(cli.output, OutputFormat::Pretty) {
            serde_json::to_string_pretty(&wrapped)?
        } else {
            serde_json::to_string(&wrapped)?
        };
        println!("{}", json_str);
    } else {
        println!("Database: {}", response.database);
        if let Some(v) = response.schema_version {
            println!("Schema version: {}", v);
        }
        println!();
        println!("Symbols: {}", response.symbols.total);
        if !response.symbols.by_kind.is_empty() {
            for (kind, count) in &response.symbols.by_kind {
                println!("  {}: {}", kind, count);
            }
        }
        println!("  Avg fan-in: {:.1}", response.symbols.avg_fan_in);
        println!("  Avg complexity: {:.1}", response.symbols.avg_complexity);
        println!();
        println!("Dead code:");
        println!("  Zero fan-in: {}", response.dead_code.zero_fan_in);
        println!("  Zero fan-out: {}", response.dead_code.zero_fan_out);
        if !response.dead_code.likely_dead.is_empty() {
            println!("  Likely dead (fan_in=0 AND fan_out=0):");
            for name in &response.dead_code.likely_dead {
                println!("    - {}", name);
            }
        }
        println!();
        println!("Top hotspots (fan_in x complexity):");
        for h in &response.hotspots {
            println!(
                "  {} ({}): score={} fan_in={} complexity={}",
                h.symbol_name, h.kind, h.score, h.fan_in, h.cyclomatic_complexity
            );
        }
        println!();
        if response.coverage.tables_exist {
            let covered = response.coverage.covered_symbols.unwrap_or(0);
            let gap = response.coverage.total_symbols.saturating_sub(covered);
            println!(
                "Coverage: {}/{} symbols covered (gap: {})",
                covered, response.coverage.total_symbols, gap
            );
        } else {
            println!("Coverage: no coverage data (run 'magellan ingest-coverage')");
        }
    }

    Ok(())
}
