use crate::cli::{resolve_db_path, Cli};
use llmgrep::error::LlmError;
use llmgrep::output::{json_response, OutputFormat};

pub fn run_evolve_cmd(
    cli: &Cli,
    query: &str,
    min_score: usize,
    dry_run: bool,
    limit: usize,
) -> Result<(), LlmError> {
    let db_path = resolve_db_path(cli)?;
    let conn = rusqlite::Connection::open(&db_path)?;
    llmgrep::backend::schema_check::check_schema_version(&conn)
        .map_err(|e| LlmError::SchemaMismatch { reason: e })?;

    let options = llmgrep::query::EvolveOptions {
        query,
        min_score: min_score as i64,
        dry_run,
        limit,
    };

    let response = llmgrep::query::run_evolve(&conn, &options)?;
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
        if response.dry_run {
            eprintln!("Dry run mode — no candidates written.");
        }
        for c in &response.candidates {
            println!(
                "{}:{}  {} ({})  fan_in={} complexity={} score={}",
                c.file_path,
                c.symbol_name,
                c.kind,
                c.symbol_id,
                c.fan_in,
                c.cyclomatic_complexity,
                c.score
            );
        }
        eprintln!();
        eprintln!("Candidates: {}", response.total_count);
        if !response.dry_run {
            eprintln!("Written to candidate_facts: {}", response.written);
        }
    }

    Ok(())
}
