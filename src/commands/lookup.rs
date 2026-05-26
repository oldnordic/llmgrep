use crate::cli::{resolve_db_path, Cli};
use llmgrep::backend::Backend;
use llmgrep::error::LlmError;
use llmgrep::output::OutputFormat;

pub fn run_lookup(cli: &Cli, fqn: &str) -> Result<(), LlmError> {
    let db_path = resolve_db_path(cli)?;

    if fqn.trim().is_empty() {
        return Err(LlmError::InvalidQuery {
            query: "--fqn cannot be empty".to_string(),
        });
    }

    let total_start = std::time::Instant::now();

    let detect_start = std::time::Instant::now();
    let backend = Backend::detect_and_open(&db_path)?;
    let backend_detection_ms = detect_start.elapsed().as_millis() as u64;

    let query_start = std::time::Instant::now();
    let symbol = backend.lookup(fqn, &db_path.to_string_lossy())?;
    let query_execution_ms = query_start.elapsed().as_millis() as u64;

    let format_start = std::time::Instant::now();
    match cli.output {
        OutputFormat::Human => {
            println!("Symbol: {}", symbol.name);
            println!("Kind: {}", symbol.kind);
            println!("FQN: {}", symbol.fqn.as_deref().unwrap_or("<none>"));
            if let Some(canonical_fqn) = &symbol.canonical_fqn {
                println!("Canonical FQN: {}", canonical_fqn);
            }
            if let Some(display_fqn) = &symbol.display_fqn {
                println!("Display FQN: {}", display_fqn);
            }
            println!(
                "Location: {}:{}:{}",
                symbol.span.file_path, symbol.span.start_line, symbol.span.start_col
            );
            if let Some(parent) = &symbol.parent {
                println!("Parent: {}", parent);
            }
            if let Some(language) = &symbol.language {
                println!("Language: {}", language);
            }
        }
        OutputFormat::Json | OutputFormat::Pretty => {
            let response = vec![symbol];
            let rendered = if matches!(cli.output, OutputFormat::Pretty) {
                serde_json::to_string_pretty(&response)?
            } else {
                serde_json::to_string(&response)?
            };
            println!("{}", rendered);
        }
    }
    let output_formatting_ms = format_start.elapsed().as_millis() as u64;
    let total_ms = total_start.elapsed().as_millis() as u64;

    if cli.show_metrics {
        eprintln!("Performance metrics:");
        eprintln!("  Backend detection: {}ms", backend_detection_ms);
        eprintln!("  Query execution: {}ms", query_execution_ms);
        eprintln!("  Output formatting: {}ms", output_formatting_ms);
        eprintln!("  Total: {}ms", total_ms);
    }

    Ok(())
}
