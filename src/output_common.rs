//! Shared output formatting utilities for all response types.

use crate::error::LlmError;
use crate::output::OutputFormat;
use serde::Serialize;

/// Render any serializable response as JSON with partial result wrapper
pub fn render_json_response<T: Serialize>(
    data: &T,
    partial: bool,
    format: OutputFormat,
) -> Result<String, LlmError> {
    use crate::output::json_response_with_partial;

    let payload = json_response_with_partial(data, partial);
    let rendered = if matches!(format, OutputFormat::Pretty) {
        serde_json::to_string_pretty(&payload)
    } else {
        serde_json::to_string(&payload)
    }?;
    Ok(rendered)
}

/// Format the "total: N" header for human-readable output
pub fn format_total_header(total: u64) -> String {
    format!("total: {}", total)
}

/// Format the "partial: true" footer for human-readable output
pub fn format_partial_footer() -> &'static str {
    "partial: true"
}

/// Check if format is JSON (either Json or Pretty)
pub fn is_json_format(format: OutputFormat) -> bool {
    matches!(format, OutputFormat::Json | OutputFormat::Pretty)
}
