use crate::cli::{resolve_db_path, Cli};
use llmgrep::backend::Backend;
use llmgrep::error::LlmError;
use llmgrep::output::OutputFormat;

pub fn run_complete(cli: &Cli, prefix: String, limit: usize) -> Result<(), LlmError> {
    let db_path = resolve_db_path(cli)?;

    if prefix.trim().is_empty() {
        return Err(LlmError::InvalidQuery {
            query: "--prefix cannot be empty".to_string(),
        });
    }

    let total_start = std::time::Instant::now();

    let detect_start = std::time::Instant::now();
    let backend = Backend::detect_and_open(&db_path)?;
    let backend_detection_ms = detect_start.elapsed().as_millis() as u64;

    let query_start = std::time::Instant::now();
    let completions = backend.complete(&prefix, limit)?;
    let query_execution_ms = query_start.elapsed().as_millis() as u64;

    let format_start = std::time::Instant::now();
    match cli.output {
        OutputFormat::Human => {
            for completion in &completions {
                println!("{}", completion);
            }
        }
        OutputFormat::Json | OutputFormat::Pretty => {
            use serde_json::json;
            let response = json!({
                "completions": completions,
                "prefix": prefix,
                "count": completions.len()
            });
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
