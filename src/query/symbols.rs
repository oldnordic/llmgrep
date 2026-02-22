//! Symbol search implementation.
//!
//! This module provides symbol search functionality with fuzzy matching,
//! filtering, and AST context enrichment.

use crate::algorithm::{apply_algorithm_filters, create_symbol_set_temp_table, SymbolSetStrategy};
use crate::ast::check_ast_table_exists;
use crate::error::LlmError;
use crate::output::{SearchResponse, SymbolMatch};
use crate::query::builder::build_search_query;
use crate::query::chunks::search_chunks_by_span;
use crate::query::options::SearchOptions;
use crate::query::util::{
    infer_language, match_id, normalize_kind_label, score_match, snippet_from_file,
    span_context_from_file, span_id, SymbolNodeData, MAX_REGEX_SIZE,
};
use crate::safe_extraction::extract_symbol_content_safe;
use crate::SortMode;
use regex::RegexBuilder;
use rusqlite::{params_from_iter, Connection, ErrorCode, OpenFlags};
use std::collections::HashMap;
use std::path::Path;

/// Internal implementation of search_symbols that takes an explicit Connection.
///
/// This function contains the core SQL query logic for searching symbols.
/// It is separated from search_symbols() to enable trait method implementation
/// while maintaining backward-compatible wrapper.
pub(crate) fn search_symbols_impl(
    conn: &Connection,
    db_path: &Path,
    options: &SearchOptions,
) -> Result<(SearchResponse, bool, bool), LlmError> {
    // Apply algorithm filters (pre-computed or one-shot execution)
    let (algorithm_symbol_ids, supernode_map, paths_bounded) = if options.algorithm.is_active() {
        apply_algorithm_filters(db_path, &options.algorithm)?
    } else {
        (Vec::new(), HashMap::new(), false)
    };

    // Convert to Option<&Vec<String>> for existing code
    let symbol_set_filter = if algorithm_symbol_ids.is_empty() {
        None
    } else {
        Some(&algorithm_symbol_ids)
    };

    let (sql, params, symbol_set_strategy) = build_search_query(
        options.query,
        options.path_filter,
        options.kind_filter,
        options.language_filter,
        options.use_regex,
        false,
        options.candidates,
        options.metrics,
        options.sort_by,
        options.symbol_id,
        options.fqn_pattern,
        options.exact_fqn,
        false, // has_ast_table - set to false for now, will check properly below
        &[],   // ast_kinds - set to empty for now, will use options.ast.ast_kinds below
        None,  // min_depth
        None,  // max_depth
        None,  // inside_kind
        None,  // contains_kind
        symbol_set_filter,
    );

    // Check if ast_nodes table exists for AST filtering
    let has_ast_table = check_ast_table_exists(conn)
        .map_err(|e| LlmError::SearchFailed {
            reason: format!("Failed to check ast_nodes table: {}", e),
        })?;

    // If we have AST options, rebuild query with correct AST settings
    let (sql, params, symbol_set_strategy) = if !options.ast.ast_kinds.is_empty() || has_ast_table || options.depth.min_depth.is_some() || options.depth.max_depth.is_some() || options.depth.inside.is_some() || options.depth.contains.is_some() {
        build_search_query(
            options.query,
            options.path_filter,
            options.kind_filter,
            options.language_filter,
            options.use_regex,
            false,
            options.candidates,
            options.metrics,
            options.sort_by,
            options.symbol_id,
            options.fqn_pattern,
            options.exact_fqn,
            has_ast_table,
            &options.ast.ast_kinds,
            options.depth.min_depth,
            options.depth.max_depth,
            options.depth.inside,
            options.depth.contains,
            symbol_set_filter,
        )
    } else {
        (sql, params, symbol_set_strategy)
    };

    // Note: temp_table_name will be used in Plan 11-04 for JOIN logic
    let temp_table_name = if symbol_set_strategy == SymbolSetStrategy::TempTable {
        if let Some(ids) = symbol_set_filter {
            Some(create_symbol_set_temp_table(conn, ids)?)
        } else {
            None
        }
    } else {
        None
    };

    let mut stmt = conn.prepare_cached(&sql)?;

    let mut rows = stmt.query(params_from_iter(params))?;
    let mut results = Vec::new();
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

    // Only compute scores for Relevance mode (Position mode skips scoring for performance)
    let compute_scores = options.sort_by == SortMode::Relevance;

    // Check if depth filtering is active (needed for ast_context enrichment)
    let has_depth_filter = options.depth.min_depth.is_some() || options.depth.max_depth.is_some();

    while let Some(row) = rows.next()? {
        let data: String = row.get(0)?;
        let file_path: String = row.get(1)?;
        // Read metrics columns (may be NULL)
        let fan_in: Option<i64> = row.get(2).ok();
        let fan_out: Option<i64> = row.get(3).ok();
        let cyclomatic_complexity: Option<i64> = row.get(4).ok();
        // Read symbol_id column (may be NULL)
        let symbol_id_from_query: Option<String> = row.get(5).ok();

        // Read AST columns (may be NULL if ast_nodes table doesn't exist)
        // Basic AST context is populated from the LEFT JOIN with ast_nodes
        // Enriched fields (depth, parent_kind, children_count_by_kind, decision_points)
        // require additional processing via get_ast_context_for_symbol() when with_ast_context is set
        let ast_context: Option<crate::ast::AstContext> = match row.get::<_, String>("ast_kind").ok() {
            Some(kind) => {
                // All AST columns should be present if ast_kind is present
                match (row.get("ast_id"), row.get("ast_parent_id"), row.get("ast_byte_start"), row.get("ast_byte_end")) {
                    (Ok(ast_id), Ok(parent_id), Ok(byte_start), Ok(byte_end)) => Some(crate::ast::AstContext {
                        ast_id,
                        kind,
                        parent_id,
                        byte_start,
                        byte_end,
                        // Enriched fields start as None - populated later if with_ast_context is set
                        depth: None,
                        parent_kind: None,
                        children_count_by_kind: None,
                        decision_points: None,
                    }),
                    _ => None,
                }
            },
            None => None,
        };

        let symbol: SymbolNodeData = serde_json::from_str(&data)?;

        // Use symbol_id from query if available, otherwise from JSON data
        let symbol_id = symbol_id_from_query.or_else(|| symbol.symbol_id.clone());

        let name = symbol
            .name
            .clone()
            .unwrap_or_else(|| "<unknown>".to_string());
        let display_fqn = symbol.display_fqn.clone().unwrap_or_default();
        let fqn = symbol.fqn.clone().unwrap_or_default();

        if let Some(ref pattern) = regex {
            if !pattern.is_match(&name)
                && !pattern.is_match(&display_fqn)
                && !pattern.is_match(&fqn)
            {
                continue;
            }
        }

        let (snippet, snippet_truncated, content_hash, symbol_kind_from_chunk) =
            if options.snippet.include {
                // Try chunks table first for faster, pre-validated content
                match search_chunks_by_span(conn, &file_path, symbol.byte_start, symbol.byte_end) {
                    Ok(Some(chunk)) => {
                        // Apply max_bytes limit to chunk content
                        let content_bytes = chunk.content.as_bytes();
                        let capped_end = content_bytes.len().min(options.snippet.max_bytes);
                        let truncated = capped_end < content_bytes.len();

                        // Safe UTF-8 slice at character boundary
                        let snippet_content = if capped_end < content_bytes.len() {
                            // Use safe extraction to avoid splitting multi-byte characters
                            match extract_symbol_content_safe(content_bytes, 0, capped_end) {
                                Some(s) => s,
                                None => {
                                    // Fallback to chunk content if safe extraction fails
                                    chunk.content.chars().take(capped_end).collect()
                                }
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
                    Ok(None) => {
                        // Chunk not found, log fallback and use file I/O
                        eprintln!(
                            "Chunk fallback: {}:{}-{}",
                            file_path, symbol.byte_start, symbol.byte_end
                        );
                        let (snippet, truncated) = snippet_from_file(
                            &file_path,
                            symbol.byte_start,
                            symbol.byte_end,
                            options.snippet.max_bytes,
                            &mut file_cache,
                        );
                        (snippet, truncated, None, None)
                    }
                    Err(e) => {
                        // Error querying chunks, fall back to file I/O
                        eprintln!(
                            "Chunk query error for {}:{}-{}: {}, using file I/O",
                            file_path, symbol.byte_start, symbol.byte_end, e
                        );
                        let (snippet, truncated) = snippet_from_file(
                            &file_path,
                            symbol.byte_start,
                            symbol.byte_end,
                            options.snippet.max_bytes,
                            &mut file_cache,
                        );
                        (snippet, truncated, None, None)
                    }
                }
            } else {
                (None, None, None, None)
            };
        let context = if options.context.include {
            let capped = options.context.lines > options.context.max_lines;
            let effective_lines = options.context.lines.min(options.context.max_lines);
            span_context_from_file(
                &file_path,
                symbol.start_line,
                symbol.end_line,
                effective_lines,
                capped,
                &mut file_cache,
            )
        } else {
            None
        };

        let span = crate::output::Span {
            span_id: span_id(&file_path, symbol.byte_start, symbol.byte_end),
            file_path: file_path.clone(),
            byte_start: symbol.byte_start,
            byte_end: symbol.byte_end,
            start_line: symbol.start_line,
            start_col: symbol.start_col,
            end_line: symbol.end_line,
            end_col: symbol.end_col,
            context,
        };

        let match_id = match_id(&file_path, symbol.byte_start, symbol.byte_end, &name);
        // Only compute scores in Relevance mode (Position mode skips scoring for performance)
        let score = if compute_scores {
            score_match(options.query, &name, &display_fqn, &fqn, regex.as_ref())
        } else {
            0
        };
        let fqn = if options.fqn.fqn { symbol.fqn } else { None };
        let canonical_fqn = if options.fqn.canonical_fqn {
            symbol.canonical_fqn
        } else {
            None
        };
        let display_fqn = if options.fqn.display_fqn {
            symbol.display_fqn
        } else {
            None
        };

        // Convert metrics from Option<i64> to Option<u64>
        let complexity_score = None; // Not available in symbol_metrics
        let fan_in = fan_in.and_then(|v| if v >= 0 { Some(v as u64) } else { None });
        let fan_out = fan_out.and_then(|v| if v >= 0 { Some(v as u64) } else { None });
        let cyclomatic_complexity =
            cyclomatic_complexity.and_then(|v| if v >= 0 { Some(v as u64) } else { None });

        // Infer language from file extension
        let language = infer_language(&file_path).map(|s| s.to_string());

        // Normalize kind (prefer kind_normalized from data, otherwise normalize kind)
        let kind_normalized = symbol
            .kind_normalized
            .clone()
            .unwrap_or_else(|| normalize_kind_label(&symbol.kind));

        // Enrich ast_context if --with-ast-context flag is set OR depth filtering is active
        let needs_ast_enrichment = options.ast.with_ast_context || has_depth_filter;
        // Check if we have an active ast_kinds filter that should override the exact-match JOIN result
        let has_ast_kind_filter = !options.ast.ast_kinds.is_empty();
        let ast_context = if needs_ast_enrichment {
            if let Some(mut ctx) = ast_context {
                // If ast_kinds filter is active and the current context doesn't match, use preferred lookup
                if has_ast_kind_filter && !options.ast.ast_kinds.contains(&ctx.kind) {
                    match crate::ast::get_ast_context_for_symbol_with_preference(
                        conn,
                        &file_path,
                        symbol.byte_start,
                        symbol.byte_end,
                        true, // include_enriched
                        &options.ast.ast_kinds,
                    ) {
                        Ok(Some(pref_ctx)) => Some(pref_ctx),
                        Ok(None) => {
                            // No preferred kind found, fall back to enriching the existing context
                            if let Ok(depth) = if has_depth_filter {
                                crate::ast::calculate_decision_depth(conn, ctx.ast_id)
                            } else {
                                crate::ast::calculate_ast_depth(conn, ctx.ast_id)
                            } {
                                ctx.depth = depth;
                            }
                            if let Ok(kind) = crate::ast::get_parent_kind(conn, ctx.parent_id) {
                                ctx.parent_kind = kind;
                            }
                            if let Ok(children) = crate::ast::count_children_by_kind(conn, ctx.ast_id) {
                                ctx.children_count_by_kind = Some(children);
                            }
                            if let Ok(decision_points) = crate::ast::count_decision_points(conn, ctx.ast_id) {
                                ctx.decision_points = Some(decision_points);
                            }
                            Some(ctx)
                        },
                        Err(e) => {
                            eprintln!("Warning: Failed to get preferred AST context: {}", e);
                            if let Ok(depth) = if has_depth_filter {
                                crate::ast::calculate_decision_depth(conn, ctx.ast_id)
                            } else {
                                crate::ast::calculate_ast_depth(conn, ctx.ast_id)
                            } {
                                ctx.depth = depth;
                            }
                            if let Ok(kind) = crate::ast::get_parent_kind(conn, ctx.parent_id) {
                                ctx.parent_kind = kind;
                            }
                            if let Ok(children) = crate::ast::count_children_by_kind(conn, ctx.ast_id) {
                                ctx.children_count_by_kind = Some(children);
                            }
                            if let Ok(decision_points) = crate::ast::count_decision_points(conn, ctx.ast_id) {
                                ctx.decision_points = Some(decision_points);
                            }
                            Some(ctx)
                        }
                    }
                } else {
                    // Populate enriched fields
                    // Use decision depth when depth filtering is active, otherwise use AST depth
                    if has_depth_filter {
                        match crate::ast::calculate_decision_depth(conn, ctx.ast_id) {
                            Ok(depth) => ctx.depth = depth,
                            Err(e) => {
                                eprintln!("Warning: Failed to calculate decision depth: {}", e);
                            }
                        }
                    } else {
                        match crate::ast::calculate_ast_depth(conn, ctx.ast_id) {
                            Ok(depth) => ctx.depth = depth,
                            Err(e) => {
                                eprintln!("Warning: Failed to calculate AST depth: {}", e);
                            }
                        }
                    }
                    match crate::ast::get_parent_kind(conn, ctx.parent_id) {
                        Ok(kind) => ctx.parent_kind = kind,
                        Err(e) => {
                            eprintln!("Warning: Failed to get parent kind: {}", e);
                        }
                    }
                    match crate::ast::count_children_by_kind(conn, ctx.ast_id) {
                        Ok(children) => ctx.children_count_by_kind = Some(children),
                        Err(e) => {
                            eprintln!("Warning: Failed to count children: {}", e);
                        }
                    }
                    match crate::ast::count_decision_points(conn, ctx.ast_id) {
                        Ok(decision_points) => ctx.decision_points = Some(decision_points),
                        Err(e) => {
                            eprintln!("Warning: Failed to count decision points: {}", e);
                        }
                    }
                    Some(ctx)
                }
            } else {
                // Try to get AST context by symbol span if not already populated
                // Pass ast_kinds to prefer nodes matching the filter
                match crate::ast::get_ast_context_for_symbol_with_preference(
                    conn,
                    &file_path,
                    symbol.byte_start,
                    symbol.byte_end,
                    true, // include_enriched
                    &options.ast.ast_kinds,
                ) {
                    Ok(ctx) => ctx,
                    Err(e) => {
                        eprintln!("Warning: Failed to get AST context: {}", e);
                        None
                    }
                }
            }
        } else {
            ast_context
        };

        results.push(SymbolMatch {
            match_id,
            span,
            name,
            kind: symbol.kind,
            parent: None,
            symbol_id: symbol_id.clone(),
            score: if options.include_score {
                Some(score)
            } else {
                None
            },
            fqn,
            canonical_fqn,
            display_fqn,
            content_hash,
            symbol_kind_from_chunk,
            snippet,
            snippet_truncated,
            language,
            kind_normalized: Some(kind_normalized),
            complexity_score,
            fan_in,
            fan_out,
            cyclomatic_complexity,
            ast_context,
            supernode_id: symbol_id.as_ref()
                .and_then(|id| supernode_map.get(id).cloned()),
        });
    }

    // Apply depth filtering if min_depth or max_depth specified
    // This is done post-query due to SQLite recursive CTE limitations
    if has_depth_filter {
        // Filter results by decision depth
        results.retain(|result| {
            // Only filter if we have AST context with ast_id
            if let Some(ref ast_ctx) = result.ast_context {
                match crate::ast::calculate_decision_depth(conn, ast_ctx.ast_id) {
                    Ok(Some(depth)) => {
                        // Check min/max bounds
                        let min_ok = options
                            .depth
                            .min_depth
                            .is_none_or(|m| (depth as usize) >= m);
                        let max_ok = options
                            .depth
                            .max_depth
                            .is_none_or(|m| (depth as usize) <= m);
                        min_ok && max_ok
                    }
                    Ok(None) => true, // No depth data, keep the result
                    Err(_) => true, // Error calculating depth, keep the result
                }
            } else {
                true // No AST context, keep the result
            }
        });
    }

    let mut partial = false;
    let total_count = if options.use_regex {
        if results.len() >= options.candidates {
            partial = true;
        }
        results.len() as u64
    } else {
        let (count_sql, count_params, _symbol_set_strategy) = build_search_query(
            options.query,
            options.path_filter,
            options.kind_filter,
            options.language_filter,
            options.use_regex,
            true,
            0,
            options.metrics,
            options.sort_by,
            options.symbol_id,
            options.fqn_pattern,
            options.exact_fqn,
            has_ast_table,
            &options.ast.ast_kinds,
            options.depth.min_depth,
            options.depth.max_depth,
            options.depth.inside,
            options.depth.contains,
            None,  // symbol_set_filter - will be populated in Plan 11-04
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

    // Ambiguity detection: warn if multiple symbols have the same name
    // Only warn in human mode and when not using symbol_id lookup
    if options.symbol_id.is_none() && !options.use_regex && total_count > 1 {
        // Group results by name to find collisions
        let mut name_groups: std::collections::HashMap<&str, Vec<&SymbolMatch>> =
            std::collections::HashMap::new();
        for result in &results {
            name_groups.entry(&result.name).or_default().push(result);
        }

        // Find names with multiple different canonical_fqns
        for (name, group) in &name_groups {
            let unique_fqns: std::collections::HashSet<_> = group
                .iter()
                .filter_map(|r| r.canonical_fqn.as_ref())
                .collect();

            if unique_fqns.len() > 1 {
                // Multiple symbols with same name but different FQNs
                eprintln!(
                    "Warning: Ambiguous symbol \"{}\" ({} candidates across database)",
                    name, total_count
                );
                eprintln!("Top {} candidates:", group.len().min(5));
                for result in group.iter().take(5) {
                    if let Some(symbol_id) = &result.symbol_id {
                        let fqn = result.canonical_fqn.as_deref().unwrap_or("<unknown FQN>");
                        eprintln!("  - {} (use --symbol-id {})", fqn, symbol_id);
                    }
                }
                eprintln!("Use --symbol-id <id> for precise lookup");
                break; // Only warn once per query
            }
        }
    }

    // Cleanup temporary table if it was created
    if let Some(table_name) = temp_table_name {
        let _ = conn.execute(&format!("DROP TABLE IF EXISTS {}", table_name), []);
    }

    Ok((
        SearchResponse {
            results,
            query: options.query.to_string(),
            path_filter: options
                .path_filter
                .map(|path| path.to_string_lossy().to_string()),
            kind_filter: options.kind_filter.map(|value| value.to_string()),
            total_count,
            notice: None,
        },
        partial,
        paths_bounded,
    ))
}

/// Public wrapper for search_symbols that handles connection opening and validation.
///
/// This function opens the database connection, validates it, and delegates to
/// search_symbols_impl() for the actual query logic. This maintains backward
/// compatibility while enabling trait method implementation.
pub fn search_symbols(options: SearchOptions) -> Result<(SearchResponse, bool, bool), LlmError> {
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
    search_symbols_impl(&conn, options.db_path, &options)
}
