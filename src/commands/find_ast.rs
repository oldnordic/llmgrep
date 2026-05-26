use crate::cli::{resolve_db_path, Cli};
use llmgrep::backend::Backend;
use llmgrep::error::LlmError;
use llmgrep::output::OutputFormat;

pub fn run_find_ast(cli: &Cli, kind: &str) -> Result<(), LlmError> {
    let db_path = resolve_db_path(cli)?;

    if kind.trim().is_empty() {
        return Err(LlmError::InvalidQuery {
            query: "--kind cannot be empty".to_string(),
        });
    }

    let total_start = std::time::Instant::now();

    let detect_start = std::time::Instant::now();
    let backend = Backend::detect_and_open(&db_path)?;
    let backend_detection_ms = detect_start.elapsed().as_millis() as u64;

    let query_start = std::time::Instant::now();
    let json_value = backend.find_ast(kind)?;
    let query_execution_ms = query_start.elapsed().as_millis() as u64;

    let nodes = if json_value["data"]["nodes"].is_array() {
        json_value["data"]["nodes"].as_array()
    } else {
        json_value["nodes"].as_array()
    };

    if let Some(node_array) = nodes {
        if node_array.is_empty() {
            eprintln!("No AST nodes found with kind '{}'", kind);
            eprintln!("Hint: Check available kinds with: magellan label --list");
            return Ok(());
        }
    }

    let format_start = std::time::Instant::now();
    let rendered = if matches!(cli.output, OutputFormat::Pretty) {
        serde_json::to_string_pretty(&json_value)?
    } else {
        serde_json::to_string(&json_value)?
    };
    let output_formatting_ms = format_start.elapsed().as_millis() as u64;
    let total_ms = total_start.elapsed().as_millis() as u64;

    println!("{}", rendered);

    if cli.show_metrics {
        eprintln!("Performance metrics:");
        eprintln!("  Backend detection: {}ms", backend_detection_ms);
        eprintln!("  Query execution: {}ms", query_execution_ms);
        eprintln!("  Output formatting: {}ms", output_formatting_ms);
        eprintln!("  Total: {}ms", total_ms);
    }

    Ok(())
}
