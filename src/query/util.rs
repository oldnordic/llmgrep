//! Utility functions for query operations.
//!
//! This module provides helper functions for file loading, snippet extraction,
/// scoring, and ID generation.

use crate::output::SpanContext;
use regex::Regex;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;

pub(crate) const MAX_REGEX_SIZE: usize = 10_000; // 10KB limit to prevent memory exhaustion

/// Infer programming language from file extension
///
/// Returns standard language label based on file extension.
/// Returns None for unknown extensions.
pub fn infer_language(file_path: &str) -> Option<&'static str> {
    match Path::new(file_path).extension().and_then(|s| s.to_str()) {
        Some("rs") => Some("Rust"),
        Some("py") => Some("Python"),
        Some("js") => Some("JavaScript"),
        Some("ts") => Some("TypeScript"),
        Some("jsx") => Some("JavaScript"),
        Some("tsx") => Some("TypeScript"),
        Some("c") => Some("C"),
        Some("cpp") | Some("cc") | Some("cxx") | Some("hpp") | Some("hxx") => Some("C++"),
        Some("h") => Some("C"), // Assume C for .h files (could also be C++)
        Some("java") => Some("Java"),
        Some("go") => Some("Go"),
        Some("rb") => Some("Ruby"),
        Some("php") => Some("PHP"),
        Some("swift") => Some("Swift"),
        Some("kt") | Some("kts") => Some("Kotlin"),
        Some("scala") => Some("Scala"),
        Some("sh") | Some("bash") => Some("Shell"),
        Some("lua") => Some("Lua"),
        Some("r") => Some("R"),
        Some("m") => Some("Matlab"), // Could also be Objective-C
        Some("cs") => Some("C#"),
        _ => None,
    }
}

/// Normalize symbol kind to standard label name
///
/// Converts various kind representations to lowercase normalized form.
/// Used for populating kind_normalized field in SymbolMatch.
pub(crate) fn normalize_kind_label(kind: &str) -> String {
    kind.to_lowercase()
}

/// Create a LIKE pattern for SQL queries
pub(crate) fn like_pattern(query: &str) -> String {
    let escaped = query
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{}%", escaped)
}

/// Create a LIKE prefix pattern for SQL queries
pub(crate) fn like_prefix(path: &std::path::Path) -> String {
    let raw = path.to_string_lossy().to_string();
    let escaped = raw
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("{}%", escaped)
}

/// Extract the referenced symbol name from a reference name
pub(crate) fn referenced_symbol_from_name(name: &str) -> String {
    name.strip_prefix("ref to ").unwrap_or(name).to_string()
}

/// File cache entry containing file bytes and lines
pub(crate) struct FileCache {
    pub(crate) bytes: Vec<u8>,
    pub(crate) lines: Vec<String>,
}

/// Load a file into the cache
pub(crate) fn load_file<'a>(path: &str, cache: &'a mut HashMap<String, FileCache>) -> Option<&'a FileCache> {
    if !cache.contains_key(path) {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(e) => {
                eprintln!("Warning: Failed to read file '{}': {}", path, e);
                return None;
            }
        };
        let text = String::from_utf8_lossy(&bytes);
        let lines = text.split('\n').map(|line| line.to_string()).collect();
        cache.insert(path.to_string(), FileCache { bytes, lines });
    }
    cache.get(path)
}

/// Extract a snippet from a file
pub(crate) fn snippet_from_file(
    file_path: &str,
    byte_start: u64,
    byte_end: u64,
    max_bytes: usize,
    cache: &mut HashMap<String, FileCache>,
) -> (Option<String>, Option<bool>) {
    if max_bytes == 0 {
        return (None, None);
    }
    let file = match load_file(file_path, cache) {
        Some(file) => file,
        None => return (None, None),
    };
    let start = byte_start as usize;
    let end = byte_end as usize;
    if start >= file.bytes.len() || end > file.bytes.len() || start >= end {
        return (None, None);
    }
    let capped_end = end.min(start + max_bytes);
    let truncated = capped_end < end;

    // Use safe UTF-8 extraction to handle multi-byte characters
    // This prevents panics on emoji, CJK, and accented characters
    let snippet = match crate::safe_extraction::extract_symbol_content_safe(&file.bytes, start, capped_end) {
        Some(s) => s,
        None => {
            // Fallback: if safe extraction fails, use from_utf8_lossy
            // This is less ideal but shouldn't panic
            String::from_utf8_lossy(&file.bytes[start..capped_end]).to_string()
        }
    };

    (Some(snippet), Some(truncated))
}

/// Extract context lines from a file
pub(crate) fn span_context_from_file(
    file_path: &str,
    start_line: u64,
    end_line: u64,
    context_lines: usize,
    capped: bool,
    cache: &mut HashMap<String, FileCache>,
) -> Option<SpanContext> {
    let file = load_file(file_path, cache)?;
    let line_count = file.lines.len() as u64;
    if line_count == 0 {
        return None;
    }
    let start_line = start_line.max(1).min(line_count);
    let end_line = end_line.max(start_line).min(line_count);
    let before_start = start_line.saturating_sub(context_lines as u64).max(1);
    let after_end = (end_line + context_lines as u64).min(line_count);

    let before = file.lines[(before_start - 1) as usize..(start_line - 1) as usize].to_vec();
    let selected = file.lines[(start_line - 1) as usize..end_line as usize].to_vec();
    let after = file.lines[end_line as usize..after_end as usize].to_vec();
    let truncated = capped
        || (context_lines > 0 && (before.len() < context_lines || after.len() < context_lines));

    Some(SpanContext {
        before,
        selected,
        after,
        truncated,
    })
}

/// Score a match based on query string
pub(crate) fn score_match(
    query: &str,
    name: &str,
    display_fqn: &str,
    fqn: &str,
    regex: Option<&Regex>,
) -> u64 {
    let mut score = 0;

    if name == query {
        score = score.max(100);
    }
    if display_fqn == query {
        score = score.max(95);
    }
    if fqn == query {
        score = score.max(90);
    }

    if name.starts_with(query) {
        score = score.max(80);
    }
    if display_fqn.starts_with(query) {
        score = score.max(70);
    }
    if name.contains(query) {
        score = score.max(60);
    }
    if display_fqn.contains(query) {
        score = score.max(50);
    }
    if fqn.contains(query) {
        score = score.max(40);
    }

    if let Some(pattern) = regex {
        if pattern.is_match(name) {
            score = score.max(70);
        } else if pattern.is_match(display_fqn) {
            score = score.max(60);
        } else if pattern.is_match(fqn) {
            score = score.max(50);
        }
    }

    score
}

/// Generate a span ID from file path and byte range
pub(crate) fn span_id(file_path: &str, byte_start: u64, byte_end: u64) -> String {
    let mut hasher = Sha256::new();
    hasher.update(file_path.as_bytes());
    hasher.update(b":");
    hasher.update(byte_start.to_string().as_bytes());
    hasher.update(b":");
    hasher.update(byte_end.to_string().as_bytes());
    let digest = hasher.finalize();
    hex::encode(&digest[..8])
}

/// Generate a match ID from symbol information
pub(crate) fn match_id(file_path: &str, byte_start: u64, byte_end: u64, name: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(name.as_bytes());
    hasher.update(b":");
    hasher.update(file_path.as_bytes());
    hasher.update(b":");
    hasher.update(byte_start.to_string().as_bytes());
    hasher.update(b":");
    hasher.update(byte_end.to_string().as_bytes());
    let digest = hasher.finalize();
    hex::encode(&digest[..8])
}

/// Node data for symbols from JSON
#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
pub(crate) struct SymbolNodeData {
    #[serde(default)]
    pub(crate) symbol_id: Option<String>,
    #[serde(default)]
    pub(crate) name: Option<String>,
    #[serde(default)]
    pub(crate) kind: String,
    #[serde(default)]
    pub(crate) kind_normalized: Option<String>,
    #[serde(default)]
    pub(crate) fqn: Option<String>,
    #[serde(default)]
    pub(crate) canonical_fqn: Option<String>,
    #[serde(default)]
    pub(crate) display_fqn: Option<String>,
    pub(crate) byte_start: u64,
    pub(crate) byte_end: u64,
    pub(crate) start_line: u64,
    pub(crate) start_col: u64,
    pub(crate) end_line: u64,
    pub(crate) end_col: u64,
}

/// Node data for references from JSON
#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
pub(crate) struct ReferenceNodeData {
    pub(crate) file: String,
    pub(crate) byte_start: u64,
    pub(crate) byte_end: u64,
    pub(crate) start_line: u64,
    pub(crate) start_col: u64,
    pub(crate) end_line: u64,
    pub(crate) end_col: u64,
}

/// Node data for calls from JSON
#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
pub(crate) struct CallNodeData {
    pub(crate) file: String,
    pub(crate) caller: String,
    pub(crate) callee: String,
    #[serde(default)]
    pub(crate) caller_symbol_id: Option<String>,
    #[serde(default)]
    pub(crate) callee_symbol_id: Option<String>,
    pub(crate) byte_start: u64,
    pub(crate) byte_end: u64,
    pub(crate) start_line: u64,
    pub(crate) start_col: u64,
    pub(crate) end_line: u64,
    pub(crate) end_col: u64,
}
