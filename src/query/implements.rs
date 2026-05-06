//! Implements search implementation.
//!
//! This module provides implements search functionality for finding
//! type-trait implementation relationships.

use crate::error::LlmError;
use crate::output::{ImplementsMatch, ImplementsSearchResponse};
use crate::query::builder::build_implements_query;
use crate::query::chunks::search_chunks_by_span;
use crate::query::options::SearchOptions;
use crate::query::util::{
    json_extract, match_id, score_match, snippet_from_file, span_context_from_file, span_id,
    MAX_REGEX_SIZE,
};
use crate::safe_extraction::extract_symbol_content_safe;
use crate::SortMode;
use regex::RegexBuilder;
use rusqlite::{params_from_iter, Connection, ErrorCode, OpenFlags};
use std::collections::HashMap;

/// Internal implementation of search_implements that takes an explicit Connection.
pub(crate) fn search_implements_impl(
    conn: &Connection,
    options: &SearchOptions,
) -> Result<(ImplementsSearchResponse, bool), LlmError> {
    let (sql, params) = build_implements_query(
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

    let compute_scores = options.sort_by == SortMode::Relevance;

    while let Some(row) = rows.next()? {
        let type_data: String = row.get(0)?;
        let type_name: String = row.get(1)?;
        let type_file_path: String = row.get(2)?;
        let trait_name: String = row.get(3)?;
        let trait_file_path: String = row.get(4)?;
        let trait_data: String = row.get(5)?;
        let type_byte_start: u64 = json_extract(&type_data, "byte_start").unwrap_or(0);
        let type_byte_end: u64 = json_extract(&type_data, "byte_end").unwrap_or(0);
        let type_start_line: u64 = json_extract(&type_data, "start_line").unwrap_or(0);
        let type_start_col: u64 = json_extract(&type_data, "start_col").unwrap_or(0);
        let type_end_line: u64 = json_extract(&type_data, "end_line").unwrap_or(0);
        let type_end_col: u64 = json_extract(&type_data, "end_col").unwrap_or(0);
        let type_symbol_id: Option<String> = json_extract(&type_data, "symbol_id");

        let _trait_byte_start: u64 = json_extract(&trait_data, "byte_start").unwrap_or(0);
        let _trait_byte_end: u64 = json_extract(&trait_data, "byte_end").unwrap_or(0);
        let _trait_start_line: u64 = json_extract(&trait_data, "start_line").unwrap_or(0);
        let trait_symbol_id: Option<String> = json_extract(&trait_data, "symbol_id");

        // Filter by query matching type or trait name
        if let Some(ref pattern) = regex {
            if !pattern.is_match(&type_name) && !pattern.is_match(&trait_name) {
                continue;
            }
        } else if !type_name.contains(options.query) && !trait_name.contains(options.query) {
            continue;
        }

        // Path filter: match against type or trait file path
        if let Some(path) = options.path_filter {
            let path_str = path.to_string_lossy();
            if !type_file_path.contains(path_str.as_ref())
                && !trait_file_path.contains(path_str.as_ref())
            {
                continue;
            }
        }

        let score = if compute_scores {
            let type_score = score_match(options.query, &type_name, "", "", regex.as_ref());
            let trait_score = score_match(options.query, &trait_name, "", "", regex.as_ref());
            type_score.max(trait_score)
        } else {
            0
        };

        let context = if options.context.include {
            let capped = options.context.lines > options.context.max_lines;
            let effective_lines = options.context.lines.min(options.context.max_lines);
            span_context_from_file(
                &type_file_path,
                type_start_line,
                type_end_line,
                effective_lines,
                capped,
                &mut file_cache,
            )
        } else {
            None
        };

        let (snippet, snippet_truncated, content_hash, symbol_kind_from_chunk) =
            if options.snippet.include {
                match search_chunks_by_span(conn, &type_file_path, type_byte_start, type_byte_end) {
                    Ok(Some(chunk)) => {
                        let content_bytes = chunk.content.as_bytes();
                        let capped_end = content_bytes.len().min(options.snippet.max_bytes);
                        let truncated = capped_end < content_bytes.len();

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
                        let (snippet, truncated) = snippet_from_file(
                            &type_file_path,
                            type_byte_start,
                            type_byte_end,
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
            span_id: span_id(&type_file_path, type_byte_start, type_byte_end),
            file_path: type_file_path.clone(),
            byte_start: type_byte_start,
            byte_end: type_byte_end,
            start_line: type_start_line,
            start_col: type_start_col,
            end_line: type_end_line,
            end_col: type_end_col,
            context,
        };
        let name = format!("{} impl {}", type_name, trait_name);
        let match_id = match_id(&type_file_path, type_byte_start, type_byte_end, &name);
        results.push(ImplementsMatch {
            match_id,
            span,
            type_name,
            trait_name,
            type_symbol_id,
            trait_symbol_id,
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
        let (count_sql, count_params) = build_implements_query(
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
        ImplementsSearchResponse {
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

/// Public wrapper for search_implements that handles connection opening and validation.
pub fn search_implements(
    options: SearchOptions,
) -> Result<(ImplementsSearchResponse, bool), LlmError> {
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

    search_implements_impl(&conn, &options)
}
