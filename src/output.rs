use chrono::Utc;
use clap::ValueEnum;
use serde::Serialize;
use std::fmt;

const SCHEMA_VERSION: &str = "1.0.0";

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum OutputFormat {
    Human,
    Json,
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

#[derive(Serialize)]
pub struct JsonResponse<T> {
    pub schema_version: &'static str,
    pub execution_id: String,
    pub tool: &'static str,
    pub timestamp: String,
    pub partial: bool,
    pub data: T,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub code: String,
    pub error: String,
    pub message: String,
    pub span: Option<Span>,
    pub remediation: Option<String>,
}

#[derive(Serialize)]
pub struct Span {
    pub span_id: String,
    pub file_path: String,
    pub byte_start: u64,
    pub byte_end: u64,
    pub start_line: u64,
    pub start_col: u64,
    pub end_line: u64,
    pub end_col: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<SpanContext>,
}

#[derive(Serialize)]
pub struct SpanContext {
    pub before: Vec<String>,
    pub selected: Vec<String>,
    pub after: Vec<String>,
    pub truncated: bool,
}

#[derive(Serialize)]
pub struct SymbolMatch {
    pub match_id: String,
    pub span: Span,
    pub name: String,
    pub kind: String,
    pub parent: Option<String>,
    pub symbol_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fqn: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canonical_fqn: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_fqn: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_kind_from_chunk: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet_truncated: Option<bool>,
}

#[derive(Serialize)]
pub struct ReferenceMatch {
    pub match_id: String,
    pub span: Span,
    pub referenced_symbol: String,
    pub reference_kind: Option<String>,
    pub target_symbol_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_kind_from_chunk: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet_truncated: Option<bool>,
}

#[derive(Serialize)]
pub struct CallMatch {
    pub match_id: String,
    pub span: Span,
    pub caller: String,
    pub callee: String,
    pub caller_symbol_id: Option<String>,
    pub callee_symbol_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_kind_from_chunk: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet_truncated: Option<bool>,
}

#[derive(Serialize)]
pub struct SearchResponse {
    pub results: Vec<SymbolMatch>,
    pub query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_filter: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind_filter: Option<String>,
    pub total_count: u64,
}

#[derive(Serialize)]
pub struct ReferenceSearchResponse {
    pub results: Vec<ReferenceMatch>,
    pub query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_filter: Option<String>,
    pub total_count: u64,
}

#[derive(Serialize)]
pub struct CallSearchResponse {
    pub results: Vec<CallMatch>,
    pub query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_filter: Option<String>,
    pub total_count: u64,
}

#[derive(Serialize)]
pub struct CombinedSearchResponse {
    pub query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_filter: Option<String>,
    pub symbols: SearchResponse,
    pub references: ReferenceSearchResponse,
    pub calls: CallSearchResponse,
    pub total_count: u64,
    pub limit_mode: String,
}

pub fn json_response<T>(data: T) -> JsonResponse<T> {
    json_response_with_partial(data, false)
}

pub fn json_response_with_partial<T>(data: T, partial: bool) -> JsonResponse<T> {
    JsonResponse {
        schema_version: SCHEMA_VERSION,
        execution_id: execution_id(),
        tool: "llmgrep",
        timestamp: Utc::now().to_rfc3339(),
        partial,
        data,
    }
}

pub fn execution_id() -> String {
    let timestamp = Utc::now().timestamp();
    let pid = std::process::id();
    format!("{:x}-{:x}", timestamp, pid)
}
