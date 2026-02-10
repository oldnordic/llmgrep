//! Output formatting and response types for llmgrep.
//!
//! This module defines the public API types for serializing search results
//! in various formats (human-readable, JSON, pretty-printed JSON).

use crate::ast::AstContext;
use chrono::Utc;
use clap::ValueEnum;
use serde::Serialize;
use std::fmt;

const SCHEMA_VERSION: &str = "1.0.0";

/// Output format for search results.
///
/// Determines how search results are displayed to the user.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum OutputFormat {
    /// Human-readable formatted output with colors and indentation
    Human,
    /// Compact JSON output (single line)
    Json,
    /// Pretty-printed JSON with indentation
    Pretty,
}

impl fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            OutputFormat::Human => "human",
            OutputFormat::Json => "json",
            OutputFormat::Pretty => "pretty",
        };
        write!(f, "{}", value)
    }
}

/// Performance metrics for search operations.
///
/// Tracks timing breakdown for different phases of search execution.
/// Used internally for debugging and performance analysis.
#[derive(Serialize, Clone, Debug)]
pub struct PerformanceMetrics {
    /// Time taken to detect backend format (SQLite vs Native-V2) in milliseconds
    pub backend_detection_ms: u64,
    /// Time taken to execute the core search query in milliseconds
    pub query_execution_ms: u64,
    /// Time taken to format results for output in milliseconds
    pub output_formatting_ms: u64,
    /// Total time from start to finish in milliseconds
    pub total_ms: u64,
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self {
            backend_detection_ms: 0,
            query_execution_ms: 0,
            output_formatting_ms: 0,
            total_ms: 0,
        }
    }
}

impl PerformanceMetrics {
    /// Create a new PerformanceMetrics instance with all fields set to zero.
    pub fn new() -> Self {
        Self::default()
    }
}

/// JSON response wrapper with metadata.
///
/// All JSON responses from llmgrep follow this structure for consistency
/// and versioning.
///
/// # Type Parameters
///
/// * `T` - The response data type (e.g., `SearchResponse`, `ReferenceSearchResponse`)
///
/// # Example
///
/// ```json
/// {
///   "schema_version": "1.0.0",
///   "execution_id": "1234567890-abcd",
///   "tool": "llmgrep",
///   "timestamp": "2024-01-15T10:30:00Z",
///   "partial": false,
///   "data": { ... }
/// }
/// ```
#[derive(Serialize)]
pub struct JsonResponse<T> {
    /// Schema version for response structure compatibility
    pub schema_version: &'static str,
    /// Unique execution identifier (timestamp + process ID)
    pub execution_id: String,
    /// Tool name (always "llmgrep")
    pub tool: &'static str,
    /// ISO 8601 timestamp of when the search was executed
    pub timestamp: String,
    /// Whether results are partial (e.g., candidates limit hit)
    pub partial: bool,
    /// Optional performance metrics (only included when requested)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub performance: Option<PerformanceMetrics>,
    /// The actual response data
    pub data: T,
}

/// Error response structure for JSON output.
///
/// Provides structured error information with remediation hints.
#[derive(Serialize)]
pub struct ErrorResponse {
    /// Error code (e.g., "LLM-E001", "LLM-E105")
    pub code: String,
    /// Error category/type
    pub error: String,
    /// Human-readable error message
    pub message: String,
    /// Optional span information for source-level errors
    pub span: Option<Span>,
    /// Suggested remediation steps
    pub remediation: Option<String>,
}

/// Source code location information.
///
/// Represents a contiguous span of source code with line/column information
/// for display and navigation.
#[derive(Serialize)]
pub struct Span {
    /// Unique span identifier
    pub span_id: String,
    /// Absolute path to the source file
    pub file_path: String,
    /// Byte offset from file start (inclusive)
    pub byte_start: u64,
    /// Byte offset from file start (exclusive)
    pub byte_end: u64,
    /// 1-based line number of span start
    pub start_line: u64,
    /// 1-based column number of span start
    pub start_col: u64,
    /// 1-based line number of span end
    pub end_line: u64,
    /// 1-based column number of span end
    pub end_col: u64,
    /// Optional context lines before/after the span
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<SpanContext>,
}

/// Context lines surrounding a span.
///
/// Provides before/after/selected lines for displaying search results
/// with surrounding code context.
#[derive(Serialize)]
pub struct SpanContext {
    /// Lines before the matched span
    pub before: Vec<String>,
    /// The matched lines (the span content)
    pub selected: Vec<String>,
    /// Lines after the matched span
    pub after: Vec<String>,
    /// Whether context was truncated due to size limits
    pub truncated: bool,
}

/// A symbol match from a search operation.
///
/// Represents a single symbol (function, struct, enum, etc.) that matched
/// the search criteria, with all available metadata.
#[derive(Serialize)]
pub struct SymbolMatch {
    /// Unique match identifier
    pub match_id: String,
    /// Source code location
    pub span: Span,
    /// Symbol name (e.g., "function_name")
    pub name: String,
    /// Symbol kind (e.g., "function_item", "struct_item")
    pub kind: String,
    /// Parent symbol name (if nested)
    pub parent: Option<String>,
    /// 32-character BLAKE3 hash symbol ID
    pub symbol_id: Option<String>,
    /// Relevance score (higher = more relevant)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<u64>,
    /// Fully-qualified name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fqn: Option<String>,
    /// Canonical (normalized) fully-qualified name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canonical_fqn: Option<String>,
    /// Display-friendly fully-qualified name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_fqn: Option<String>,
    /// SHA-256 hash of the symbol content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
    /// Symbol kind from code_chunks table (legacy field)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_kind_from_chunk: Option<String>,
    /// Source code snippet
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
    /// Whether the snippet was truncated due to size limits
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet_truncated: Option<bool>,
    // Label fields (language and normalized kind)
    /// Programming language (rust, python, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Normalized symbol kind (lowercase, standardized)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind_normalized: Option<String>,
    // Metrics fields (from symbol_metrics table)
    /// AST complexity score
    #[serde(skip_serializing_if = "Option::is_none")]
    pub complexity_score: Option<u64>,
    /// Number of incoming references (fan-in)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fan_in: Option<u64>,
    /// Number of outgoing calls (fan-out)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fan_out: Option<u64>,
    /// Cyclomatic complexity
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cyclomatic_complexity: Option<u64>,
    // AST fields (from ast_nodes table)
    /// AST context (depth, parent_kind, children, decision_points)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ast_context: Option<AstContext>,
    // Condense fields (SCC membership from magellan condense)
    /// Supernode ID for strongly-connected component members
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supernode_id: Option<String>,
}

/// A reference match from a reference search operation.
///
/// Represents a location where a symbol is referenced (used) in code.
/// Used by the `--mode references` search mode.
#[derive(Serialize)]
pub struct ReferenceMatch {
    /// Unique match identifier
    pub match_id: String,
    /// Source code location of the reference
    pub span: Span,
    /// Name of the symbol being referenced
    pub referenced_symbol: String,
    /// Kind of reference (read, write, call, etc.)
    pub reference_kind: Option<String>,
    /// Symbol ID of the referenced symbol
    pub target_symbol_id: Option<String>,
    /// Relevance score
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<u64>,
    /// SHA-256 hash of the content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
    /// Symbol kind from code_chunks table (legacy field)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_kind_from_chunk: Option<String>,
    /// Source code snippet at the reference location
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
    /// Whether the snippet was truncated
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet_truncated: Option<bool>,
}

/// A call match from a call search operation.
///
/// Represents a function call relationship between a caller and callee.
/// Used by the `--mode calls` search mode.
#[derive(Serialize)]
pub struct CallMatch {
    /// Unique match identifier
    pub match_id: String,
    /// Source code location of the call
    pub span: Span,
    /// Name of the calling symbol
    pub caller: String,
    /// Name of the called symbol
    pub callee: String,
    /// Symbol ID of the caller
    pub caller_symbol_id: Option<String>,
    /// Symbol ID of the callee
    pub callee_symbol_id: Option<String>,
    /// Relevance score
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<u64>,
    /// SHA-256 hash of the content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
    /// Symbol kind from code_chunks table (legacy field)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_kind_from_chunk: Option<String>,
    /// Source code snippet showing the call
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
    /// Whether the snippet was truncated
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet_truncated: Option<bool>,
}

/// Response from a symbol search operation.
///
/// Contains all matching symbols along with search metadata.
#[derive(Serialize)]
pub struct SearchResponse {
    /// List of matching symbols
    pub results: Vec<SymbolMatch>,
    /// The search query string
    pub query: String,
    /// Path filter that was applied (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_filter: Option<String>,
    /// Kind filter that was applied (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind_filter: Option<String>,
    /// Total number of matches (may be greater than results.len() if limited)
    pub total_count: u64,
    /// Optional notice (e.g., results truncated, algorithm applied)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notice: Option<String>,
}

/// Response from a reference search operation.
///
/// Contains all locations where a symbol is referenced.
#[derive(Serialize)]
pub struct ReferenceSearchResponse {
    /// List of reference locations
    pub results: Vec<ReferenceMatch>,
    /// The search query string
    pub query: String,
    /// Path filter that was applied (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_filter: Option<String>,
    /// Total number of reference matches
    pub total_count: u64,
}

/// Response from a call search operation.
///
/// Contains all function call relationships matching the search.
#[derive(Serialize)]
pub struct CallSearchResponse {
    /// List of call relationships
    pub results: Vec<CallMatch>,
    /// The search query string
    pub query: String,
    /// Path filter that was applied (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_filter: Option<String>,
    /// Total number of call matches
    pub total_count: u64,
}

/// Combined response for searches that include symbols, references, and calls.
///
/// Used when `--mode combined` is specified, providing all three types of
/// results in a single response.
#[derive(Serialize)]
pub struct CombinedSearchResponse {
    /// The search query string
    pub query: String,
    /// Path filter that was applied (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_filter: Option<String>,
    /// Symbol search results
    pub symbols: SearchResponse,
    /// Reference search results
    pub references: ReferenceSearchResponse,
    /// Call search results
    pub calls: CallSearchResponse,
    /// Total count across all search modes
    pub total_count: u64,
    /// Description of the limit mode applied (e.g., "unlimited", "per-mode")
    pub limit_mode: String,
}

/// Create a JSON response with the provided data.
///
/// # Type Parameters
///
/// * `T` - The response data type
///
/// # Returns
///
/// A `JsonResponse` wrapper with metadata and the provided data.
pub fn json_response<T>(data: T) -> JsonResponse<T> {
    json_response_with_partial(data, false)
}

/// Create a JSON response with a partial flag.
///
/// # Type Parameters
///
/// * `T` - The response data type
///
/// # Arguments
///
/// * `data` - The response data
/// * `partial` - Whether results are partial (e.g., hit candidate limit)
///
/// # Returns
///
/// A `JsonResponse` wrapper with the partial flag set.
pub fn json_response_with_partial<T>(data: T, partial: bool) -> JsonResponse<T> {
    json_response_with_partial_and_performance(data, partial, None)
}

/// Create a JSON response with partial flag and performance metrics.
///
/// # Type Parameters
///
/// * `T` - The response data type
///
/// # Arguments
///
/// * `data` - The response data
/// * `partial` - Whether results are partial
/// * `performance` - Optional performance metrics
///
/// # Returns
///
/// A fully-populated `JsonResponse` wrapper.
pub fn json_response_with_partial_and_performance<T>(
    data: T,
    partial: bool,
    performance: Option<PerformanceMetrics>,
) -> JsonResponse<T> {
    JsonResponse {
        schema_version: SCHEMA_VERSION,
        execution_id: execution_id(),
        tool: "llmgrep",
        timestamp: Utc::now().to_rfc3339(),
        partial,
        performance,
        data,
    }
}

/// Generate a unique execution ID.
///
/// Combines the current Unix timestamp with the process ID for uniqueness.
///
/// # Returns
///
/// A hexadecimal string in the format `{timestamp}-{pid}`.
pub fn execution_id() -> String {
    let timestamp = Utc::now().timestamp();
    let pid = std::process::id();
    format!("{:x}-{:x}", timestamp, pid)
}
