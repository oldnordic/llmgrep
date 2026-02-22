//! Call search implementation.
//!
//! This module provides call search functionality for finding
//! outgoing function calls from symbols.

use crate::error::LlmError;
use crate::output::{CallMatch, CallSearchResponse};
use crate::query::builder::build_call_query;
use crate::query::chunks::search_chunks_by_span;
use crate::query::options::SearchOptions;
use crate::query::util::{
    match_id, score_match, snippet_from_file, span_context_from_file, span_id, CallNodeData,
    MAX_REGEX_SIZE,
};
use crate::safe_extraction::extract_symbol_content_safe;
use crate::SortMode;
use regex::RegexBuilder;
use rusqlite::{params_from_iter, Connection, ErrorCode, OpenFlags};
use std::collections::HashMap;

/// Internal implementation of search_calls that takes an explicit Connection.
///
/// This function contains the core SQL query logic for searching function calls.
/// It is separated from the public `search_calls()` to enable reuse
/// within the SqliteBackend trait implementation.
pub(crate) fn search_calls_impl(
    conn: &Connection,
    options: &SearchOptions,
) -> Result<(CallSearchResponse, bool), LlmError> {
    let (sql, params) = build_call_query(
        options.query,
        options.path_filter,
        options.use_regex,
        false,
        options.candidates,
    );
    let mut stmt = conn.prepare_cached(&sql)?;
    let mut rows = stmt.query(params_from_iter(params))?;
    let regex = if options.use_regex {
        Some(
            RegexBuilder::new(options.query)
                .size_limit(MAX_REGEX_SIZE)
                .build()
                .map_err(|e| LlmError::RegexRejected {
                    reason: format!("Regex too complex or invalid: {}", e),
                })?,
        )
    } else {
        None
    };
    let mut file_cache = HashMap::new();
    let mut results = Vec::new();

    // Only compute scores for Relevance mode (Position mode skips scoring for performance)
    let compute_scores = options.sort_by == SortMode::Relevance;

    while let Some(row) = rows.next()? {
        let data: String = row.get(0)?;
        let call: CallNodeData = serde_json::from_str(&data)?;

        if let Some(ref pattern) = regex {
            if !pattern.is_match(&call.caller) && !pattern.is_match(&call.callee) {
                continue;
            }
        } else if !call.caller.contains(options.query) && !call.callee.contains(options.query) {
            continue;
        }

        // Only compute scores in Relevance mode (Position mode skips scoring for performance)
        let score = if compute_scores {
            let caller_score = score_match(options.query, &call.caller, "", "", regex.as_ref());
            let callee_score = score_match(options.query, &call.callee, "", "", regex.as_ref());
            caller_score.max(callee_score)
        } else {
            0
        };

        let context = if options.context.include {
            let capped = options.context.lines > options.context.max_lines;
            let effective_lines = options.context.lines.min(options.context.max_lines);
            span_context_from_file(
                &call.file,
                call.start_line,
                call.end_line,
                effective_lines,
                capped,
                &mut file_cache,
            )
        } else {
            None
        };
        let (snippet, snippet_truncated, content_hash, symbol_kind_from_chunk) =
            if options.snippet.include {
                // Try chunks table first for faster, pre-validated content
                match search_chunks_by_span(conn, &call.file, call.byte_start, call.byte_end) {
                    Ok(Some(chunk)) => {
                        // Apply max_bytes limit to chunk content
                        let content_bytes = chunk.content.as_bytes();
                        let capped_end = content_bytes.len().min(options.snippet.max_bytes);
                        let truncated = capped_end < content_bytes.len();

                        // Safe UTF-8 slice at character boundary
                        let snippet_content = if capped_end < content_bytes.len() {
                            match extract_symbol_content_safe(content_bytes, 0, capped_end) {
                                Some(s) => s,
                                None => chunk.content.chars().take(capped_end).collect(),
                            }
                        } else {
                            chunk.content.clone()
                        };

                        (
                            Some(snippet_content),
                            Some(truncated),
                            Some(chunk.content_hash),
                            chunk.symbol_kind,
                        )
                    }
                    Ok(None) | Err(_) => {
                        // Chunk not found or error, fall back to file I/O
                        let (snippet, truncated) = snippet_from_file(
                            &call.file,
                            call.byte_start,
                            call.byte_end,
                            options.snippet.max_bytes,
                            &mut file_cache,
                        );
                        (snippet, truncated, None, None)
                    }
                }
            } else {
                (None, None, None, None)
            };

        let span = crate::output::Span {
            span_id: span_id(&call.file, call.byte_start, call.byte_end),
            file_path: call.file.clone(),
            byte_start: call.byte_start,
            byte_end: call.byte_end,
            start_line: call.start_line,
            start_col: call.start_col,
            end_line: call.end_line,
            end_col: call.end_col,
            context,
        };
        let name = format!("{}->{}", call.caller, call.callee);
        let match_id = match_id(&call.file, call.byte_start, call.byte_end, &name);
        results.push(CallMatch {
            match_id,
            span,
            caller: call.caller,
            callee: call.callee,
            caller_symbol_id: call.caller_symbol_id,
            callee_symbol_id: call.callee_symbol_id,
            score: if options.include_score {
                Some(score)
            } else {
                None
            },
            content_hash,
            symbol_kind_from_chunk,
            snippet,
            snippet_truncated,
        });
    }

    let mut partial = false;
    let total_count = if options.use_regex {
        if results.len() >= options.candidates {
            partial = true;
        }
        results.len() as u64
    } else {
        let (count_sql, count_params) = build_call_query(
            options.query,
            options.path_filter,
            options.use_regex,
            true,
            0,
        );
        let count = conn.query_row(&count_sql, params_from_iter(count_params), |row| row.get(0))?;
        if options.candidates < count as usize {
            partial = true;
        }
        count
    };

    // Only sort by score in Relevance mode (Position mode relies on SQL ORDER BY)
    if compute_scores {
        results.sort_by(|a, b| {
            b.score
                .unwrap_or(0)
                .cmp(&a.score.unwrap_or(0))
                .then_with(|| a.span.start_line.cmp(&b.span.start_line))
                .then_with(|| a.span.start_col.cmp(&b.span.start_col))
                .then_with(|| a.span.byte_start.cmp(&b.span.byte_start))
        });
    }
    results.truncate(options.limit);

    Ok((
        CallSearchResponse {
            results,
            query: options.query.to_string(),
            path_filter: options
                .path_filter
                .map(|path| path.to_string_lossy().to_string()),
            total_count,
        },
        partial,
    ))
}

/// Public wrapper for search_calls that handles connection opening and validation.
///
/// This function opens the database connection, validates it, and delegates to
/// search_calls_impl() for the actual query logic. This maintains backward
/// compatibility while enabling trait method implementation.
pub fn search_calls(options: SearchOptions) -> Result<(CallSearchResponse, bool), LlmError> {
    let conn = match Connection::open_with_flags(options.db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
    {
        Ok(conn) => conn,
        Err(rusqlite::Error::SqliteFailure(err, msg)) => match err.code {
            ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase => {
                return Err(LlmError::DatabaseCorrupted {
                    reason: msg
                        .unwrap_or_else(|| "Database file is invalid or corrupted".to_string()),
                });
            }
            ErrorCode::CannotOpen => {
                return Err(LlmError::DatabaseNotFound {
                    path: options.db_path.display().to_string(),
                });
            }
            _ => return Err(LlmError::from(rusqlite::Error::SqliteFailure(err, msg))),
        },
        Err(e) => return Err(LlmError::from(e)),
    };

    // Force database validation by checking if schema exists
    // This catches "not a database" errors that occur lazily
    conn.query_row(
        "SELECT name FROM sqlite_master WHERE type='table' LIMIT 1",
        [],
        |_| Ok(()),
    )
    .map_err(|e| match e {
        rusqlite::Error::SqliteFailure(err, ref msg) => match err.code {
            ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase => LlmError::DatabaseCorrupted {
                reason: msg
                    .as_ref()
                    .map(|s| s.as_str())
                    .unwrap_or("Database file is invalid or corrupted")
                    .to_string(),
            },
            _ => LlmError::from(e),
        },
        other => LlmError::from(other),
    })?;

    // Call the implementation
    search_calls_impl(&conn, &options)
}
