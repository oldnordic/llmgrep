use crate::cli::{resolve_db_path, validate_path, Cli};
use llmgrep::backend::Backend;
use llmgrep::error::LlmError;
use llmgrep::output::OutputFormat;
use std::path::Path;

pub fn run_ast(
    cli: &Cli,
    file: &Path,
    position: Option<usize>,
    limit: usize,
) -> Result<(), LlmError> {
    let db_path = resolve_db_path(cli)?;

    let validated_file = validate_path(file, false)?;

    if !validated_file.exists() {
        return Err(LlmError::PathValidationFailed {
            path: file.display().to_string(),
            reason: "File does not exist".to_string(),
        });
    }

    let total_start = std::time::Instant::now();

    let detect_start = std::time::Instant::now();
    let backend = Backend::detect_and_open(&db_path)?;
    let backend_detection_ms = detect_start.elapsed().as_millis() as u64;

    let query_start = std::time::Instant::now();
    let json_value = backend.ast(&validated_file, position, limit)?;
    let query_execution_ms = query_start.elapsed().as_millis() as u64;

    if position.is_none() {
        if let Some(data) = json_value.get("data") {
            if let Some(count) = data.get("count").and_then(|c| c.as_u64()) {
                if count > limit as u64 {
                    eprintln!(
                        "Warning: AST output truncated to {} nodes (total: {})",
                        limit, count
                    );
                    eprintln!("         Use --limit {} to see all nodes.", count);
                }
            }
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
