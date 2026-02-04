use crate::algorithm::{apply_algorithm_filters, create_symbol_set_temp_table, symbol_set_filter_strategy, AlgorithmOptions, SymbolSetStrategy};
use crate::ast::{check_ast_table_exists, AstContext};
use crate::error::LlmError;
use crate::output::{
    CallMatch, CallSearchResponse, ReferenceMatch, ReferenceSearchResponse, SearchResponse,
    SpanContext, SymbolMatch,
};
use crate::safe_extraction::extract_symbol_content_safe;
use crate::SortMode;
use regex::{Regex, RegexBuilder};
use rusqlite::{params_from_iter, Connection, ErrorCode, OpenFlags, ToSql};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[allow(unused_imports)]
use serde_json::json;

const MAX_REGEX_SIZE: usize = 10_000; // 10KB limit to prevent memory exhaustion

/// Code chunk from Magellan's code_chunks table
///
/// Represents pre-extracted code content with SHA-256 hash for deduplication.
/// Chunks are created during Magellan indexing and provide faster snippet retrieval
/// than file I/O.
#[derive(Debug, Clone)]
pub struct CodeChunk {
    pub id: i64,
    pub file_path: String,
    pub byte_start: u64,
    pub byte_end: u64,
    pub content: String,
    pub content_hash: String,
    pub symbol_name: Option<String>,
    pub symbol_kind: Option<String>,
}

/// Options for all search functions
#[derive(Debug, Clone)]
pub struct SearchOptions<'a> {
    /// Database path
    pub db_path: &'a Path,
    /// Search query string
    pub query: &'a str,
    /// Optional path filter
    pub path_filter: Option<&'a PathBuf>,
    /// Optional kind filter (symbols only) - comma-separated values
    pub kind_filter: Option<&'a str>,
    /// Optional language filter (symbols only)
    pub language_filter: Option<&'a str>,
    /// Maximum results to return
    pub limit: usize,
    /// Use regex matching
    pub use_regex: bool,
    /// Candidate limit for filtering
    pub candidates: usize,
    /// Context options
    pub context: ContextOptions,
    /// Snippet options
    pub snippet: SnippetOptions,
    /// FQN options (symbols only)
    pub fqn: FqnOptions,
    /// Include score in results
    pub include_score: bool,
    /// Sorting mode for results
    pub sort_by: SortMode,
    /// Metrics filtering options
    pub metrics: MetricsOptions,
    /// AST filtering options
    pub ast: AstOptions<'a>,
    /// Depth filtering options
    pub depth: DepthOptions<'a>,
    /// Algorithm-based filtering options
    pub algorithm: AlgorithmOptions<'a>,
    /// SymbolId for direct BLAKE3 hash lookup (overrides name-based search)
    pub symbol_id: Option<&'a str>,
    /// FQN pattern filter (LIKE match on canonical_fqn)
    pub fqn_pattern: Option<&'a str>,
    /// Exact FQN filter (exact match on canonical_fqn)
    pub exact_fqn: Option<&'a str>,
}

/// Context extraction options
#[derive(Debug, Clone, Copy, Default)]
pub struct ContextOptions {
    /// Include context
    pub include: bool,
    /// Lines of context before/after
    pub lines: usize,
    /// Maximum context lines
    pub max_lines: usize,
}

/// Snippet extraction options
#[derive(Debug, Clone, Copy, Default)]
pub struct SnippetOptions {
    /// Include snippet
    pub include: bool,
    /// Maximum snippet bytes
    pub max_bytes: usize,
}

/// FQN inclusion options (symbols only)
#[derive(Debug, Clone, Copy, Default)]
pub struct FqnOptions {
    /// Include basic FQN
    pub fqn: bool,
    /// Include canonical FQN
    pub canonical_fqn: bool,
    /// Include display FQN
    pub display_fqn: bool,
}

/// Metrics-based filtering options
#[derive(Debug, Clone, Copy, Default)]
pub struct MetricsOptions {
    /// Minimum cyclomatic complexity
    pub min_complexity: Option<usize>,
    /// Maximum cyclomatic complexity
    pub max_complexity: Option<usize>,
    /// Minimum fan-in (incoming references)
    pub min_fan_in: Option<usize>,
    /// Minimum fan-out (outgoing calls)
    pub min_fan_out: Option<usize>,
}

/// AST-based filtering options
#[derive(Debug, Clone, Default)]
pub struct AstOptions<'a> {
    /// Filter by AST node kind(s) - can be multiple kinds
    /// When --ast-kind is specified with shorthands or comma-separated values,
    /// this contains the expanded list of node kind strings.
    pub ast_kinds: Vec<String>,
    /// Enable enriched AST context calculation (depth, parent_kind, children, decision_points)
    pub with_ast_context: bool,
    /// Phantom data for lifetime parameter (for future use if needed)
    pub _phantom: std::marker::PhantomData<&'a ()>,
}

impl<'a> AstOptions<'a> {
    /// Create empty AstOptions
    pub fn new() -> Self {
        Self {
            ast_kinds: Vec::new(),
            with_ast_context: false,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Check if any AST kinds are specified
    pub fn has_ast_kinds(&self) -> bool {
        !self.ast_kinds.is_empty()
    }

    /// Get the first AST kind (for backward compatibility)
    pub fn first_ast_kind(&self) -> Option<&str> {
        self.ast_kinds.first().map(|s| s.as_str())
    }
}

/// Depth-based filtering options
#[derive(Debug, Clone, Copy, Default)]
pub struct DepthOptions<'a> {
    /// Minimum nesting depth (decision points only)
    pub min_depth: Option<usize>,
    /// Maximum nesting depth (decision points only)
    pub max_depth: Option<usize>,
    /// Find nodes within parent of this kind (--inside)
    pub inside: Option<&'a str>,
    /// Find parents containing nodes of this kind (--contains)
    pub contains: Option<&'a str>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct SymbolNodeData {
    #[serde(default)]
    symbol_id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    kind_normalized: Option<String>,
    #[serde(default)]
    fqn: Option<String>,
    #[serde(default)]
    canonical_fqn: Option<String>,
    #[serde(default)]
    display_fqn: Option<String>,
    byte_start: u64,
    byte_end: u64,
    start_line: u64,
    start_col: u64,
    end_line: u64,
    end_col: u64,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ReferenceNodeData {
    file: String,
    byte_start: u64,
    byte_end: u64,
    start_line: u64,
    start_col: u64,
    end_line: u64,
    end_col: u64,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct CallNodeData {
    file: String,
    caller: String,
    callee: String,
    #[serde(default)]
    caller_symbol_id: Option<String>,
    #[serde(default)]
    callee_symbol_id: Option<String>,
    byte_start: u64,
    byte_end: u64,
    start_line: u64,
    start_col: u64,
    end_line: u64,
    end_col: u64,
}

/// Infer programming language from file extension
///
/// Returns standard language label based on file extension.
/// Returns None for unknown extensions.
fn infer_language(file_path: &str) -> Option<&'static str> {
    match Path::new(file_path).extension().and_then(|s| s.to_str()) {
        Some("rs") => Some("rust"),
        Some("py") => Some("python"),
        Some("js") => Some("javascript"),
        Some("ts") => Some("typescript"),
        Some("jsx") => Some("javascript"),
        Some("tsx") => Some("typescript"),
        Some("c") => Some("c"),
        Some("cpp") | Some("cc") | Some("cxx") | Some("hpp") | Some("hxx") => Some("cpp"),
        Some("h") => Some("c"), // Assume C for .h files (could also be C++)
        Some("java") => Some("java"),
        Some("go") => Some("go"),
        Some("rb") => Some("ruby"),
        Some("php") => Some("php"),
        Some("swift") => Some("swift"),
        Some("kt") | Some("kts") => Some("kotlin"),
        Some("scala") => Some("scala"),
        Some("sh") | Some("bash") => Some("shell"),
        Some("lua") => Some("lua"),
        Some("r") => Some("r"),
        Some("m") => Some("matlab"), // Could also be Objective-C
        Some("cs") => Some("csharp"),
        _ => None,
    }
}

/// Normalize symbol kind to standard label name
///
/// Converts various kind representations to lowercase normalized form.
/// Used for populating kind_normalized field in SymbolMatch.
fn normalize_kind_label(kind: &str) -> String {
    kind.to_lowercase()
}

/// Query the code_chunks table by symbol name
///
/// Searches for chunks associated with a specific symbol name.
/// Returns all matching chunks since a symbol may have multiple chunks.
pub fn search_chunks_by_symbol_name(
    conn: &Connection,
    symbol_name: &str,
) -> Result<Vec<CodeChunk>, LlmError> {
    let sql = r#"
        SELECT id, file_path, byte_start, byte_end, content, content_hash, symbol_name, symbol_kind
        FROM code_chunks
        WHERE symbol_name = ?
    "#;

    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([symbol_name], |row| {
        Ok(CodeChunk {
            id: row.get(0)?,
            file_path: row.get(1)?,
            byte_start: row.get(2)?,
            byte_end: row.get(3)?,
            content: row.get(4)?,
            content_hash: row.get(5)?,
            symbol_name: row.get(6)?,
            symbol_kind: row.get(7)?,
        })
    })?;

    let mut chunks = Vec::new();
    for row in rows {
        chunks.push(row?);
    }
    Ok(chunks)
}

/// Query the code_chunks table by span
///
/// Searches for a chunk at the exact file path and byte range.
/// Returns None if no chunk exists for this span.
pub fn search_chunks_by_span(
    conn: &Connection,
    file_path: &str,
    byte_start: u64,
    byte_end: u64,
) -> Result<Option<CodeChunk>, LlmError> {
    let sql = r#"
        SELECT id, file_path, byte_start, byte_end, content, content_hash, symbol_name, symbol_kind
        FROM code_chunks
        WHERE file_path = ? AND byte_start = ? AND byte_end = ?
        LIMIT 1
    "#;

    let mut stmt = conn.prepare(sql)?;
    let mut rows = stmt.query_map(
        rusqlite::params![file_path, byte_start as i64, byte_end as i64],
        |row| {
            Ok(CodeChunk {
                id: row.get(0)?,
                file_path: row.get(1)?,
                byte_start: row.get(2)?,
                byte_end: row.get(3)?,
                content: row.get(4)?,
                content_hash: row.get(5)?,
                symbol_name: row.get(6)?,
                symbol_kind: row.get(7)?,
            })
        },
    )?;

    match rows.next() {
        Some(Ok(chunk)) => Ok(Some(chunk)),
        Some(Err(e)) => Err(LlmError::from(e)),
        None => Ok(None),
    }
}

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

    // Apply algorithm filters (pre-computed or one-shot execution)
    let (algorithm_symbol_ids, supernode_map, paths_bounded) = if options.algorithm.is_active() {
        apply_algorithm_filters(options.db_path, &options.algorithm)?
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
    let has_ast_table = check_ast_table_exists(&conn)
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
            Some(create_symbol_set_temp_table(&conn, ids)?)
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
        let ast_context: Option<AstContext> = match row.get::<_, String>("ast_kind").ok() {
            Some(kind) => {
                // All AST columns should be present if ast_kind is present
                match (row.get("ast_id"), row.get("ast_parent_id"), row.get("ast_byte_start"), row.get("ast_byte_end")) {
                    (Ok(ast_id), Ok(parent_id), Ok(byte_start), Ok(byte_end)) => Some(AstContext {
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
                match search_chunks_by_span(&conn, &file_path, symbol.byte_start, symbol.byte_end) {
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
                        &conn,
                        &file_path,
                        symbol.byte_start,
                        symbol.byte_end,
                        true, // include_enriched
                        &options.ast.ast_kinds,
                    ) {
                        Ok(Some(pref_ctx)) => Some(pref_ctx),
                        Ok(None) => {
                            // No preferred kind found, fall back to enriching the existing context
                            if has_depth_filter {
                                if let Ok(depth) = crate::ast::calculate_decision_depth(&conn, ctx.ast_id) {
                                    ctx.depth = depth;
                                }
                            } else {
                                if let Ok(depth) = crate::ast::calculate_ast_depth(&conn, ctx.ast_id) {
                                    ctx.depth = depth;
                                }
                            }
                            if let Ok(kind) = crate::ast::get_parent_kind(&conn, ctx.parent_id) {
                                ctx.parent_kind = kind;
                            }
                            if let Ok(children) = crate::ast::count_children_by_kind(&conn, ctx.ast_id) {
                                ctx.children_count_by_kind = Some(children);
                            }
                            if let Ok(decision_points) = crate::ast::count_decision_points(&conn, ctx.ast_id) {
                                ctx.decision_points = Some(decision_points);
                            }
                            Some(ctx)
                        },
                        Err(e) => {
                            eprintln!("Warning: Failed to get preferred AST context: {}", e);
                            if has_depth_filter {
                                if let Ok(depth) = crate::ast::calculate_decision_depth(&conn, ctx.ast_id) {
                                    ctx.depth = depth;
                                }
                            } else {
                                if let Ok(depth) = crate::ast::calculate_ast_depth(&conn, ctx.ast_id) {
                                    ctx.depth = depth;
                                }
                            }
                            if let Ok(kind) = crate::ast::get_parent_kind(&conn, ctx.parent_id) {
                                ctx.parent_kind = kind;
                            }
                            if let Ok(children) = crate::ast::count_children_by_kind(&conn, ctx.ast_id) {
                                ctx.children_count_by_kind = Some(children);
                            }
                            if let Ok(decision_points) = crate::ast::count_decision_points(&conn, ctx.ast_id) {
                                ctx.decision_points = Some(decision_points);
                            }
                            Some(ctx)
                        }
                    }
                } else {
                    // Populate enriched fields
                    // Use decision depth when depth filtering is active, otherwise use AST depth
                    if has_depth_filter {
                        match crate::ast::calculate_decision_depth(&conn, ctx.ast_id) {
                            Ok(depth) => ctx.depth = depth,
                            Err(e) => {
                                eprintln!("Warning: Failed to calculate decision depth: {}", e);
                            }
                        }
                    } else {
                        match crate::ast::calculate_ast_depth(&conn, ctx.ast_id) {
                            Ok(depth) => ctx.depth = depth,
                            Err(e) => {
                                eprintln!("Warning: Failed to calculate AST depth: {}", e);
                            }
                        }
                    }
                    match crate::ast::get_parent_kind(&conn, ctx.parent_id) {
                        Ok(kind) => ctx.parent_kind = kind,
                        Err(e) => {
                            eprintln!("Warning: Failed to get parent kind: {}", e);
                        }
                    }
                    match crate::ast::count_children_by_kind(&conn, ctx.ast_id) {
                        Ok(children) => ctx.children_count_by_kind = Some(children),
                        Err(e) => {
                            eprintln!("Warning: Failed to count children: {}", e);
                        }
                    }
                    match crate::ast::count_decision_points(&conn, ctx.ast_id) {
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
                    &conn,
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
        results = results
            .into_iter()
            .filter(|result| {
                // Only filter if we have AST context with ast_id
                if let Some(ref ast_ctx) = result.ast_context {
                    match crate::ast::calculate_decision_depth(&conn, ast_ctx.ast_id) {
                        Ok(Some(depth)) => {
                            // Check min/max bounds
                            let min_ok = options
                                .depth
                                .min_depth
                                .map_or(true, |m| (depth as usize) >= m);
                            let max_ok = options
                                .depth
                                .max_depth
                                .map_or(true, |m| (depth as usize) <= m);
                            min_ok && max_ok
                        }
                        Ok(None) => true, // No depth data, keep the result
                        Err(_) => true, // Error calculating depth, keep the result
                    }
                } else {
                    true // No AST context, keep the result
                }
            })
            .collect();
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
            let unique_fqns: HashSet<_> = group
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

pub fn search_references(
    options: SearchOptions,
) -> Result<(ReferenceSearchResponse, bool), LlmError> {
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

    let (sql, params) = build_reference_query(
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
        let name: String = row.get(1)?;
        let target_symbol_id: Option<String> = row.get(2)?;
        let reference: ReferenceNodeData = serde_json::from_str(&data)?;
        let referenced_symbol = referenced_symbol_from_name(&name);

        if let Some(ref pattern) = regex {
            if !pattern.is_match(&referenced_symbol) {
                continue;
            }
        } else if !referenced_symbol.contains(options.query) {
            continue;
        }

        // Only compute scores in Relevance mode (Position mode skips scoring for performance)
        let score = if compute_scores {
            score_match(options.query, &referenced_symbol, "", "", regex.as_ref())
        } else {
            0
        };
        let context = if options.context.include {
            let capped = options.context.lines > options.context.max_lines;
            let effective_lines = options.context.lines.min(options.context.max_lines);
            span_context_from_file(
                &reference.file,
                reference.start_line,
                reference.end_line,
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
                match search_chunks_by_span(
                    &conn,
                    &reference.file,
                    reference.byte_start,
                    reference.byte_end,
                ) {
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
                            &reference.file,
                            reference.byte_start,
                            reference.byte_end,
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
            span_id: span_id(&reference.file, reference.byte_start, reference.byte_end),
            file_path: reference.file.clone(),
            byte_start: reference.byte_start,
            byte_end: reference.byte_end,
            start_line: reference.start_line,
            start_col: reference.start_col,
            end_line: reference.end_line,
            end_col: reference.end_col,
            context,
        };
        let match_id = match_id(
            &reference.file,
            reference.byte_start,
            reference.byte_end,
            &referenced_symbol,
        );
        results.push(ReferenceMatch {
            match_id,
            span,
            referenced_symbol,
            reference_kind: None,
            target_symbol_id,
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
        let (count_sql, count_params) = build_reference_query(
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
        ReferenceSearchResponse {
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
                match search_chunks_by_span(&conn, &call.file, call.byte_start, call.byte_end) {
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

fn build_search_query(
    query: &str,
    path_filter: Option<&PathBuf>,
    kind_filter: Option<&str>,
    language_filter: Option<&str>,
    use_regex: bool,
    count_only: bool,
    limit: usize,
    metrics: MetricsOptions,
    sort_by: SortMode,
    symbol_id: Option<&str>,
    fqn_pattern: Option<&str>,
    exact_fqn: Option<&str>,
    has_ast_table: bool,
    ast_kinds: &[String],
    _min_depth: Option<usize>,
    _max_depth: Option<usize>,
    inside_kind: Option<&str>,
    contains_kind: Option<&str>,
    symbol_set_filter: Option<&Vec<String>>,
) -> (String, Vec<Box<dyn ToSql>>, SymbolSetStrategy) {
    let mut params: Vec<Box<dyn ToSql>> = Vec::new();
    let mut where_clauses = Vec::new();

    // SymbolId mode: Direct lookup by BLAKE3 hash (bypasses name search)
    if let Some(sid) = symbol_id {
        where_clauses.push("json_extract(s.data, '$.symbol_id') = ?".to_string());
        params.push(Box::new(sid.to_string()));
    } else if !use_regex {
        // Standard name-based search (only if not using symbol_id)
        let like_query = like_pattern(query);
        where_clauses.push(
            "(s.name LIKE ? ESCAPE '\\' OR s.display_fqn LIKE ? ESCAPE '\\' OR s.fqn LIKE ? ESCAPE '\\')"
                .to_string(),
        );
        params.push(Box::new(like_query.clone()));
        params.push(Box::new(like_query.clone()));
        params.push(Box::new(like_query));
    }

    // FQN pattern filter (LIKE match on canonical_fqn)
    if let Some(pattern) = fqn_pattern {
        where_clauses
            .push("json_extract(s.data, '$.canonical_fqn') LIKE ? ESCAPE '\\'".to_string());
        params.push(Box::new(pattern.to_string()));
    }

    // Exact FQN filter (exact match on canonical_fqn)
    if let Some(exact) = exact_fqn {
        where_clauses.push("json_extract(s.data, '$.canonical_fqn') = ?".to_string());
        params.push(Box::new(exact.to_string()));
    }

    if let Some(path) = path_filter {
        where_clauses.push("f.file_path LIKE ? ESCAPE '\\'".to_string());
        params.push(Box::new(like_prefix(path)));
    }

    if let Some(kind) = kind_filter {
        where_clauses.push("(s.kind_normalized = ? OR s.kind = ?)".to_string());
        params.push(Box::new(kind.to_string()));
        params.push(Box::new(kind.to_string()));
    }

    // Language filter: Filter by inferred language from file extension
    // Note: This uses file extension matching since language labels aren't
    // directly stored in graph_entities. A future enhancement could use
    // label tables for faster filtering.
    if let Some(language) = language_filter {
        let extensions = match language {
            "rust" => ".rs",
            "python" => ".py",
            "javascript" => ".js",
            "typescript" => ".ts",
            "c" => ".c",
            "cpp" => ".cpp",
            "java" => ".java",
            "go" => ".go",
            _ => "", // Unknown language - no filter
        };
        if !extensions.is_empty() {
            where_clauses.push("f.file_path LIKE ? ESCAPE '\\'".to_string());
            params.push(Box::new(format!("%{}", extensions)));
        }
    }

    // AST kind filter: Filter by AST node kind(s) using overlap matching
    // This uses an EXISTS subquery to handle cases where AST nodes overlap
    // with symbol spans but don't have exact byte matches
    if !ast_kinds.is_empty() {
        if has_ast_table {
            if ast_kinds.len() == 1 {
                // Single kind - use EXISTS with overlap check
                where_clauses.push(format!(
                    "EXISTS (
                        SELECT 1 FROM ast_nodes
                        WHERE kind = ?
                        AND byte_start < json_extract(s.data, '$.byte_end')
                        AND byte_end > json_extract(s.data, '$.byte_start')
                    )"
                ));
                params.push(Box::new(ast_kinds[0].clone()));
            } else {
                // Multiple kinds - use EXISTS with IN and overlap check
                let placeholders = vec!["?"; ast_kinds.len()].join(",");
                where_clauses.push(format!(
                    "EXISTS (
                        SELECT 1 FROM ast_nodes
                        WHERE kind IN ({})
                        AND byte_start < json_extract(s.data, '$.byte_end')
                        AND byte_end > json_extract(s.data, '$.byte_start')
                    )",
                    placeholders
                ));
                for kind in ast_kinds {
                    params.push(Box::new(kind.clone()));
                }
            }
        }
        // If ast_nodes table doesn't exist, we silently ignore the filter
        // (graceful degradation)
    }

    // Add metrics filter WHERE clauses
    // For filters, we use IS NOT NULL to ensure symbols have metrics
    if let Some(min_cc) = metrics.min_complexity {
        where_clauses.push(
            "(sm.cyclomatic_complexity IS NOT NULL AND sm.cyclomatic_complexity >= ?)".to_string(),
        );
        params.push(Box::new(min_cc as i64));
    }
    if let Some(max_cc) = metrics.max_complexity {
        where_clauses.push(
            "(sm.cyclomatic_complexity IS NOT NULL AND sm.cyclomatic_complexity <= ?)".to_string(),
        );
        params.push(Box::new(max_cc as i64));
    }
    if let Some(min_fi) = metrics.min_fan_in {
        where_clauses.push("(sm.fan_in IS NOT NULL AND sm.fan_in >= ?)".to_string());
        params.push(Box::new(min_fi as i64));
    }
    if let Some(min_fo) = metrics.min_fan_out {
        where_clauses.push("(sm.fan_out IS NOT NULL AND sm.fan_out >= ?)".to_string());
        params.push(Box::new(min_fo as i64));
    }

    // Structural search: --inside KIND (find descendants within any ancestor of type KIND)
    if let Some(inside_kind) = inside_kind {
        if has_ast_table {
            // Use a correlated EXISTS subquery with recursive CTE to check all ancestors
            // This handles nested structures like: function -> block -> closure
            where_clauses.push(format!(
                "EXISTS (
                    WITH RECURSIVE ancestors AS (
                        SELECT id, parent_id FROM ast_nodes WHERE id = an.id
                        UNION ALL
                        SELECT a.id, a.parent_id FROM ast_nodes a
                        JOIN ancestors anc ON a.id = anc.parent_id
                        WHERE a.parent_id IS NOT NULL
                    )
                    SELECT 1 FROM ast_nodes p
                    JOIN ancestors anc ON p.id = anc.parent_id
                    WHERE p.kind = ?
                )"
            ));
            params.push(Box::new(inside_kind.to_string()));
        }
    }

    // Structural search: --contains KIND (find parents with children of type KIND)
    if let Some(contains_kind) = contains_kind {
        if has_ast_table {
            where_clauses.push("an.id IN (SELECT parent_id FROM ast_nodes WHERE kind = ? AND parent_id IS NOT NULL)".to_string());
            params.push(Box::new(contains_kind.to_string()));
        }
    }

    // Note: Depth filtering (min_depth, max_depth) is handled post-query
    // due to SQLite recursive CTE limitations in WHERE clauses.
    // See Task 6 for post-query filtering implementation.

    // SymbolSet filter condition
    let symbol_set_strategy = if let Some(symbol_ids) = symbol_set_filter {
        let strategy = symbol_set_filter_strategy(symbol_ids);
        match strategy {
            SymbolSetStrategy::InClause if !symbol_ids.is_empty() => {
                let placeholders = symbol_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
                where_clauses.push(format!("json_extract(s.data, '$.symbol_id') IN ({})", placeholders));
                params.extend(symbol_ids.iter().map(|id| Box::new(id.clone()) as Box<dyn ToSql>));
            }
            SymbolSetStrategy::TempTable => {
                // Will be handled via JOIN in execution (Plan 11-04)
                where_clauses.push("EXISTS (SELECT 1 FROM symbol_set_filter WHERE symbol_set_filter.symbol_id = json_extract(s.data, '$.symbol_id'))".to_string());
            }
            _ => {}
        }
        strategy
    } else {
        SymbolSetStrategy::None
    };

    let select_clause = if count_only {
        "SELECT COUNT(*)"
    } else {
        // Include AST columns when ast_nodes table exists
        if has_ast_table {
            "SELECT s.data, f.file_path, sm.fan_in, sm.fan_out, sm.cyclomatic_complexity, json_extract(s.data, '$.symbol_id') AS symbol_id, an.id AS ast_id, an.kind AS ast_kind, an.parent_id AS ast_parent_id, an.byte_start AS ast_byte_start, an.byte_end AS ast_byte_end"
        } else {
            "SELECT s.data, f.file_path, sm.fan_in, sm.fan_out, sm.cyclomatic_complexity, json_extract(s.data, '$.symbol_id') AS symbol_id"
        }
    };

    let mut sql = format!(
        "{select_clause}
FROM (
    SELECT id,
           data,
           json_extract(data, '$.name') AS name,
           json_extract(data, '$.display_fqn') AS display_fqn,
           json_extract(data, '$.fqn') AS fqn,
           json_extract(data, '$.canonical_fqn') AS canonical_fqn,
           json_extract(data, '$.kind') AS kind,
           json_extract(data, '$.kind_normalized') AS kind_normalized,
           json_extract(data, '$.start_line') AS start_line,
           json_extract(data, '$.start_col') AS start_col,
           json_extract(data, '$.byte_start') AS byte_start,
           json_extract(data, '$.byte_end') AS byte_end
    FROM graph_entities
    WHERE kind = 'Symbol'
) s
JOIN graph_edges e ON e.to_id = s.id AND e.edge_type = 'DEFINES'
JOIN (
    SELECT id, json_extract(data, '$.path') AS file_path
    FROM graph_entities
    WHERE kind = 'File'
) f ON f.id = e.from_id
LEFT JOIN symbol_metrics sm ON s.id = sm.symbol_id
{ast_join}
WHERE {where_clause}",
        select_clause = select_clause,
        ast_join = if has_ast_table {
            // Exact match on byte span - this is the correct approach for Magellan
            // The get_ast_context_for_symbol() function handles overlap matching when needed
            "LEFT JOIN ast_nodes an ON json_extract(s.data, '$.byte_start') = an.byte_start AND json_extract(s.data, '$.byte_end') = an.byte_end".to_string()
        } else {
            "".to_string()
        },
        where_clause = if where_clauses.is_empty() {
            "1=1".to_string()
        } else {
            where_clauses.join(" AND ")
        },
    );

    if !count_only {
        // Determine ORDER BY clause based on sort mode
        let order_by = match sort_by {
            SortMode::FanIn => {
                // Sort by fan_in descending, NULLs last
                "COALESCE(sm.fan_in, 0) DESC, s.start_line, s.start_col, s.byte_start, s.byte_end, s.id"
            }
            SortMode::FanOut => {
                // Sort by fan_out descending, NULLs last
                "COALESCE(sm.fan_out, 0) DESC, s.start_line, s.start_col, s.byte_start, s.byte_end, s.id"
            }
            SortMode::Complexity => {
                // Sort by cyclomatic_complexity descending, NULLs last
                "COALESCE(sm.cyclomatic_complexity, 0) DESC, s.start_line, s.start_col, s.byte_start, s.byte_end, s.id"
            }
            SortMode::AstComplexity => {
                // Sort by AST complexity (cyclomatic_complexity), same as Complexity mode
                "COALESCE(sm.cyclomatic_complexity, 0) DESC, s.start_line, s.start_col, s.byte_start, s.byte_end, s.id"
            }
            SortMode::NestingDepth => {
                // Nesting depth requires post-query calculation
                // Fall back to position ordering for now
                // Future: batch depth calculation then in-memory sort
                "s.start_line, s.start_col, s.byte_start, s.byte_end, s.id"
            }
            SortMode::Position => {
                // Position-based ordering (faster, pure SQL)
                "s.start_line, s.start_col, s.byte_start, s.byte_end, s.id"
            }
            SortMode::Relevance => {
                // Relevance ordering happens in-memory after scoring
                "s.start_line, s.start_col, s.byte_start, s.byte_end, s.id"
            }
        };
        sql.push_str(&format!("\nORDER BY {}\n", order_by));
        sql.push_str("LIMIT ?");
        params.push(Box::new(limit as u64));
    }

    (sql, params, symbol_set_strategy)
}

fn build_reference_query(
    query: &str,
    path_filter: Option<&PathBuf>,
    use_regex: bool,
    count_only: bool,
    limit: usize,
) -> (String, Vec<Box<dyn ToSql>>) {
    let mut params: Vec<Box<dyn ToSql>> = Vec::new();
    let mut where_clauses = vec!["r.kind = 'Reference'".to_string()];

    if !use_regex {
        let like_query = like_pattern(query);
        where_clauses.push("r.name LIKE ? ESCAPE '\\'".to_string());
        params.push(Box::new(like_query));
    }

    if let Some(path) = path_filter {
        where_clauses.push("json_extract(r.data, '$.file') LIKE ? ESCAPE '\\'".to_string());
        params.push(Box::new(like_prefix(path)));
    }

    let select_clause = if count_only {
        "SELECT COUNT(*)"
    } else {
        "SELECT r.data, r.name, json_extract(s.data, '$.symbol_id') AS target_symbol_id"
    };

    let mut sql = format!(
        "{select_clause}
FROM graph_entities r
LEFT JOIN graph_edges e ON e.from_id = r.id AND e.edge_type = 'REFERENCES'
LEFT JOIN graph_entities s ON s.id = e.to_id AND s.kind = 'Symbol'
WHERE {where_clause}",
        select_clause = select_clause,
        where_clause = where_clauses.join(" AND "),
    );

    if !count_only {
        sql.push_str(
            "\nORDER BY json_extract(r.data, '$.start_line'), json_extract(r.data, '$.start_col'), json_extract(r.data, '$.byte_start'), r.id\n",
        );
        sql.push_str("LIMIT ?");
        params.push(Box::new(limit as u64));
    }

    (sql, params)
}

fn build_call_query(
    query: &str,
    path_filter: Option<&PathBuf>,
    use_regex: bool,
    count_only: bool,
    limit: usize,
) -> (String, Vec<Box<dyn ToSql>>) {
    let mut params: Vec<Box<dyn ToSql>> = Vec::new();
    let mut where_clauses = vec!["c.kind = 'Call'".to_string()];

    if !use_regex {
        let like_query = like_pattern(query);
        where_clauses.push(
            "(json_extract(c.data, '$.caller') LIKE ? ESCAPE '\\' OR json_extract(c.data, '$.callee') LIKE ? ESCAPE '\\')"
                .to_string(),
        );
        params.push(Box::new(like_query.clone()));
        params.push(Box::new(like_query));
    }

    if let Some(path) = path_filter {
        where_clauses.push("json_extract(c.data, '$.file') LIKE ? ESCAPE '\\'".to_string());
        params.push(Box::new(like_prefix(path)));
    }

    let select_clause = if count_only {
        "SELECT COUNT(*)"
    } else {
        "SELECT c.data"
    };

    let mut sql = format!(
        "{select_clause}
FROM graph_entities c
WHERE {where_clause}",
        select_clause = select_clause,
        where_clause = where_clauses.join(" AND "),
    );

    if !count_only {
        sql.push_str(
            "\nORDER BY json_extract(c.data, '$.start_line'), json_extract(c.data, '$.start_col'), json_extract(c.data, '$.byte_start'), c.id\n",
        );
        sql.push_str("LIMIT ?");
        params.push(Box::new(limit as u64));
    }

    (sql, params)
}

fn like_pattern(query: &str) -> String {
    let escaped = query
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{}%", escaped)
}

fn like_prefix(path: &Path) -> String {
    let raw = path.to_string_lossy().to_string();
    let escaped = raw
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("{}%", escaped)
}

fn referenced_symbol_from_name(name: &str) -> String {
    name.strip_prefix("ref to ").unwrap_or(name).to_string()
}

struct FileCache {
    bytes: Vec<u8>,
    lines: Vec<String>,
}

fn load_file<'a>(path: &str, cache: &'a mut HashMap<String, FileCache>) -> Option<&'a FileCache> {
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

fn snippet_from_file(
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
    let snippet = match extract_symbol_content_safe(&file.bytes, start, capped_end) {
        Some(s) => s,
        None => {
            // Fallback: if safe extraction fails, use from_utf8_lossy
            // This is less ideal but shouldn't panic
            String::from_utf8_lossy(&file.bytes[start..capped_end]).to_string()
        }
    };

    (Some(snippet), Some(truncated))
}

fn span_context_from_file(
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

fn score_match(
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

fn span_id(file_path: &str, byte_start: u64, byte_end: u64) -> String {
    let mut hasher = Sha256::new();
    hasher.update(file_path.as_bytes());
    hasher.update(b":");
    hasher.update(byte_start.to_string().as_bytes());
    hasher.update(b":");
    hasher.update(byte_end.to_string().as_bytes());
    let digest = hasher.finalize();
    hex::encode(&digest[..8])
}

fn match_id(file_path: &str, byte_start: u64, byte_end: u64, name: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_score_match_empty_query() {
        // Empty query matches everything via starts_with (every string starts with "")
        // This is the current behavior - every name starts with empty string
        let score = score_match("", "any_name", "any_display_fqn", "any_fqn", None);
        assert_eq!(score, 80, "Empty query matches via name.starts_with('')");
    }

    #[test]
    fn test_score_match_exact_name() {
        let score = score_match("foo", "foo", "", "", None);
        assert_eq!(score, 100, "Exact name match should return score 100");
    }

    #[test]
    fn test_score_match_exact_display_fqn() {
        let score = score_match("foo", "", "foo", "", None);
        assert_eq!(score, 95, "Exact display_fqn match should return score 95");
    }

    #[test]
    fn test_score_match_exact_fqn() {
        let score = score_match("foo", "", "", "foo", None);
        assert_eq!(score, 90, "Exact fqn match should return score 90");
    }

    #[test]
    fn test_score_match_name_prefix() {
        let score = score_match("foo", "foobar", "", "", None);
        assert_eq!(score, 80, "Name prefix match should return score 80");
    }

    #[test]
    fn test_score_match_display_fqn_prefix() {
        let score = score_match("foo", "", "foobar", "", None);
        assert_eq!(score, 70, "Display_fqn prefix match should return score 70");
    }

    #[test]
    fn test_score_match_name_contains() {
        let score = score_match("foo", "barfoobar", "", "", None);
        assert_eq!(score, 60, "Name contains match should return score 60");
    }

    #[test]
    fn test_score_match_display_fqn_contains() {
        let score = score_match("foo", "", "barfoobar", "", None);
        assert_eq!(
            score, 50,
            "Display_fqn contains match should return score 50"
        );
    }

    #[test]
    fn test_score_match_fqn_contains() {
        let score = score_match("foo", "", "", "barfoobar", None);
        assert_eq!(score, 40, "Fqn contains match should return score 40");
    }

    #[test]
    fn test_score_match_tie_handling() {
        // Same query against equivalent names should produce equal scores
        let score1 = score_match("test", "test_value", "", "", None);
        let score2 = score_match("test", "test_another", "", "", None);
        assert_eq!(
            score1, score2,
            "Equivalent matches should produce equal scores"
        );
    }

    #[test]
    fn test_score_match_regex_name() {
        let regex = Regex::new("foo.*").ok();
        let score = score_match("foo.*", "foobar", "", "", regex.as_ref());
        assert_eq!(score, 70, "Regex match on name should return score 70");
    }

    #[test]
    fn test_score_match_regex_display_fqn() {
        let regex = Regex::new("foo.*").ok();
        let score = score_match("foo.*", "", "foobar", "", regex.as_ref());
        assert_eq!(
            score, 60,
            "Regex match on display_fqn should return score 60"
        );
    }

    #[test]
    fn test_score_match_regex_fqn() {
        let regex = Regex::new("foo.*").ok();
        let score = score_match("foo.*", "", "", "foobar", regex.as_ref());
        assert_eq!(score, 50, "Regex match on fqn should return score 50");
    }

    #[test]
    fn test_score_match_boundary_max() {
        // Exact name match should cap at 100
        let score = score_match("test", "test", "test", "test", None);
        assert_eq!(score, 100, "Score should never exceed 100");
    }

    #[test]
    fn test_score_match_no_match() {
        let score = score_match("xyz", "abc", "def", "ghi", None);
        assert_eq!(score, 0, "No match should return score 0");
    }

    #[test]
    fn test_score_match_regex_no_match() {
        let regex = Regex::new("xyz.*").ok();
        let score = score_match("xyz.*", "abc", "def", "ghi", regex.as_ref());
        assert_eq!(score, 0, "Regex no match should return score 0");
    }

    #[test]
    fn test_score_match_priority_exact_over_prefix() {
        // Exact match should take priority over prefix match
        let score = score_match("foo", "foo", "foobar", "", None);
        assert_eq!(
            score, 100,
            "Exact name match should take priority over prefix"
        );
    }

    #[test]
    fn test_score_match_priority_prefix_over_contains() {
        // Prefix match should take priority over contains match
        let score = score_match("foo", "foobar", "barfoobar", "", None);
        assert_eq!(score, 80, "Prefix match should take priority over contains");
    }

    #[test]
    fn test_score_match_multiple_matches_highest_score() {
        // When multiple matches exist, highest score should be returned
        let score = score_match("foo", "foo", "foobar", "barfoobar", None);
        assert_eq!(score, 100, "Should return highest score from all matches");
    }

    #[test]
    fn test_score_match_case_sensitive() {
        // Matching should be case-sensitive
        let score1 = score_match("foo", "foo", "", "", None);
        let score2 = score_match("foo", "Foo", "", "", None);
        assert_eq!(score1, 100, "Exact case match should return 100");
        // "Foo" doesn't start with or contain "foo" (case-sensitive)
        assert_eq!(score2, 0, "Different case should not match");
    }

    #[test]
    fn test_score_match_empty_name_field() {
        // Empty fields should be handled correctly
        let score = score_match("foo", "", "", "", None);
        assert_eq!(
            score, 0,
            "All empty fields with non-empty query should return 0"
        );
    }

    // Helper to count parameter placeholders in SQL
    fn count_params(sql: &str) -> usize {
        sql.matches('?').count()
    }

    #[test]
    fn test_build_search_query_basic() {
        let (sql, params, _strategy) = build_search_query(
            "test",
            None,
            None,
            None,
            false,
            false,
            100,
            MetricsOptions::default(),
            SortMode::default(),
            None,
            None,
            None,
            false, // has_ast_table
            &[],   // ast_kinds
            None,  // min_depth
            None,  // max_depth
            None,  // inside_kind
            None,  // contains_kind
            None,  // symbol_set_filter
        );

        // Should have LIKE clauses for name, display_fqn, fqn
        assert!(sql.contains("s.name LIKE ? ESCAPE '\\'"));
        assert!(sql.contains("s.display_fqn LIKE ? ESCAPE '\\'"));
        assert!(sql.contains("s.fqn LIKE ? ESCAPE '\\'"));

        // Should have LIMIT clause
        assert!(sql.contains("LIMIT ?"));

        // Should have 3 LIKE params + 1 LIMIT param
        assert_eq!(params.len(), 4);
        assert_eq!(count_params(&sql), 4);
    }

    #[test]
    fn test_build_search_query_with_kind_filter() {
        let (sql, params, _strategy) = build_search_query(
            "test",
            None,
            Some("Function"),
            None,
            false,
            false,
            100,
            MetricsOptions::default(),
            SortMode::default(),
            None,
            None,
            None,
            false, // has_ast_table
            &[],   // ast_kinds
            None,  // min_depth
            None,  // max_depth
            None,  // inside_kind
            None,  // contains_kind
            None,  // symbol_set_filter
        );

        // Should add kind filter
        assert!(sql.contains("s.kind_normalized = ? OR s.kind = ?"));

        // Should have 3 LIKE params + 2 kind params + 1 LIMIT param
        assert_eq!(params.len(), 6);
        assert_eq!(count_params(&sql), 6);
    }

    #[test]
    fn test_build_search_query_with_path_filter() {
        let path = PathBuf::from("/src/module");
        let (sql, params, _strategy) = build_search_query(
            "test",
            Some(&path),
            None,
            None,
            false,
            false,
            100,
            MetricsOptions::default(),
            SortMode::default(),
            None,
            None,
            None,
            false, // has_ast_table
            &[],   // ast_kinds
            None,  // min_depth
            None,  // max_depth
            None,  // inside_kind
            None,  // contains_kind
            None,  // symbol_set_filter
        );

        // Should add file path filter
        assert!(sql.contains("f.file_path LIKE ? ESCAPE '\\'"));

        // Should have 3 LIKE params + 1 path param + 1 LIMIT param
        assert_eq!(params.len(), 5);
        assert_eq!(count_params(&sql), 5);
    }

    #[test]
    fn test_build_search_query_regex_mode() {
        let (sql, params, _strategy) = build_search_query(
            "test.*",
            None,
            None,
            None,
            true,
            false,
            100,
            MetricsOptions::default(),
            SortMode::default(),
            None,
            None,
            None,
            false, // has_ast_table
            &[],   // ast_kinds
            None,  // min_depth
            None,  // max_depth
            None,  // inside_kind
            None,  // contains_kind
            None,  // symbol_set_filter
        );

        // Should NOT have LIKE clauses in regex mode
        assert!(!sql.contains("LIKE ? ESCAPE '\\'"));

        // Should have LIMIT clause
        assert!(sql.contains("LIMIT ?"));

        // Should only have LIMIT param (no LIKE params)
        assert_eq!(params.len(), 1);
        assert_eq!(count_params(&sql), 1);
    }

    #[test]
    fn test_build_search_query_count_only() {
        let (sql, params, _strategy) = build_search_query(
            "test",
            None,
            None,
            None,
            false,
            true,
            0,
            MetricsOptions::default(),
            SortMode::default(),
            None,
            None,
            None,
            false, // has_ast_table
            &[],   // ast_kinds
            None,  // min_depth
            None,  // max_depth
            None,  // inside_kind
            None,  // contains_kind
            None,  // symbol_set_filter
        );

        // Should start with COUNT
        assert!(sql.starts_with("SELECT COUNT(*)"));

        // Should NOT have LIMIT clause
        assert!(!sql.contains("LIMIT"));

        // Should have 3 LIKE params (no LIMIT param)
        assert_eq!(params.len(), 3);
        assert_eq!(count_params(&sql), 3);
    }

    #[test]
    fn test_build_search_query_regular_query() {
        let (sql, params, _strategy) = build_search_query(
            "test",
            None,
            None,
            None,
            false,
            false,
            100,
            MetricsOptions::default(),
            SortMode::default(),
            None,
            None,
            None,
            false, // has_ast_table
            &[],   // ast_kinds
            None,  // min_depth
            None,  // max_depth
            None,  // inside_kind
            None,  // contains_kind
            None,  // symbol_set_filter
        );

        // Should have ORDER BY
        assert!(sql.contains("ORDER BY"));

        // Should have LIMIT clause
        assert!(sql.contains("LIMIT ?"));

        // Should have params
        assert!(!params.is_empty());
    }

    #[test]
    fn test_build_search_query_with_metrics_fan_in_sort() {
        let (sql, params, _strategy) = build_search_query(
            "test",
            None,
            None,
            None,
            false,
            false,
            100,
            MetricsOptions::default(),
            SortMode::FanIn,
            None,
            None,
            None,
            false, // has_ast_table
            &[],   // ast_kinds
            None,  // min_depth
            None,  // max_depth
            None,  // inside_kind
            None,  // contains_kind
            None,  // symbol_set_filter
        );

        // Should ORDER BY fan_in DESC
        assert!(sql.contains("COALESCE(sm.fan_in, 0) DESC"));

        // Should have basic params
        assert!(!params.is_empty());
    }

    #[test]
    fn test_build_search_query_with_metrics_fan_out_sort() {
        let (sql, params, _strategy) = build_search_query(
            "test",
            None,
            None,
            None,
            false,
            false,
            100,
            MetricsOptions::default(),
            SortMode::FanOut,
            None,
            None,
            None,
            false, // has_ast_table
            &[],   // ast_kinds
            None,  // min_depth
            None,  // max_depth
            None,  // inside_kind
            None,  // contains_kind
            None,  // symbol_set_filter
        );

        // Should ORDER BY fan_out DESC
        assert!(sql.contains("COALESCE(sm.fan_out, 0) DESC"));

        // Should have basic params
        assert!(!params.is_empty());
    }

    #[test]
    fn test_build_search_query_with_metrics_complexity_sort() {
        let (sql, params, _strategy) = build_search_query(
            "test",
            None,
            None,
            None,
            false,
            false,
            100,
            MetricsOptions::default(),
            SortMode::Complexity,
            None,
            None,
            None,
            false, // has_ast_table
            &[],   // ast_kinds
            None,  // min_depth
            None,  // max_depth
            None,  // inside_kind
            None,  // contains_kind
            None,  // symbol_set_filter
        );

        // Should ORDER BY cyclomatic_complexity DESC
        assert!(sql.contains("COALESCE(sm.cyclomatic_complexity, 0) DESC"));

        // Should have basic params
        assert!(!params.is_empty());
    }

    #[test]
    fn test_build_search_query_with_min_complexity_filter() {
        let metrics = MetricsOptions {
            min_complexity: Some(5),
            ..Default::default()
        };
        let (sql, params, _strategy) = build_search_query(
            "test",
            None,
            None,
            None,
            false,
            false,
            100,
            metrics,
            SortMode::default(),
            None,
            None,
            None,
            false, // has_ast_table
            &[],   // ast_kinds
            None,  // min_depth
            None,  // max_depth
            None,  // inside_kind
            None,  // contains_kind
            None,  // symbol_set_filter
        );

        // Should filter by min_complexity
        assert!(sql.contains("sm.cyclomatic_complexity >= ?"));

        // Should have 3 LIKE params + 1 filter param + 1 LIMIT param
        assert_eq!(params.len(), 5);
        assert_eq!(count_params(&sql), 5);
    }

    #[test]
    fn test_build_search_query_with_max_complexity_filter() {
        let metrics = MetricsOptions {
            max_complexity: Some(20),
            ..Default::default()
        };
        let (sql, params, _strategy) = build_search_query(
            "test",
            None,
            None,
            None,
            false,
            false,
            100,
            metrics,
            SortMode::default(),
            None,
            None,
            None,
            false, // has_ast_table
            &[],   // ast_kinds
            None,  // min_depth
            None,  // max_depth
            None,  // inside_kind
            None,  // contains_kind
            None,  // symbol_set_filter
        );

        // Should filter by max_complexity
        assert!(sql.contains("sm.cyclomatic_complexity <= ?"));

        // Should have 3 LIKE params + 1 filter param + 1 LIMIT param
        assert_eq!(params.len(), 5);
        assert_eq!(count_params(&sql), 5);
    }

    #[test]
    fn test_build_search_query_with_min_fan_in_filter() {
        let metrics = MetricsOptions {
            min_fan_in: Some(10),
            ..Default::default()
        };
        let (sql, params, _strategy) = build_search_query(
            "test",
            None,
            None,
            None,
            false,
            false,
            100,
            metrics,
            SortMode::default(),
            None,
            None,
            None,
            false, // has_ast_table
            &[],   // ast_kinds
            None,  // min_depth
            None,  // max_depth
            None,  // inside_kind
            None,  // contains_kind
            None,  // symbol_set_filter
        );

        // Should filter by min_fan_in
        assert!(sql.contains("sm.fan_in >= ?"));

        // Should have 3 LIKE params + 1 filter param + 1 LIMIT param
        assert_eq!(params.len(), 5);
        assert_eq!(count_params(&sql), 5);
    }

    #[test]
    fn test_build_search_query_with_metrics_join() {
        let (sql, _, _) = build_search_query(
            "test",
            None,
            None,
            None,
            false,
            false,
            100,
            MetricsOptions::default(),
            SortMode::default(),
            None,
            None,
            None,
            false, // has_ast_table
            &[],   // ast_kinds
            None,  // min_depth
            None,  // max_depth
            None,  // inside_kind
            None,  // contains_kind
            None,  // symbol_set_filter
        );

        // Should LEFT JOIN symbol_metrics
        assert!(sql.contains("LEFT JOIN symbol_metrics sm"));

        // Should select metrics columns
        assert!(sql.contains("sm.fan_in, sm.fan_out, sm.cyclomatic_complexity"));
    }

    #[test]
    fn test_build_search_query_combined_filters() {
        let metrics = MetricsOptions {
            min_complexity: Some(5),
            max_complexity: Some(20),
            min_fan_in: Some(10),
            ..Default::default()
        };
        let (sql, params, _strategy) = build_search_query(
            "test",
            None,
            None,
            None,
            false,
            false,
            100,
            metrics,
            SortMode::default(),
            None,
            None,
            None,
            false, // has_ast_table
            &[],   // ast_kinds
            None,  // min_depth
            None,  // max_depth
            None,  // inside_kind
            None,  // contains_kind
            None,  // symbol_set_filter
        );

        // Should have all filter clauses
        assert!(sql.contains("sm.cyclomatic_complexity >= ?"));
        assert!(sql.contains("sm.cyclomatic_complexity <= ?"));
        assert!(sql.contains("sm.fan_in >= ?"));

        // Should have 3 LIKE params + 3 filter params + 1 LIMIT param
        assert_eq!(params.len(), 7);
        assert_eq!(count_params(&sql), 7);
    }

    #[test]
    fn test_build_reference_query_basic() {
        let (sql, params) = build_reference_query("test", None, false, false, 100);

        // Should have kind filter
        assert!(sql.contains("r.kind = 'Reference'"));

        // Should join with graph_edges
        assert!(sql.contains("LEFT JOIN graph_edges e"));

        // Should have LIKE clause
        assert!(sql.contains("r.name LIKE ? ESCAPE '\\'"));

        // Should have LIMIT clause
        assert!(sql.contains("LIMIT ?"));

        // Should have 1 LIKE param + 1 LIMIT param
        assert_eq!(params.len(), 2);
        assert_eq!(count_params(&sql), 2);
    }

    #[test]
    fn test_build_reference_query_with_path_filter() {
        let path = PathBuf::from("/src/module");
        let (sql, params) = build_reference_query("test", Some(&path), false, false, 100);

        // Should add file path filter
        assert!(sql.contains("json_extract(r.data, '$.file') LIKE ? ESCAPE '\\'"));

        // Should have 1 LIKE param + 1 path param + 1 LIMIT param
        assert_eq!(params.len(), 3);
        assert_eq!(count_params(&sql), 3);
    }

    #[test]
    fn test_build_reference_query_count_only() {
        let (sql, params) = build_reference_query("test", None, false, true, 0);

        // Should start with COUNT
        assert!(sql.starts_with("SELECT COUNT(*)"));

        // Should NOT have LIMIT clause
        assert!(!sql.contains("LIMIT"));

        // Should have 1 LIKE param (no LIMIT param)
        assert_eq!(params.len(), 1);
        assert_eq!(count_params(&sql), 1);
    }

    #[test]
    fn test_build_call_query_basic() {
        let (sql, params) = build_call_query("test", None, false, false, 100);

        // Should have kind filter
        assert!(sql.contains("c.kind = 'Call'"));

        // Should have json_extract for caller/callee
        assert!(sql.contains("json_extract(c.data, '$.caller')"));
        assert!(sql.contains("json_extract(c.data, '$.callee')"));

        // Should have LIKE clauses for caller/callee
        assert!(sql.contains("json_extract(c.data, '$.caller') LIKE ? ESCAPE '\\'"));
        assert!(sql.contains("json_extract(c.data, '$.callee') LIKE ? ESCAPE '\\'"));

        // Should have LIMIT clause
        assert!(sql.contains("LIMIT ?"));

        // Should have 2 LIKE params + 1 LIMIT param
        assert_eq!(params.len(), 3);
        assert_eq!(count_params(&sql), 3);
    }

    #[test]
    fn test_build_call_query_with_path_filter() {
        let path = PathBuf::from("/src/module");
        let (sql, params) = build_call_query("test", Some(&path), false, false, 100);

        // Should add file path filter
        assert!(sql.contains("json_extract(c.data, '$.file') LIKE ? ESCAPE '\\'"));

        // Should have 2 LIKE params + 1 path param + 1 LIMIT param
        assert_eq!(params.len(), 4);
        assert_eq!(count_params(&sql), 4);
    }

    #[test]
    fn test_build_call_query_count_only() {
        let (sql, params) = build_call_query("test", None, false, true, 0);

        // Should start with COUNT
        assert!(sql.starts_with("SELECT COUNT(*)"));

        // Should NOT have LIMIT clause
        assert!(!sql.contains("LIMIT"));

        // Should have 2 LIKE params (no LIMIT param)
        assert_eq!(params.len(), 2);
        assert_eq!(count_params(&sql), 2);
    }

    #[test]
    fn test_like_pattern_percent_escaping() {
        let result = like_pattern("test%value");
        assert_eq!(result, "%test\\%value%");
    }

    #[test]
    fn test_like_pattern_underscore_escaping() {
        let result = like_pattern("test_value");
        assert_eq!(result, "%test\\_value%");
    }

    #[test]
    fn test_like_pattern_backslash_escaping() {
        let result = like_pattern("test\\value");
        assert_eq!(result, "%test\\\\value%");
    }

    #[test]
    fn test_like_pattern_multiple_special_chars() {
        let result = like_pattern("test%value_\\more");
        assert_eq!(result, "%test\\%value\\_\\\\more%");
    }

    #[test]
    fn test_like_pattern_empty_string() {
        let result = like_pattern("");
        assert_eq!(result, "%%");
    }

    #[test]
    fn test_like_prefix_path() {
        let path = PathBuf::from("/src/path");
        let result = like_prefix(&path);
        assert_eq!(result, "/src/path%");
    }

    #[test]
    fn test_like_prefix_with_percent() {
        let path = PathBuf::from("/src/path%test");
        let result = like_prefix(&path);
        // Should escape the % in the path
        assert_eq!(result, "/src/path\\%test%");
    }

    #[test]
    fn test_like_prefix_with_underscore() {
        let path = PathBuf::from("/src/path_test");
        let result = like_prefix(&path);
        // Should escape the _ in the path
        assert_eq!(result, "/src/path\\_test%");
    }

    #[test]
    fn test_like_prefix_with_backslash() {
        let path = PathBuf::from("C:\\src\\path");
        let result = like_prefix(&path);
        // Should escape backslashes
        assert_eq!(result, "C:\\\\src\\\\path%");
    }

    #[test]
    fn test_build_search_query_combined_filters_path_kind() {
        let path = PathBuf::from("/src/module");
        let (sql, params, _strategy) = build_search_query(
            "test",
            Some(&path),
            Some("Function"),
            None,
            false,
            false,
            100,
            MetricsOptions::default(),
            SortMode::default(),
            None,
            None,
            None,
            false, // has_ast_table
            &[],   // ast_kinds
            None,  // min_depth
            None,  // max_depth
            None,  // inside_kind
            None,  // contains_kind
            None,  // symbol_set_filter
        );

        // Should have all filters
        assert!(sql.contains("s.name LIKE ? ESCAPE '\\'"));
        assert!(sql.contains("f.file_path LIKE ? ESCAPE '\\'"));
        assert!(sql.contains("s.kind_normalized = ? OR s.kind = ?"));

        // Should have 3 LIKE params + 1 path param + 2 kind params + 1 LIMIT param
        assert_eq!(params.len(), 7);
        assert_eq!(count_params(&sql), 7);
    }

    #[test]
    fn test_build_reference_query_regex_mode() {
        let (sql, params) = build_reference_query("test.*", None, true, false, 100);

        // Should NOT have LIKE clauses in regex mode
        assert!(!sql.contains("LIKE ? ESCAPE '\\'"));

        // Should have LIMIT clause
        assert!(sql.contains("LIMIT ?"));

        // Should only have LIMIT param (no LIKE params)
        assert_eq!(params.len(), 1);
        assert_eq!(count_params(&sql), 1);
    }

    #[test]
    fn test_build_call_query_regex_mode() {
        let (sql, params) = build_call_query("test.*", None, true, false, 100);

        // Should NOT have LIKE clauses in regex mode
        assert!(!sql.contains("LIKE ? ESCAPE '\\'"));

        // Should have LIMIT clause
        assert!(sql.contains("LIMIT ?"));

        // Should only have LIMIT param (no LIKE params)
        assert_eq!(params.len(), 1);
        assert_eq!(count_params(&sql), 1);
    }

    // Helper to create a test database with sample data for search_symbols tests
    fn create_test_db() -> (tempfile::NamedTempFile, Connection) {
        let db_file = tempfile::NamedTempFile::new().unwrap();
        let conn = Connection::open(db_file.path()).unwrap();

        // Create schema
        conn.execute(
            "CREATE TABLE graph_entities (
                id INTEGER PRIMARY KEY,
                kind TEXT NOT NULL,
                data TEXT NOT NULL
            )",
            [],
        )
        .unwrap();
        conn.execute(
            "CREATE TABLE graph_edges (
                id INTEGER PRIMARY KEY,
                from_id INTEGER NOT NULL,
                to_id INTEGER NOT NULL,
                edge_type TEXT NOT NULL
            )",
            [],
        )
        .unwrap();
        // Create symbol_metrics table (required for LEFT JOIN in queries)
        conn.execute(
            "CREATE TABLE symbol_metrics (
                symbol_id INTEGER PRIMARY KEY,
                symbol_name TEXT NOT NULL,
                kind TEXT NOT NULL,
                file_path TEXT NOT NULL,
                loc INTEGER NOT NULL DEFAULT 0,
                estimated_loc REAL NOT NULL DEFAULT 0.0,
                fan_in INTEGER NOT NULL DEFAULT 0,
                fan_out INTEGER NOT NULL DEFAULT 0,
                cyclomatic_complexity INTEGER NOT NULL DEFAULT 1,
                last_updated INTEGER NOT NULL DEFAULT 0,
                FOREIGN KEY (symbol_id) REFERENCES graph_entities(id) ON DELETE CASCADE
            )",
            [],
        )
        .unwrap();

        // Insert test File entity
        conn.execute(
            "INSERT INTO graph_entities (id, kind, data) VALUES (1, 'File', '{\"path\":\"/test/file.rs\"}')",
            [],
        ).unwrap();

        // Insert test Symbol entities
        conn.execute(
            "INSERT INTO graph_entities (id, kind, data) VALUES
                (10, 'Symbol', '{\"name\":\"test_func\",\"kind\":\"Function\",\"kind_normalized\":\"function\",\"display_fqn\":\"test_func\",\"fqn\":\"module::test_func\",\"canonical_fqn\":\"/test/file.rs::test_func\",\"symbol_id\":\"sym1\",\"byte_start\":100,\"byte_end\":200,\"start_line\":5,\"start_col\":0,\"end_line\":10,\"end_col\":1}'),
                (11, 'Symbol', '{\"name\":\"TestStruct\",\"kind\":\"Struct\",\"kind_normalized\":\"struct\",\"display_fqn\":\"TestStruct\",\"fqn\":\"module::TestStruct\",\"canonical_fqn\":\"/test/file.rs::TestStruct\",\"symbol_id\":\"sym2\",\"byte_start\":300,\"byte_end\":400,\"start_line\":15,\"start_col\":0,\"end_line\":20,\"end_col\":1}'),
                (12, 'Symbol', '{\"name\":\"helper\",\"kind\":\"Function\",\"kind_normalized\":\"function\",\"display_fqn\":\"helper\",\"fqn\":\"module::helper\",\"canonical_fqn\":\"/test/file.rs::helper\",\"symbol_id\":\"sym3\",\"byte_start\":500,\"byte_end\":600,\"start_line\":25,\"start_col\":0,\"end_line\":30,\"end_col\":1}')",
            [],
        ).unwrap();

        // Insert DEFINES edges from File to Symbols
        conn.execute(
            "INSERT INTO graph_edges (from_id, to_id, edge_type) VALUES (1, 10, 'DEFINES'), (1, 11, 'DEFINES'), (1, 12, 'DEFINES')",
            [],
        ).unwrap();

        (db_file, conn)
    }

    // Public API tests for search_symbols()
    mod pub_api_tests_symbols {
        use super::*;

        #[test]
        fn test_search_symbols_basic() {
            let (_db_file, _conn) = create_test_db();
            let db_path = _db_file.path();

            let options = SearchOptions {
                db_path,
                query: "test_func",
                path_filter: None,
                kind_filter: None,
                limit: 10,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: false,
                sort_by: SortMode::default(),
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (response, partial) = search_symbols(options).unwrap();
            assert!(!partial, "Should not be partial");
            assert_eq!(response.results.len(), 1, "Should find 1 result");
            assert_eq!(
                response.results[0].name, "test_func",
                "Should match test_func"
            );
        }

        #[test]
        fn test_search_symbols_empty_results() {
            let (_db_file, _conn) = create_test_db();
            let db_path = _db_file.path();

            let options = SearchOptions {
                db_path,
                query: "nonexistent",
                path_filter: None,
                kind_filter: None,
                limit: 10,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: false,
                sort_by: SortMode::default(),
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (response, partial) = search_symbols(options).unwrap();
            assert!(!partial, "Should not be partial");
            assert_eq!(response.results.len(), 0, "Should find 0 results");
        }

        #[test]
        fn test_search_symbols_prefix_match() {
            let (_db_file, _conn) = create_test_db();
            let db_path = _db_file.path();

            let options = SearchOptions {
                db_path,
                query: "test",
                path_filter: None,
                kind_filter: None,
                limit: 10,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: false,
                sort_by: SortMode::default(),
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (response, partial) = search_symbols(options).unwrap();
            assert!(!partial, "Should not be partial");
            assert_eq!(response.results.len(), 2, "Should find 2 results");

            let names: Vec<&str> = response.results.iter().map(|r| r.name.as_str()).collect();
            assert!(names.contains(&"test_func"), "Should contain test_func");
            assert!(names.contains(&"TestStruct"), "Should contain TestStruct");
        }

        #[test]
        fn test_search_symbols_contains_match() {
            let (_db_file, _conn) = create_test_db();
            let db_path = _db_file.path();

            let options = SearchOptions {
                db_path,
                query: "helper",
                path_filter: None,
                kind_filter: None,
                limit: 10,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: false,
                sort_by: SortMode::default(),
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (response, partial) = search_symbols(options).unwrap();
            assert!(!partial, "Should not be partial");
            assert_eq!(response.results.len(), 1, "Should find 1 result");
            assert_eq!(response.results[0].name, "helper", "Should match helper");
        }

        #[test]
        fn test_search_symbols_kind_filter() {
            let (_db_file, _conn) = create_test_db();
            let db_path = _db_file.path();

            let options = SearchOptions {
                db_path,
                query: "test",
                path_filter: None,
                kind_filter: Some("Function"),
                limit: 10,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: false,
                sort_by: SortMode::default(),
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (response, partial) = search_symbols(options).unwrap();
            assert!(!partial, "Should not be partial");
            assert_eq!(response.results.len(), 1, "Should find 1 Function result");
            assert_eq!(response.results[0].name, "test_func", "Should be test_func");
            assert_eq!(
                response.results[0].kind, "Function",
                "Should be Function kind"
            );
        }

        #[test]
        fn test_search_symbols_limit() {
            let (_db_file, _conn) = create_test_db();
            let db_path = _db_file.path();

            let options = SearchOptions {
                db_path,
                query: "test",
                path_filter: None,
                kind_filter: None,
                limit: 1,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: false,
                sort_by: SortMode::default(),
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (response, partial) = search_symbols(options).unwrap();
            assert!(!partial, "Should not be partial");
            assert_eq!(
                response.results.len(),
                1,
                "Should return at most 1 result due to limit"
            );
        }

        #[test]
        fn test_search_symbols_regex_mode() {
            let (_db_file, _conn) = create_test_db();
            let db_path = _db_file.path();

            let options = SearchOptions {
                db_path,
                query: "test.*",
                path_filter: None,
                kind_filter: None,
                limit: 10,
                use_regex: true,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: false,
                sort_by: SortMode::default(),
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (response, partial) = search_symbols(options).unwrap();
            assert!(!partial, "Should not be partial");
            assert_eq!(
                response.results.len(),
                1,
                "Should find 1 result matching regex"
            );
            assert_eq!(response.results[0].name, "test_func", "Should be test_func");
        }

        #[test]
        fn test_search_symbols_regex_no_match() {
            let (_db_file, _conn) = create_test_db();
            let db_path = _db_file.path();

            let options = SearchOptions {
                db_path,
                query: "xyz.*",
                path_filter: None,
                kind_filter: None,
                limit: 10,
                use_regex: true,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: false,
                sort_by: SortMode::default(),
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (response, partial) = search_symbols(options).unwrap();
            assert!(!partial, "Should not be partial");
            assert_eq!(response.results.len(), 0, "Should find 0 results");
        }

        #[test]
        fn test_search_symbols_score_exact_match() {
            let (_db_file, _conn) = create_test_db();
            let db_path = _db_file.path();

            let options = SearchOptions {
                db_path,
                query: "test_func",
                path_filter: None,
                kind_filter: None,
                limit: 10,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: true,
                sort_by: SortMode::default(),
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (response, partial) = search_symbols(options).unwrap();
            assert!(!partial, "Should not be partial");
            assert_eq!(response.results.len(), 1, "Should find 1 result");
            assert_eq!(
                response.results[0].score,
                Some(100),
                "Exact match should have score 100"
            );
        }

        #[test]
        fn test_search_symbols_score_prefix_match() {
            let (_db_file, _conn) = create_test_db();
            let db_path = _db_file.path();

            let options = SearchOptions {
                db_path,
                query: "test",
                path_filter: None,
                kind_filter: None,
                limit: 10,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: true,
                sort_by: SortMode::default(),
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (response, partial) = search_symbols(options).unwrap();
            assert!(!partial, "Should not be partial");
            assert_eq!(response.results.len(), 2, "Should find 2 results");

            let test_func = response
                .results
                .iter()
                .find(|r| r.name == "test_func")
                .unwrap();
            assert_eq!(
                test_func.score,
                Some(80),
                "test_func should have prefix score 80"
            );

            let test_struct = response
                .results
                .iter()
                .find(|r| r.name == "TestStruct")
                .unwrap();
            assert_eq!(
                test_struct.score,
                Some(0),
                "TestStruct should have score 0 (case mismatch)"
            );
        }

        #[test]
        fn test_search_symbols_partial_result() {
            let (_db_file, _conn) = create_test_db();
            let db_path = _db_file.path();

            let options = SearchOptions {
                db_path,
                query: "test",
                path_filter: None,
                kind_filter: None,
                limit: 10,
                use_regex: false,
                candidates: 1,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: false,
                sort_by: SortMode::default(),
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (response, partial) = search_symbols(options).unwrap();
            assert!(partial, "Should be partial since candidates < total count");
            assert_eq!(response.results.len(), 1, "Should return at most 1 result");
        }

        #[test]
        fn test_search_symbols_total_count() {
            let (_db_file, _conn) = create_test_db();
            let db_path = _db_file.path();

            let options = SearchOptions {
                db_path,
                query: "test",
                path_filter: None,
                kind_filter: None,
                limit: 10,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: false,
                sort_by: SortMode::default(),
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (response, partial) = search_symbols(options).unwrap();
            assert!(!partial, "Should not be partial");
            assert_eq!(response.total_count, 2, "Total count should be 2");
        }

        #[test]
        fn test_search_symbols_ordering() {
            let (_db_file, _conn) = create_test_db();
            let db_path = _db_file.path();

            let options = SearchOptions {
                db_path,
                query: "test",
                path_filter: None,
                kind_filter: None,
                limit: 10,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: true,
                sort_by: SortMode::default(),
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (response, partial) = search_symbols(options).unwrap();
            assert!(!partial, "Should not be partial");
            assert_eq!(response.results.len(), 2, "Should find 2 results");

            assert_eq!(
                response.results[0].name, "test_func",
                "test_func should be first (higher score)"
            );
            assert_eq!(
                response.results[0].score,
                Some(80),
                "test_func should have prefix score 80"
            );
            assert_eq!(
                response.results[1].name, "TestStruct",
                "TestStruct should be second"
            );
            assert_eq!(
                response.results[1].score,
                Some(0),
                "TestStruct should have score 0"
            );
        }

        #[test]
        fn test_search_symbols_include_score() {
            let (_db_file, _conn) = create_test_db();
            let db_path = _db_file.path();

            let options = SearchOptions {
                db_path,
                query: "test_func",
                path_filter: None,
                kind_filter: None,
                limit: 10,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: true,
                sort_by: SortMode::default(),
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (response, partial) = search_symbols(options).unwrap();
            assert!(!partial, "Should not be partial");
            assert_eq!(response.results.len(), 1, "Should find 1 result");
            assert!(
                response.results[0].score.is_some(),
                "Score should be included"
            );
            assert_eq!(response.results[0].score, Some(100), "Score should be 100");
        }

        #[test]
        fn test_search_symbols_with_fqn() {
            let (_db_file, _conn) = create_test_db();
            let db_path = _db_file.path();

            let options = SearchOptions {
                db_path,
                query: "test_func",
                path_filter: None,
                kind_filter: None,
                limit: 10,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions {
                    fqn: true,
                    canonical_fqn: false,
                    display_fqn: false,
                },
                include_score: false,
                sort_by: SortMode::default(),
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (response, partial) = search_symbols(options).unwrap();
            assert!(!partial, "Should not be partial");
            assert_eq!(response.results.len(), 1, "Should find 1 result");
            assert_eq!(
                response.results[0].fqn,
                Some("module::test_func".to_string()),
                "FQN should be included"
            );
            assert!(
                response.results[0].display_fqn.is_none(),
                "display_fqn should not be included"
            );
        }
    }
    // Public API tests for search_calls()
    mod pub_api_tests {
        use super::*;
        use rusqlite::Connection;
        use tempfile::NamedTempFile;

        fn create_test_db_with_calls() -> (NamedTempFile, Connection) {
            let db_file = NamedTempFile::new().unwrap();
            let conn = Connection::open(db_file.path()).unwrap();

            // Create schema
            conn.execute(
                "CREATE TABLE graph_entities (
                    id INTEGER PRIMARY KEY,
                    kind TEXT NOT NULL,
                    data TEXT NOT NULL
                )",
                [],
            )
            .unwrap();

            // Insert test Call entities
            conn.execute(
                "INSERT INTO graph_entities (id, kind, data) VALUES
                    (10, 'Call', '{\"file\":\"/test/file.rs\",\"caller\":\"main\",\"callee\":\"test_func\",\"caller_symbol_id\":\"sym1\",\"callee_symbol_id\":\"sym2\",\"byte_start\":50,\"byte_end\":70,\"start_line\":5,\"start_col\":4,\"end_line\":5,\"end_col\":24}'),
                    (11, 'Call', '{\"file\":\"/test/file.rs\",\"caller\":\"main\",\"callee\":\"helper\",\"caller_symbol_id\":\"sym1\",\"callee_symbol_id\":\"sym3\",\"byte_start\":100,\"byte_end\":115,\"start_line\":10,\"start_col\":4,\"end_line\":10,\"end_col\":19}'),
                    (12, 'Call', '{\"file\":\"/test/other.rs\",\"caller\":\"process\",\"callee\":\"test_func\",\"caller_symbol_id\":\"sym4\",\"callee_symbol_id\":\"sym2\",\"byte_start\":200,\"byte_end\":220,\"start_line\":20,\"start_col\":0,\"end_line\":20,\"end_col\":20}')",
                [],
            ).unwrap();

            (db_file, conn)
        }

        #[test]
        fn test_search_calls_basic() {
            let (_db_file, _conn) = create_test_db_with_calls();

            let options = SearchOptions {
                db_path: _db_file.path(),
                query: "test_func",
                path_filter: None,
                kind_filter: None,
                limit: 10,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: true,
                sort_by: SortMode::default(),
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (response, _partial) = search_calls(options).unwrap();

            // Should find 2 calls where callee is "test_func"
            assert_eq!(response.results.len(), 2);
            assert_eq!(response.total_count, 2);
            assert_eq!(response.query, "test_func");

            // Both results should have callee "test_func"
            for result in &response.results {
                assert_eq!(result.callee, "test_func");
            }
        }

        #[test]
        fn test_search_calls_caller_match() {
            let (_db_file, _conn) = create_test_db_with_calls();

            let options = SearchOptions {
                db_path: _db_file.path(),
                query: "main",
                path_filter: None,
                kind_filter: None,
                limit: 10,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: true,
                sort_by: SortMode::default(),
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (response, _partial) = search_calls(options).unwrap();

            // Should find 2 calls where caller is "main"
            assert_eq!(response.results.len(), 2);
            assert_eq!(response.total_count, 2);

            // Both results should have caller "main"
            for result in &response.results {
                assert_eq!(result.caller, "main");
            }
        }

        #[test]
        fn test_search_calls_empty_results() {
            let (_db_file, _conn) = create_test_db_with_calls();

            let options = SearchOptions {
                db_path: _db_file.path(),
                query: "nonexistent",
                path_filter: None,
                kind_filter: None,
                limit: 10,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: true,
                sort_by: SortMode::default(),
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (response, _partial) = search_calls(options).unwrap();

            // Should find 0 results
            assert_eq!(response.results.len(), 0);
            assert_eq!(response.total_count, 0);
        }

        #[test]
        fn test_search_calls_regex_mode() {
            let (_db_file, _conn) = create_test_db_with_calls();

            let options = SearchOptions {
                db_path: _db_file.path(),
                query: "test.*",
                path_filter: None,
                kind_filter: None,
                limit: 10,
                use_regex: true,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: true,
                sort_by: SortMode::default(),
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (response, _partial) = search_calls(options).unwrap();

            // Should find 2 calls matching "test.*" pattern (callee is "test_func")
            assert_eq!(response.results.len(), 2);
            assert_eq!(response.total_count, 2);

            // Both should match because callee is "test_func" which matches "test.*"
            for result in &response.results {
                assert_eq!(result.callee, "test_func");
            }
        }

        #[test]
        fn test_search_calls_regex_no_match() {
            let (_db_file, _conn) = create_test_db_with_calls();

            let options = SearchOptions {
                db_path: _db_file.path(),
                query: "xyz.*",
                path_filter: None,
                kind_filter: None,
                limit: 10,
                use_regex: true,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: true,
                sort_by: SortMode::default(),
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (response, _partial) = search_calls(options).unwrap();

            // Should find 0 results - nothing matches "xyz.*"
            assert_eq!(response.results.len(), 0);
            assert_eq!(response.total_count, 0);
        }

        #[test]
        fn test_search_calls_score_callee_match() {
            let (_db_file, _conn) = create_test_db_with_calls();

            let options = SearchOptions {
                db_path: _db_file.path(),
                query: "test_func",
                path_filter: None,
                kind_filter: None,
                limit: 10,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: true,
                sort_by: SortMode::default(),
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (response, _partial) = search_calls(options).unwrap();

            // Should find results with scores
            assert!(!response.results.is_empty());

            // All results should have scores
            for result in &response.results {
                assert!(result.score.is_some());
                // Exact match on callee should give score 100
                assert_eq!(result.score.unwrap(), 100);
            }
        }

        #[test]
        fn test_search_calls_score_caller_match() {
            let (_db_file, _conn) = create_test_db_with_calls();

            let options = SearchOptions {
                db_path: _db_file.path(),
                query: "main",
                path_filter: None,
                kind_filter: None,
                limit: 10,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: true,
                sort_by: SortMode::default(),
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (response, _partial) = search_calls(options).unwrap();

            // Should find results with scores
            assert!(!response.results.is_empty());

            // All results should have scores
            for result in &response.results {
                assert!(result.score.is_some());
                // Exact match on caller should give score 100
                assert_eq!(result.score.unwrap(), 100);
            }
        }

        #[test]
        fn test_search_calls_limit() {
            let (_db_file, _conn) = create_test_db_with_calls();

            let options = SearchOptions {
                db_path: _db_file.path(),
                query: "test_func",
                path_filter: None,
                kind_filter: None,
                limit: 1,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: true,
                sort_by: SortMode::default(),
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (response, _partial) = search_calls(options).unwrap();

            // Should return only 1 result due to limit
            assert_eq!(response.results.len(), 1);
            // But total_count should reflect all matches
            assert_eq!(response.total_count, 2);
        }

        #[test]
        fn test_search_calls_total_count() {
            let (_db_file, _conn) = create_test_db_with_calls();

            let options = SearchOptions {
                db_path: _db_file.path(),
                query: "test_func",
                path_filter: None,
                kind_filter: None,
                limit: 10,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: true,
                sort_by: SortMode::default(),
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (response, _partial) = search_calls(options).unwrap();

            // total_count should accurately reflect all matching results
            assert_eq!(response.total_count, 2);
        }

        #[test]
        fn test_search_calls_path_filter() {
            let (_db_file, _conn) = create_test_db_with_calls();

            let path = PathBuf::from("/test/file.rs");
            let options = SearchOptions {
                db_path: _db_file.path(),
                query: "test_func",
                path_filter: Some(&path),
                kind_filter: None,
                limit: 10,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: true,
                sort_by: SortMode::default(),
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (response, _partial) = search_calls(options).unwrap();

            // Should only find calls in /test/file.rs
            assert_eq!(response.results.len(), 1);
            assert_eq!(response.total_count, 1);

            // Result should be from the filtered path
            assert_eq!(response.results[0].span.file_path, "/test/file.rs");
        }

        #[test]
        fn test_search_calls_include_score() {
            let (_db_file, _conn) = create_test_db_with_calls();

            let options = SearchOptions {
                db_path: _db_file.path(),
                query: "test_func",
                path_filter: None,
                kind_filter: None,
                limit: 10,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: true,
                sort_by: SortMode::default(),
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (response, _partial) = search_calls(options).unwrap();

            // All results should include scores when include_score is true
            for result in &response.results {
                assert!(result.score.is_some());
                assert!(result.score.unwrap() > 0);
            }
        }

        #[test]
        fn test_search_calls_ordering() {
            let (_db_file, _conn) = create_test_db_with_calls();

            let options = SearchOptions {
                db_path: _db_file.path(),
                query: "test_func",
                path_filter: None,
                kind_filter: None,
                limit: 10,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: true,
                sort_by: SortMode::default(),
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (response, _partial) = search_calls(options).unwrap();

            // Results should be sorted by score (all same here), then by start_line
            if response.results.len() > 1 {
                for i in 1..response.results.len() {
                    let prev = &response.results[i - 1];
                    let curr = &response.results[i];
                    // Scores should be non-increasing
                    assert!(prev.score.unwrap() >= curr.score.unwrap());
                    // Within same score, sorted by start_line
                    if prev.score == curr.score {
                        assert!(prev.span.start_line <= curr.span.start_line);
                    }
                }
            }
        }

        // Helper function to create a test database with reference data
        fn create_test_db_with_references() -> (NamedTempFile, Connection) {
            let db_file = NamedTempFile::new().unwrap();
            let conn = Connection::open(db_file.path()).unwrap();

            // Create schema
            conn.execute(
                "CREATE TABLE graph_entities (
                    id INTEGER PRIMARY KEY,
                    kind TEXT NOT NULL,
                    data TEXT NOT NULL,
                    name TEXT
                )",
                [],
            )
            .unwrap();
            conn.execute(
                "CREATE TABLE graph_edges (
                    id INTEGER PRIMARY KEY,
                    from_id INTEGER NOT NULL,
                    to_id INTEGER NOT NULL,
                    edge_type TEXT NOT NULL
                )",
                [],
            )
            .unwrap();

            // Insert test Symbol entity
            let symbol_data = json!({
                "symbol_id": "sym1",
                "name": "test_func",
                "kind": "Function",
                "kind_normalized": "function"
            })
            .to_string();
            conn.execute(
                "INSERT INTO graph_entities (id, kind, data) VALUES (1, 'Symbol', ?1)",
                [symbol_data],
            )
            .unwrap();

            // Insert test Reference entities
            let ref1_data = json!({
                "file": "/test/file.rs",
                "byte_start": 50,
                "byte_end": 60,
                "start_line": 3,
                "start_col": 5,
                "end_line": 3,
                "end_col": 14
            })
            .to_string();
            conn.execute(
                "INSERT INTO graph_entities (id, kind, name, data) VALUES
                    (10, 'Reference', 'ref to test_func', ?1)",
                [ref1_data],
            )
            .unwrap();

            let ref2_data = json!({
                "file": "/test/file.rs",
                "byte_start": 100,
                "byte_end": 112,
                "start_line": 7,
                "start_col": 0,
                "end_line": 7,
                "end_col": 12
            })
            .to_string();
            conn.execute(
                "INSERT INTO graph_entities (id, kind, name, data) VALUES
                    (11, 'Reference', 'ref to TestStruct', ?1)",
                [ref2_data],
            )
            .unwrap();

            let ref3_data = json!({
                "file": "/test/other.rs",
                "byte_start": 200,
                "byte_end": 210,
                "start_line": 10,
                "start_col": 0,
                "end_line": 10,
                "end_col": 10
            })
            .to_string();
            conn.execute(
                "INSERT INTO graph_entities (id, kind, name, data) VALUES
                    (12, 'Reference', 'ref to helper', ?1)",
                [ref3_data],
            )
            .unwrap();

            // Insert REFERENCES edge
            conn.execute(
                "INSERT INTO graph_edges (from_id, to_id, edge_type) VALUES (10, 1, 'REFERENCES')",
                [],
            )
            .unwrap();

            (db_file, conn)
        }

        #[test]
        fn test_search_references_basic() {
            let (db_file, _conn) = create_test_db_with_references();

            let options = SearchOptions {
                db_path: db_file.path(),
                query: "test_func",
                path_filter: None,
                kind_filter: None,
                limit: 100,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: false,
                sort_by: SortMode::default(),
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (result, _partial) = search_references(options).unwrap();
            assert_eq!(
                result.results.len(),
                1,
                "Should find 1 reference to test_func"
            );
            assert_eq!(result.results[0].referenced_symbol, "test_func");
            assert_eq!(result.query, "test_func");
        }

        #[test]
        fn test_search_references_empty_results() {
            let (db_file, _conn) = create_test_db_with_references();

            let options = SearchOptions {
                db_path: db_file.path(),
                query: "nonexistent",
                path_filter: None,
                kind_filter: None,
                limit: 100,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: false,
                sort_by: SortMode::default(),
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (result, _partial) = search_references(options).unwrap();
            assert_eq!(
                result.results.len(),
                0,
                "Should find 0 references for nonexistent symbol"
            );
        }

        #[test]
        fn test_search_references_prefix_match() {
            let (db_file, _conn) = create_test_db_with_references();

            let options = SearchOptions {
                db_path: db_file.path(),
                query: "test",
                path_filter: None,
                kind_filter: None,
                limit: 100,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: false,
                sort_by: SortMode::default(),
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (result, _partial) = search_references(options).unwrap();
            assert_eq!(
                result.results.len(),
                1,
                "Should find 1 reference with 'test' prefix"
            );
            assert_eq!(result.results[0].referenced_symbol, "test_func");
        }

        #[test]
        fn test_search_references_regex_mode() {
            let (db_file, _conn) = create_test_db_with_references();

            let options = SearchOptions {
                db_path: db_file.path(),
                query: "test.*",
                path_filter: None,
                kind_filter: None,
                limit: 100,
                use_regex: true,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: false,
                sort_by: SortMode::default(),
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (result, _partial) = search_references(options).unwrap();
            assert_eq!(
                result.results.len(),
                1,
                "Should find 1 reference matching regex 'test.*'"
            );
            assert_eq!(result.results[0].referenced_symbol, "test_func");
        }

        #[test]
        fn test_search_references_regex_no_match() {
            let (db_file, _conn) = create_test_db_with_references();

            let options = SearchOptions {
                db_path: db_file.path(),
                query: "xyz.*",
                path_filter: None,
                kind_filter: None,
                limit: 100,
                use_regex: true,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: false,
                sort_by: SortMode::default(),
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (result, _partial) = search_references(options).unwrap();
            assert_eq!(
                result.results.len(),
                0,
                "Should find 0 references matching regex 'xyz.*'"
            );
        }

        #[test]
        fn test_search_references_score_exact_match() {
            let (db_file, _conn) = create_test_db_with_references();

            let options = SearchOptions {
                db_path: db_file.path(),
                query: "test_func",
                path_filter: None,
                kind_filter: None,
                limit: 100,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: true,
                sort_by: SortMode::default(),
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (result, _partial) = search_references(options).unwrap();
            assert_eq!(result.results.len(), 1);
            assert_eq!(
                result.results[0].score,
                Some(100),
                "Exact match should have score 100"
            );
        }

        #[test]
        fn test_search_references_limit() {
            let (db_file, _conn) = create_test_db_with_references();

            let options = SearchOptions {
                db_path: db_file.path(),
                query: "test",
                path_filter: None,
                kind_filter: None,
                limit: 1,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: false,
                sort_by: SortMode::default(),
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (result, _partial) = search_references(options).unwrap();
            assert_eq!(
                result.results.len(),
                1,
                "Limit should restrict results to 1"
            );
        }

        #[test]
        fn test_search_references_total_count() {
            let (db_file, _conn) = create_test_db_with_references();

            let options = SearchOptions {
                db_path: db_file.path(),
                query: "test_func",
                path_filter: None,
                kind_filter: None,
                limit: 100,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: false,
                sort_by: SortMode::default(),
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (result, _partial) = search_references(options).unwrap();
            assert_eq!(result.total_count, 1, "Total count should be 1");
        }

        #[test]
        fn test_search_references_path_filter() {
            let (db_file, _conn) = create_test_db_with_references();

            let path_filter = PathBuf::from("/test/file.rs");
            let options = SearchOptions {
                db_path: db_file.path(),
                query: "test_func",
                path_filter: Some(&path_filter),
                kind_filter: None,
                limit: 100,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: false,
                sort_by: SortMode::default(),
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (result, _partial) = search_references(options).unwrap();
            assert_eq!(
                result.results.len(),
                1,
                "Should find 1 reference in /test/file.rs"
            );
            assert_eq!(result.results[0].span.file_path, "/test/file.rs");
        }

        #[test]
        fn test_search_references_include_score() {
            let (db_file, _conn) = create_test_db_with_references();

            let options = SearchOptions {
                db_path: db_file.path(),
                query: "test_func",
                path_filter: None,
                kind_filter: None,
                limit: 100,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: true,
                sort_by: SortMode::default(),
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (result, _partial) = search_references(options).unwrap();
            assert_eq!(result.results.len(), 1);
            assert!(
                result.results[0].score.is_some(),
                "Score should be included when include_score=true"
            );
        }

        #[test]
        fn test_search_references_ordering() {
            let (db_file, _conn) = create_test_db_with_references();

            let options = SearchOptions {
                db_path: db_file.path(),
                query: "test",
                path_filter: None,
                kind_filter: None,
                limit: 100,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: true,
                sort_by: SortMode::default(),
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (result, _partial) = search_references(options).unwrap();
            // Verify that results are sorted by score (descending)
            for i in 1..result.results.len() {
                let prev_score = result.results[i - 1].score.unwrap_or(0);
                let curr_score = result.results[i].score.unwrap_or(0);
                assert!(
                    prev_score >= curr_score,
                    "Results should be sorted by score descending"
                );
            }
        }
    }

    #[test]
    fn test_load_file_returns_none_on_missing_file() {
        let mut cache = HashMap::new();
        let result = load_file("/nonexistent/path/to/file.rs", &mut cache);
        assert!(result.is_none());
        assert!(!cache.contains_key("/nonexistent/path/to/file.rs"));
    }

    #[test]
    fn test_load_file_caches_successful_reads() {
        use std::io::Write;
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("llmgrep_test_load_file.txt");
        let mut file = std::fs::File::create(&temp_file).unwrap();
        file.write_all(b"line1\nline2\nline3").unwrap();

        let mut cache = HashMap::new();
        let path_str = temp_file.to_str().unwrap();

        let result1 = load_file(path_str, &mut cache);
        assert!(result1.is_some());
        assert_eq!(result1.unwrap().lines.len(), 3);

        let result2 = load_file(path_str, &mut cache);
        assert!(result2.is_some());
        assert_eq!(cache.len(), 1);

        std::fs::remove_file(&temp_file).ok();
    }

    #[test]
    fn test_search_symbols_corrupted_database() {
        use std::io::Write;
        let temp_dir = std::env::temp_dir();
        let fake_db = temp_dir.join("llmgrep_test_corrupt.db");
        {
            let mut file = std::fs::File::create(&fake_db).unwrap();
            file.write_all(b"This is not a SQLite database").unwrap();
        }

        let result = search_symbols(SearchOptions {
            db_path: &fake_db,
            query: "test",
            path_filter: None,
            kind_filter: None,
            limit: 10,
            use_regex: false,
            candidates: 50,
            context: ContextOptions::default(),
            snippet: SnippetOptions::default(),
            fqn: FqnOptions::default(),
            include_score: true,
            sort_by: SortMode::default(),
            metrics: MetricsOptions::default(),
            ast: AstOptions::default(),
            depth: DepthOptions::default(),
            algorithm: AlgorithmOptions::default(),
            symbol_id: None,
            fqn_pattern: None,
            exact_fqn: None,
            language_filter: None,
        });

        match result {
            Err(LlmError::DatabaseCorrupted { .. }) => {}
            Err(other) => panic!("Expected DatabaseCorrupted error, got: {:?}", other),
            Ok(_) => panic!("Expected error for corrupted database"),
        }

        std::fs::remove_file(&fake_db).ok();
    }

    // Chunk retrieval tests
    mod chunk_tests {
        use super::*;
        use rusqlite::Connection;
        use tempfile::NamedTempFile;

        /// Create a test database with code_chunks table for chunk tests
        fn create_test_db_with_chunks() -> (NamedTempFile, Connection) {
            let db_file = NamedTempFile::new().unwrap();
            let conn = Connection::open(db_file.path()).unwrap();

            // Create code_chunks table
            conn.execute(
                "CREATE TABLE code_chunks (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    file_path TEXT NOT NULL,
                    byte_start INTEGER NOT NULL,
                    byte_end INTEGER NOT NULL,
                    content TEXT NOT NULL,
                    content_hash TEXT NOT NULL,
                    symbol_name TEXT,
                    symbol_kind TEXT,
                    created_at INTEGER NOT NULL
                )",
                [],
            )
            .unwrap();

            // Insert test chunks
            // SHA-256 hash of "fn test_func() { }"
            let hash1 = "a0d2da8d1f8b1a8c8f8b1a8c8f8b1a8c8f8b1a8c8f8b1a8c8f8b1a8c8f8b1a8c";
            conn.execute(
                "INSERT INTO code_chunks (file_path, byte_start, byte_end, content, content_hash, symbol_name, symbol_kind, created_at) VALUES
                    ('/test/file.rs', 100, 200, 'fn test_func() { }', ?, 'test_func', 'Function', 1700000000),
                    ('/test/file.rs', 300, 400, 'struct TestStruct { }', 'b1e3eb9e2f9c2b9d9f9c2b9d9f9c2b9d9f9c2b9d9f9c2b9d9f9c2b9d9f9c2b9d', 'TestStruct', 'Struct', 1700000001),
                    ('/test/other.rs', 500, 600, 'fn helper() { }', 'c2f4fc0f3g0d3c0e0g0d3c0e0g0d3c0e0g0d3c0e0g0d3c0e0g0d3c0e0g0d3c0e', 'helper', 'Function', 1700000002)",
                [hash1],
            ).unwrap();

            (db_file, conn)
        }

        #[test]
        fn test_search_chunks_by_symbol_name() {
            let (_db_file, conn) = create_test_db_with_chunks();

            // Query for test_func symbol
            let chunks = search_chunks_by_symbol_name(&conn, "test_func").unwrap();
            assert_eq!(chunks.len(), 1, "Should find 1 chunk for test_func");

            let chunk = &chunks[0];
            assert_eq!(chunk.file_path, "/test/file.rs");
            assert_eq!(chunk.byte_start, 100);
            assert_eq!(chunk.byte_end, 200);
            assert_eq!(chunk.content, "fn test_func() { }");
            assert_eq!(chunk.symbol_name, Some("test_func".to_string()));
            assert_eq!(chunk.symbol_kind, Some("Function".to_string()));
        }

        #[test]
        fn test_search_chunks_by_symbol_name_not_found() {
            let (_db_file, conn) = create_test_db_with_chunks();

            // Query for non-existent symbol
            let chunks = search_chunks_by_symbol_name(&conn, "nonexistent").unwrap();
            assert_eq!(
                chunks.len(),
                0,
                "Should find 0 chunks for non-existent symbol"
            );
        }

        #[test]
        fn test_search_chunks_by_span() {
            let (_db_file, conn) = create_test_db_with_chunks();

            // Query for exact span
            let chunk = search_chunks_by_span(&conn, "/test/file.rs", 100, 200).unwrap();
            assert!(chunk.is_some(), "Should find chunk for exact span");

            let chunk = chunk.unwrap();
            assert_eq!(chunk.file_path, "/test/file.rs");
            assert_eq!(chunk.byte_start, 100);
            assert_eq!(chunk.byte_end, 200);
            assert_eq!(chunk.content, "fn test_func() { }");
            assert_eq!(chunk.symbol_name, Some("test_func".to_string()));
            assert_eq!(chunk.symbol_kind, Some("Function".to_string()));
        }

        #[test]
        fn test_search_chunks_by_span_not_found() {
            let (_db_file, conn) = create_test_db_with_chunks();

            // Query for non-existent span
            let chunk = search_chunks_by_span(&conn, "/test/file.rs", 999, 1000).unwrap();
            assert!(chunk.is_none(), "Should return None for non-existent span");

            // Query for non-existent file
            let chunk = search_chunks_by_span(&conn, "/test/nonexistent.rs", 100, 200).unwrap();
            assert!(chunk.is_none(), "Should return None for non-existent file");
        }

        #[test]
        fn test_search_chunks_by_span_wrong_byte_range() {
            let (_db_file, conn) = create_test_db_with_chunks();

            // Query with wrong byte_start
            let chunk = search_chunks_by_span(&conn, "/test/file.rs", 101, 200).unwrap();
            assert!(
                chunk.is_none(),
                "Should return None when byte_start doesn't match"
            );

            // Query with wrong byte_end
            let chunk = search_chunks_by_span(&conn, "/test/file.rs", 100, 201).unwrap();
            assert!(
                chunk.is_none(),
                "Should return None when byte_end doesn't match"
            );
        }

        #[test]
        fn test_content_hash_format() {
            let (_db_file, conn) = create_test_db_with_chunks();

            let chunks = search_chunks_by_symbol_name(&conn, "test_func").unwrap();
            assert_eq!(chunks.len(), 1);

            let hash = &chunks[0].content_hash;
            assert_eq!(hash.len(), 64, "SHA-256 hash should be 64 hex characters");
            assert!(
                hash.chars().all(|c| c.is_ascii_hexdigit()),
                "Hash should contain only hex characters"
            );
        }

        #[test]
        fn test_symbol_kind_retrieval() {
            let (_db_file, conn) = create_test_db_with_chunks();

            // Test Function kind
            let chunks = search_chunks_by_symbol_name(&conn, "test_func").unwrap();
            assert_eq!(chunks[0].symbol_kind, Some("Function".to_string()));

            // Test Struct kind
            let chunks = search_chunks_by_symbol_name(&conn, "TestStruct").unwrap();
            assert_eq!(chunks[0].symbol_kind, Some("Struct".to_string()));
        }

        #[test]
        fn test_multiple_chunks_same_symbol() {
            let db_file = NamedTempFile::new().unwrap();
            let conn = Connection::open(db_file.path()).unwrap();

            // Create code_chunks table
            conn.execute(
                "CREATE TABLE code_chunks (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    file_path TEXT NOT NULL,
                    byte_start INTEGER NOT NULL,
                    byte_end INTEGER NOT NULL,
                    content TEXT NOT NULL,
                    content_hash TEXT NOT NULL,
                    symbol_name TEXT,
                    symbol_kind TEXT,
                    created_at INTEGER NOT NULL
                )",
                [],
            )
            .unwrap();

            // Insert multiple chunks for the same symbol (e.g., different parts)
            conn.execute(
                "INSERT INTO code_chunks (file_path, byte_start, byte_end, content, content_hash, symbol_name, symbol_kind, created_at) VALUES
                    ('/test/file.rs', 100, 150, 'part1', 'hash1', 'my_symbol', 'Function', 1700000000),
                    ('/test/file.rs', 150, 200, 'part2', 'hash2', 'my_symbol', 'Function', 1700000001)",
                [],
            ).unwrap();

            // Query should return all chunks for the symbol
            let chunks = search_chunks_by_symbol_name(&conn, "my_symbol").unwrap();
            assert_eq!(chunks.len(), 2, "Should find 2 chunks for my_symbol");
        }
    }

    // Metrics filtering and sorting tests
    mod metrics_tests {
        use super::*;

        fn create_test_db_with_metrics() -> (tempfile::NamedTempFile, Connection) {
            let db_file = tempfile::NamedTempFile::new().unwrap();
            let conn = Connection::open(db_file.path()).unwrap();

            // Create schema
            conn.execute(
                "CREATE TABLE graph_entities (
                    id INTEGER PRIMARY KEY,
                    kind TEXT NOT NULL,
                    data TEXT NOT NULL
                )",
                [],
            )
            .unwrap();
            conn.execute(
                "CREATE TABLE graph_edges (
                    id INTEGER PRIMARY KEY,
                    from_id INTEGER NOT NULL,
                    to_id INTEGER NOT NULL,
                    edge_type TEXT NOT NULL
                )",
                [],
            )
            .unwrap();
            conn.execute(
                "CREATE TABLE symbol_metrics (
                    symbol_id INTEGER PRIMARY KEY,
                    symbol_name TEXT NOT NULL,
                    kind TEXT NOT NULL,
                    file_path TEXT NOT NULL,
                    loc INTEGER NOT NULL DEFAULT 0,
                    estimated_loc REAL NOT NULL DEFAULT 0.0,
                    fan_in INTEGER NOT NULL DEFAULT 0,
                    fan_out INTEGER NOT NULL DEFAULT 0,
                    cyclomatic_complexity INTEGER NOT NULL DEFAULT 1,
                    last_updated INTEGER NOT NULL DEFAULT 0,
                    FOREIGN KEY (symbol_id) REFERENCES graph_entities(id) ON DELETE CASCADE
                )",
                [],
            )
            .unwrap();

            // Insert test File entity
            conn.execute(
                "INSERT INTO graph_entities (id, kind, data) VALUES (1, 'File', '{\"path\":\"/test/file.rs\"}')",
                [],
            ).unwrap();

            // Insert test Symbol entities with varying metrics
            // sym1: complexity=5, fan_in=10, fan_out=2
            // sym2: complexity=15, fan_in=5, fan_out=8
            // sym3: complexity=25, fan_in=2, fan_out=15
            conn.execute(
                "INSERT INTO graph_entities (id, kind, data) VALUES
                    (10, 'Symbol', '{\"name\":\"low_complexity\",\"kind\":\"Function\",\"kind_normalized\":\"function\",\"display_fqn\":\"low_complexity\",\"fqn\":\"module::low_complexity\",\"symbol_id\":\"sym1\",\"byte_start\":100,\"byte_end\":200,\"start_line\":5,\"start_col\":0,\"end_line\":10,\"end_col\":1}'),
                    (11, 'Symbol', '{\"name\":\"med_complexity\",\"kind\":\"Function\",\"kind_normalized\":\"function\",\"display_fqn\":\"med_complexity\",\"fqn\":\"module::med_complexity\",\"symbol_id\":\"sym2\",\"byte_start\":300,\"byte_end\":400,\"start_line\":15,\"start_col\":0,\"end_line\":20,\"end_col\":1}'),
                    (12, 'Symbol', '{\"name\":\"high_complexity\",\"kind\":\"Function\",\"kind_normalized\":\"function\",\"display_fqn\":\"high_complexity\",\"fqn\":\"module::high_complexity\",\"symbol_id\":\"sym3\",\"byte_start\":500,\"byte_end\":600,\"start_line\":25,\"start_col\":0,\"end_line\":30,\"end_col\":1}')",
                [],
            ).unwrap();

            // Insert DEFINES edges from File to Symbols
            conn.execute(
                "INSERT INTO graph_edges (from_id, to_id, edge_type) VALUES (1, 10, 'DEFINES'), (1, 11, 'DEFINES'), (1, 12, 'DEFINES')",
                [],
            ).unwrap();

            // Insert metrics - symbol_id now references graph_entities.id (INTEGER)
            conn.execute(
                "INSERT INTO symbol_metrics (symbol_id, symbol_name, kind, file_path, loc, estimated_loc, fan_in, fan_out, cyclomatic_complexity, last_updated) VALUES
                    (10, 'low_complexity', 'Function', '/test/file.rs', 50, 0.0, 10, 2, 5, 0),
                    (11, 'med_complexity', 'Function', '/test/file.rs', 100, 0.0, 5, 8, 15, 0),
                    (12, 'high_complexity', 'Function', '/test/file.rs', 150, 0.0, 2, 15, 25, 0)",
                [],
            ).unwrap();

            (db_file, conn)
        }

        #[test]
        fn test_metrics_filter_by_min_complexity() {
            let (_db_file, _conn) = create_test_db_with_metrics();
            let db_path = _db_file.path();

            let options = SearchOptions {
                db_path,
                query: "complexity",
                path_filter: None,
                kind_filter: None,
                limit: 10,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: false,
                sort_by: SortMode::default(),
                metrics: MetricsOptions {
                    min_complexity: Some(10),
                    max_complexity: None,
                    min_fan_in: None,
                    min_fan_out: None,
                },
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (response, partial) = search_symbols(options).unwrap();
            assert!(!partial, "Should not be partial");
            // Should find med_complexity (15) and high_complexity (25), but not low_complexity (5)
            assert_eq!(
                response.results.len(),
                2,
                "Should find 2 results with complexity >= 10"
            );

            let names: Vec<&str> = response.results.iter().map(|r| r.name.as_str()).collect();
            assert!(
                names.contains(&"med_complexity"),
                "Should contain med_complexity"
            );
            assert!(
                names.contains(&"high_complexity"),
                "Should contain high_complexity"
            );
            assert!(
                !names.contains(&"low_complexity"),
                "Should not contain low_complexity"
            );
        }

        #[test]
        fn test_metrics_filter_by_max_complexity() {
            let (_db_file, _conn) = create_test_db_with_metrics();
            let db_path = _db_file.path();

            let options = SearchOptions {
                db_path,
                query: "complexity",
                path_filter: None,
                kind_filter: None,
                limit: 10,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: false,
                sort_by: SortMode::default(),
                metrics: MetricsOptions {
                    min_complexity: None,
                    max_complexity: Some(10),
                    min_fan_in: None,
                    min_fan_out: None,
                },
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (response, partial) = search_symbols(options).unwrap();
            assert!(!partial, "Should not be partial");
            // Should find only low_complexity (5), not med (15) or high (25)
            assert_eq!(
                response.results.len(),
                1,
                "Should find 1 result with complexity <= 10"
            );
            assert_eq!(response.results[0].name, "low_complexity");
        }

        #[test]
        fn test_metrics_filter_combined_min_max_complexity() {
            let (_db_file, _conn) = create_test_db_with_metrics();
            let db_path = _db_file.path();

            let options = SearchOptions {
                db_path,
                query: "complexity",
                path_filter: None,
                kind_filter: None,
                limit: 10,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: false,
                sort_by: SortMode::default(),
                metrics: MetricsOptions {
                    min_complexity: Some(10),
                    max_complexity: Some(20),
                    min_fan_in: None,
                    min_fan_out: None,
                },
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (response, partial) = search_symbols(options).unwrap();
            assert!(!partial, "Should not be partial");
            // Should find only med_complexity (15), not low (5) or high (25)
            assert_eq!(
                response.results.len(),
                1,
                "Should find 1 result with complexity in range [10, 20]"
            );
            assert_eq!(response.results[0].name, "med_complexity");
        }

        #[test]
        fn test_metrics_filter_by_min_fan_in() {
            let (_db_file, _conn) = create_test_db_with_metrics();
            let db_path = _db_file.path();

            let options = SearchOptions {
                db_path,
                query: "complexity",
                path_filter: None,
                kind_filter: None,
                limit: 10,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: false,
                sort_by: SortMode::default(),
                metrics: MetricsOptions {
                    min_complexity: None,
                    max_complexity: None,
                    min_fan_in: Some(8),
                    min_fan_out: None,
                },
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (response, partial) = search_symbols(options).unwrap();
            assert!(!partial, "Should not be partial");
            // Should find only low_complexity (fan_in=10)
            assert_eq!(
                response.results.len(),
                1,
                "Should find 1 result with fan_in >= 8"
            );
            assert_eq!(response.results[0].name, "low_complexity");
            assert_eq!(
                response.results[0].fan_in,
                Some(10),
                "fan_in should be populated"
            );
        }

        #[test]
        fn test_metrics_filter_by_min_fan_out() {
            let (_db_file, _conn) = create_test_db_with_metrics();
            let db_path = _db_file.path();

            let options = SearchOptions {
                db_path,
                query: "complexity",
                path_filter: None,
                kind_filter: None,
                limit: 10,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: false,
                sort_by: SortMode::default(),
                metrics: MetricsOptions {
                    min_complexity: None,
                    max_complexity: None,
                    min_fan_in: None,
                    min_fan_out: Some(10),
                },
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (response, partial) = search_symbols(options).unwrap();
            assert!(!partial, "Should not be partial");
            // Should find only high_complexity (fan_out=15)
            assert_eq!(
                response.results.len(),
                1,
                "Should find 1 result with fan_out >= 10"
            );
            assert_eq!(response.results[0].name, "high_complexity");
            assert_eq!(
                response.results[0].fan_out,
                Some(15),
                "fan_out should be populated"
            );
        }

        #[test]
        fn test_metrics_sort_by_fan_in() {
            let (_db_file, _conn) = create_test_db_with_metrics();
            let db_path = _db_file.path();

            let options = SearchOptions {
                db_path,
                query: "complexity",
                path_filter: None,
                kind_filter: None,
                limit: 10,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: false,
                sort_by: SortMode::FanIn,
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (response, partial) = search_symbols(options).unwrap();
            assert!(!partial, "Should not be partial");
            assert_eq!(response.results.len(), 3, "Should find all 3 results");

            // Should be sorted by fan_in DESC: low_complexity (10), med_complexity (5), high_complexity (2)
            assert_eq!(
                response.results[0].name, "low_complexity",
                "First should have highest fan_in"
            );
            assert_eq!(response.results[0].fan_in, Some(10));
            assert_eq!(
                response.results[1].name, "med_complexity",
                "Second should have medium fan_in"
            );
            assert_eq!(response.results[1].fan_in, Some(5));
            assert_eq!(
                response.results[2].name, "high_complexity",
                "Third should have lowest fan_in"
            );
            assert_eq!(response.results[2].fan_in, Some(2));
        }

        #[test]
        fn test_metrics_sort_by_fan_out() {
            let (_db_file, _conn) = create_test_db_with_metrics();
            let db_path = _db_file.path();

            let options = SearchOptions {
                db_path,
                query: "complexity",
                path_filter: None,
                kind_filter: None,
                limit: 10,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: false,
                sort_by: SortMode::FanOut,
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (response, partial) = search_symbols(options).unwrap();
            assert!(!partial, "Should not be partial");
            assert_eq!(response.results.len(), 3, "Should find all 3 results");

            // Should be sorted by fan_out DESC: high_complexity (15), med_complexity (8), low_complexity (2)
            assert_eq!(
                response.results[0].name, "high_complexity",
                "First should have highest fan_out"
            );
            assert_eq!(response.results[0].fan_out, Some(15));
            assert_eq!(
                response.results[1].name, "med_complexity",
                "Second should have medium fan_out"
            );
            assert_eq!(response.results[1].fan_out, Some(8));
            assert_eq!(
                response.results[2].name, "low_complexity",
                "Third should have lowest fan_out"
            );
            assert_eq!(response.results[2].fan_out, Some(2));
        }

        #[test]
        fn test_metrics_sort_by_complexity() {
            let (_db_file, _conn) = create_test_db_with_metrics();
            let db_path = _db_file.path();

            let options = SearchOptions {
                db_path,
                query: "complexity",
                path_filter: None,
                kind_filter: None,
                limit: 10,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: false,
                sort_by: SortMode::Complexity,
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (response, partial) = search_symbols(options).unwrap();
            assert!(!partial, "Should not be partial");
            assert_eq!(response.results.len(), 3, "Should find all 3 results");

            // Should be sorted by cyclomatic_complexity DESC: high_complexity (25), med_complexity (15), low_complexity (5)
            assert_eq!(
                response.results[0].name, "high_complexity",
                "First should have highest complexity"
            );
            assert_eq!(response.results[0].cyclomatic_complexity, Some(25));
            assert_eq!(
                response.results[1].name, "med_complexity",
                "Second should have medium complexity"
            );
            assert_eq!(response.results[1].cyclomatic_complexity, Some(15));
            assert_eq!(
                response.results[2].name, "low_complexity",
                "Third should have lowest complexity"
            );
            assert_eq!(response.results[2].cyclomatic_complexity, Some(5));
        }

        #[test]
        fn test_metrics_fields_populated() {
            let (_db_file, _conn) = create_test_db_with_metrics();
            let db_path = _db_file.path();

            let options = SearchOptions {
                db_path,
                query: "low_complexity",
                path_filter: None,
                kind_filter: None,
                limit: 10,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: false,
                sort_by: SortMode::default(),
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (response, partial) = search_symbols(options).unwrap();
            assert!(!partial, "Should not be partial");
            assert_eq!(response.results.len(), 1);

            let result = &response.results[0];
            assert_eq!(result.name, "low_complexity");
            // Verify metrics fields are populated
            assert_eq!(result.fan_in, Some(10), "fan_in should be populated");
            assert_eq!(result.fan_out, Some(2), "fan_out should be populated");
            assert_eq!(
                result.cyclomatic_complexity,
                Some(5),
                "cyclomatic_complexity should be populated"
            );
            assert_eq!(
                result.complexity_score, None,
                "complexity_score is not available in symbol_metrics"
            );
        }

        #[test]
        fn test_metrics_null_handling() {
            // Create a DB where some symbols have metrics and some don't
            let db_file = tempfile::NamedTempFile::new().unwrap();
            let conn = Connection::open(db_file.path()).unwrap();

            conn.execute(
                "CREATE TABLE graph_entities (
                    id INTEGER PRIMARY KEY,
                    kind TEXT NOT NULL,
                    data TEXT NOT NULL
                )",
                [],
            )
            .unwrap();
            conn.execute(
                "CREATE TABLE graph_edges (
                    id INTEGER PRIMARY KEY,
                    from_id INTEGER NOT NULL,
                    to_id INTEGER NOT NULL,
                    edge_type TEXT NOT NULL
                )",
                [],
            )
            .unwrap();
            conn.execute(
                "CREATE TABLE symbol_metrics (
                    symbol_id INTEGER PRIMARY KEY,
                    symbol_name TEXT NOT NULL,
                    kind TEXT NOT NULL,
                    file_path TEXT NOT NULL,
                    loc INTEGER NOT NULL DEFAULT 0,
                    estimated_loc REAL NOT NULL DEFAULT 0.0,
                    fan_in INTEGER NOT NULL DEFAULT 0,
                    fan_out INTEGER NOT NULL DEFAULT 0,
                    cyclomatic_complexity INTEGER NOT NULL DEFAULT 1,
                    last_updated INTEGER NOT NULL DEFAULT 0,
                    FOREIGN KEY (symbol_id) REFERENCES graph_entities(id) ON DELETE CASCADE
                )",
                [],
            )
            .unwrap();

            conn.execute(
                "INSERT INTO graph_entities (id, kind, data) VALUES (1, 'File', '{\"path\":\"/test/file.rs\"}')",
                [],
            ).unwrap();

            // Insert 3 symbols: only sym1 has metrics
            conn.execute(
                "INSERT INTO graph_entities (id, kind, data) VALUES
                    (10, 'Symbol', '{\"name\":\"with_metrics\",\"kind\":\"Function\",\"kind_normalized\":\"function\",\"display_fqn\":\"with_metrics\",\"fqn\":\"module::with_metrics\",\"symbol_id\":\"sym1\",\"byte_start\":100,\"byte_end\":200,\"start_line\":5,\"start_col\":0,\"end_line\":10,\"end_col\":1}'),
                    (11, 'Symbol', '{\"name\":\"no_metrics_1\",\"kind\":\"Function\",\"kind_normalized\":\"function\",\"display_fqn\":\"no_metrics_1\",\"fqn\":\"module::no_metrics_1\",\"symbol_id\":\"sym2\",\"byte_start\":300,\"byte_end\":400,\"start_line\":15,\"start_col\":0,\"end_line\":20,\"end_col\":1}'),
                    (12, 'Symbol', '{\"name\":\"no_metrics_2\",\"kind\":\"Function\",\"kind_normalized\":\"function\",\"display_fqn\":\"no_metrics_2\",\"fqn\":\"module::no_metrics_2\",\"symbol_id\":\"sym3\",\"byte_start\":500,\"byte_end\":600,\"start_line\":25,\"start_col\":0,\"end_line\":30,\"end_col\":1}')",
                [],
            ).unwrap();

            conn.execute(
                "INSERT INTO graph_edges (from_id, to_id, edge_type) VALUES (1, 10, 'DEFINES'), (1, 11, 'DEFINES'), (1, 12, 'DEFINES')",
                [],
            ).unwrap();

            // Only sym1 has metrics - symbol_id now references graph_entities.id (INTEGER)
            conn.execute(
                "INSERT INTO symbol_metrics (symbol_id, symbol_name, kind, file_path, loc, estimated_loc, fan_in, fan_out, cyclomatic_complexity, last_updated) VALUES
                    (10, 'with_metrics', 'Function', '/test/file.rs', 50, 0.0, 10, 2, 5, 0)",
                [],
            ).unwrap();

            let db_path = db_file.path();

            // Test without filter: all symbols should appear
            let options = SearchOptions {
                db_path,
                query: "", // Empty query matches all
                path_filter: None,
                kind_filter: None,
                limit: 10,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: false,
                sort_by: SortMode::FanIn, // Sort by fan_in
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (response, _partial) = search_symbols(options).unwrap();
            assert_eq!(response.results.len(), 3, "Should find all 3 symbols");

            // Symbols without metrics should have None for metrics fields
            // and appear last in sorted results (COALESCE to 0)
            let with_metrics = response
                .results
                .iter()
                .find(|r| r.name == "with_metrics")
                .unwrap();
            assert_eq!(
                with_metrics.fan_in,
                Some(10),
                "Symbol with metrics should have fan_in"
            );

            let no_metrics_1 = response
                .results
                .iter()
                .find(|r| r.name == "no_metrics_1")
                .unwrap();
            assert_eq!(
                no_metrics_1.fan_in, None,
                "Symbol without metrics should have None for fan_in"
            );

            let no_metrics_2 = response
                .results
                .iter()
                .find(|r| r.name == "no_metrics_2")
                .unwrap();
            assert_eq!(
                no_metrics_2.fan_in, None,
                "Symbol without metrics should have None for fan_in"
            );

            // With filter: only symbols with metrics matching filter should appear
            let options_filter = SearchOptions {
                db_path,
                query: "",
                path_filter: None,
                kind_filter: None,
                limit: 10,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: false,
                sort_by: SortMode::default(),
                metrics: MetricsOptions {
                    min_fan_in: Some(5),
                    ..Default::default()
                },
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (response_filter, _) = search_symbols(options_filter).unwrap();
            assert_eq!(
                response_filter.results.len(),
                1,
                "Should find only 1 symbol with fan_in >= 5"
            );
            assert_eq!(response_filter.results[0].name, "with_metrics");
        }
    }

    // Tests for SymbolId lookup and ambiguity detection
    mod symbol_id_tests {
        use super::*;

        #[test]
        fn test_symbol_id_lookup_returns_single_result() {
            let (_db_file, _conn) = create_test_db();
            let db_path = _db_file.path();

            // Lookup by exact symbol_id
            let options = SearchOptions {
                db_path,
                query: "unused", // Query is ignored when symbol_id is provided
                path_filter: None,
                kind_filter: None,
                limit: 10,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: false,
                sort_by: SortMode::default(),
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: Some("sym1"),
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (response, partial) = search_symbols(options).unwrap();
            assert!(!partial, "Should not be partial");
            assert_eq!(
                response.results.len(),
                1,
                "Should find exactly 1 result by symbol_id"
            );
            assert_eq!(response.results[0].name, "test_func");
            assert_eq!(response.results[0].symbol_id.as_deref(), Some("sym1"));
        }

        #[test]
        fn test_fqn_pattern_filter() {
            let (_db_file, _conn) = create_test_db();
            let db_path = _db_file.path();

            // Filter by FQN pattern
            let options = SearchOptions {
                db_path,
                query: "test", // Query still applies
                path_filter: None,
                kind_filter: None,
                limit: 10,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions::default(),
                include_score: false,
                sort_by: SortMode::default(),
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: Some("/test/file.rs%"),
                exact_fqn: None,
                language_filter: None,
            };

            let (response, _partial) = search_symbols(options).unwrap();
            // All test symbols are in /test/file.rs
            assert!(
                !response.results.is_empty(),
                "Should find symbols matching FQN pattern"
            );
        }

        #[test]
        fn test_exact_fqn_filter() {
            let (_db_file, _conn) = create_test_db();
            let db_path = _db_file.path();

            // Filter by exact FQN
            let options = SearchOptions {
                db_path,
                query: "", // Empty query with exact_fqn should work
                path_filter: None,
                kind_filter: None,
                limit: 10,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions {
                    fqn: false,
                    canonical_fqn: true, // Enable to see canonical_fqn in results
                    display_fqn: false,
                },
                include_score: false,
                sort_by: SortMode::default(),
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: Some("/test/file.rs::test_func"),
                language_filter: None,
            };

            let (response, _partial) = search_symbols(options).unwrap();
            assert_eq!(
                response.results.len(),
                1,
                "Should find exactly 1 result by exact FQN"
            );
            assert_eq!(response.results[0].name, "test_func");
            assert_eq!(
                response.results[0].canonical_fqn.as_deref(),
                Some("/test/file.rs::test_func")
            );
        }

        #[test]
        fn test_symbol_id_included_in_json_output() {
            let (_db_file, _conn) = create_test_db();
            let db_path = _db_file.path();

            let options = SearchOptions {
                db_path,
                query: "test",
                path_filter: None,
                kind_filter: None,
                limit: 10,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions {
                    fqn: false,
                    canonical_fqn: true,
                    display_fqn: true,
                },
                include_score: false,
                sort_by: SortMode::default(),
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (response, _partial) = search_symbols(options).unwrap();
            // All test symbols have symbol_id
            for result in &response.results {
                assert!(
                    result.symbol_id.is_some(),
                    "symbol_id should be present in results"
                );
                assert!(
                    result.canonical_fqn.is_some(),
                    "canonical_fqn should be present when requested"
                );
                assert!(
                    result.display_fqn.is_some(),
                    "display_fqn should be present when requested"
                );
            }
        }

        #[test]
        fn test_ambiguity_detection_with_duplicate_names() {
            // Create a database with duplicate symbol names
            let db_file = tempfile::NamedTempFile::new().unwrap();
            let conn = Connection::open(db_file.path()).unwrap();

            // Create schema
            conn.execute(
                "CREATE TABLE graph_entities (
                    id INTEGER PRIMARY KEY,
                    kind TEXT NOT NULL,
                    data TEXT NOT NULL
                )",
                [],
            )
            .unwrap();
            conn.execute(
                "CREATE TABLE graph_edges (
                    id INTEGER PRIMARY KEY,
                    from_id INTEGER NOT NULL,
                    to_id INTEGER NOT NULL,
                    edge_type TEXT NOT NULL
                )",
                [],
            )
            .unwrap();
            conn.execute(
                "CREATE TABLE symbol_metrics (
                    symbol_id TEXT PRIMARY KEY,
                    fan_in INTEGER,
                    fan_out INTEGER,
                    cyclomatic_complexity INTEGER,
                    loc INTEGER
                )",
                [],
            )
            .unwrap();

            // Insert file
            conn.execute(
                "INSERT INTO graph_entities (id, kind, data) VALUES (1, 'File', '{\"path\":\"/test/a.rs\"}')",
                [],
            ).unwrap();
            conn.execute(
                "INSERT INTO graph_entities (id, kind, data) VALUES (2, 'File', '{\"path\":\"/test/b.rs\"}')",
                [],
            ).unwrap();

            // Insert two symbols with same name "parse" in different modules
            conn.execute(
                "INSERT INTO graph_entities (id, kind, data) VALUES
                    (10, 'Symbol', '{\"name\":\"parse\",\"kind\":\"Function\",\"display_fqn\":\"parse\",\"fqn\":\"a::parse\",\"canonical_fqn\":\"/test/a.rs::parse\",\"symbol_id\":\"parse_a\",\"byte_start\":100,\"byte_end\":200,\"start_line\":5,\"start_col\":0,\"end_line\":10,\"end_col\":1}'),
                    (11, 'Symbol', '{\"name\":\"parse\",\"kind\":\"Function\",\"display_fqn\":\"parse\",\"fqn\":\"b::parse\",\"canonical_fqn\":\"/test/b.rs::parse\",\"symbol_id\":\"parse_b\",\"byte_start\":300,\"byte_end\":400,\"start_line\":15,\"start_col\":0,\"end_line\":20,\"end_col\":1}')",
                [],
            ).unwrap();

            // Insert edges
            conn.execute(
                "INSERT INTO graph_edges (from_id, to_id, edge_type) VALUES (1, 10, 'DEFINES'), (2, 11, 'DEFINES')",
                [],
            ).unwrap();

            let db_path = db_file.path();

            // Query for "parse" - should trigger ambiguity warning
            let options = SearchOptions {
                db_path,
                query: "parse",
                path_filter: None,
                kind_filter: None,
                limit: 10,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions {
                    fqn: false,
                    canonical_fqn: true, // Enable to see canonical_fqn in results
                    display_fqn: false,
                },
                include_score: false,
                sort_by: SortMode::default(),
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            // Capture stderr to check for warning
            let (response, _partial) = search_symbols(options).unwrap();
            // Should find both symbols
            assert_eq!(
                response.results.len(),
                2,
                "Should find both 'parse' symbols"
            );
            // Both should have different canonical_fqns
            let fqns: Vec<_> = response
                .results
                .iter()
                .filter_map(|r| r.canonical_fqn.as_ref())
                .collect();
            assert_eq!(fqns.len(), 2, "Should have 2 different FQNs");
        }

        #[test]
        fn test_symbol_id_bypasses_ambiguity() {
            // Create a database with duplicate symbol names
            let db_file = tempfile::NamedTempFile::new().unwrap();
            let conn = Connection::open(db_file.path()).unwrap();

            // Create schema
            conn.execute(
                "CREATE TABLE graph_entities (
                    id INTEGER PRIMARY KEY,
                    kind TEXT NOT NULL,
                    data TEXT NOT NULL
                )",
                [],
            )
            .unwrap();
            conn.execute(
                "CREATE TABLE graph_edges (
                    id INTEGER PRIMARY KEY,
                    from_id INTEGER NOT NULL,
                    to_id INTEGER NOT NULL,
                    edge_type TEXT NOT NULL
                )",
                [],
            )
            .unwrap();
            conn.execute(
                "CREATE TABLE symbol_metrics (
                    symbol_id TEXT PRIMARY KEY,
                    fan_in INTEGER,
                    fan_out INTEGER,
                    cyclomatic_complexity INTEGER,
                    loc INTEGER
                )",
                [],
            )
            .unwrap();

            // Insert file
            conn.execute(
                "INSERT INTO graph_entities (id, kind, data) VALUES (1, 'File', '{\"path\":\"/test/a.rs\"}')",
                [],
            ).unwrap();

            // Insert two symbols with same name "parse"
            conn.execute(
                "INSERT INTO graph_entities (id, kind, data) VALUES
                    (10, 'Symbol', '{\"name\":\"parse\",\"kind\":\"Function\",\"display_fqn\":\"parse\",\"fqn\":\"a::parse\",\"canonical_fqn\":\"/test/a.rs::parse\",\"symbol_id\":\"target_parse\",\"byte_start\":100,\"byte_end\":200,\"start_line\":5,\"start_col\":0,\"end_line\":10,\"end_col\":1}'),
                    (11, 'Symbol', '{\"name\":\"parse\",\"kind\":\"Function\",\"display_fqn\":\"parse\",\"fqn\":\"b::parse\",\"canonical_fqn\":\"/test/b.rs::parse\",\"symbol_id\":\"other_parse\",\"byte_start\":300,\"byte_end\":400,\"start_line\":15,\"start_col\":0,\"end_line\":20,\"end_col\":1}')",
                [],
            ).unwrap();

            // Insert edges
            conn.execute(
                "INSERT INTO graph_edges (from_id, to_id, edge_type) VALUES (1, 10, 'DEFINES')",
                [],
            )
            .unwrap();

            let db_path = db_file.path();

            // Use symbol_id to get exact match - no ambiguity
            let options = SearchOptions {
                db_path,
                query: "ignored", // Query is ignored when symbol_id is provided
                path_filter: None,
                kind_filter: None,
                limit: 10,
                use_regex: false,
                candidates: 100,
                context: ContextOptions::default(),
                snippet: SnippetOptions::default(),
                fqn: FqnOptions {
                    fqn: false,
                    canonical_fqn: true, // Enable to see canonical_fqn in results
                    display_fqn: false,
                },
                include_score: false,
                sort_by: SortMode::default(),
                metrics: MetricsOptions::default(),
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: Some("target_parse"),
                fqn_pattern: None,
                exact_fqn: None,
                language_filter: None,
            };

            let (response, _partial) = search_symbols(options).unwrap();
            assert_eq!(
                response.results.len(),
                1,
                "Should find exactly 1 result by symbol_id"
            );
            assert_eq!(
                response.results[0].symbol_id.as_deref(),
                Some("target_parse")
            );
            assert_eq!(
                response.results[0].canonical_fqn.as_deref(),
                Some("/test/a.rs::parse")
            );
        }

        #[test]
        fn test_infer_language_from_extension() {
            // Test common language extensions
            assert_eq!(infer_language("src/main.rs"), Some("rust"));
            assert_eq!(infer_language("lib/app.py"), Some("python"));
            assert_eq!(infer_language("component.js"), Some("javascript"));
            assert_eq!(infer_language("module.ts"), Some("typescript"));
            assert_eq!(infer_language("header.h"), Some("c"));
            assert_eq!(infer_language("impl.cpp"), Some("cpp"));
            assert_eq!(infer_language("Main.java"), Some("java"));
            assert_eq!(infer_language("main.go"), Some("go"));

            // Test JSX/TSX variants
            assert_eq!(infer_language("App.jsx"), Some("javascript"));
            assert_eq!(infer_language("App.tsx"), Some("typescript"));

            // Test unknown extensions
            assert_eq!(infer_language("file.xyz"), None);
            assert_eq!(infer_language("README"), None);
            assert_eq!(infer_language("no_extension"), None);
        }

        #[test]
        fn test_normalize_kind_label() {
            // Test that kind normalization lowercases the kind
            assert_eq!(normalize_kind_label("Function"), "function");
            assert_eq!(normalize_kind_label("STRUCT"), "struct");
            assert_eq!(normalize_kind_label("Method"), "method");
            assert_eq!(normalize_kind_label("Class"), "class");
            assert_eq!(normalize_kind_label("enum"), "enum");
        }

        #[test]
        fn test_build_search_query_with_language_filter() {
            // Test that language filter adds file extension filter
            let (sql, params, _strategy) = build_search_query(
                "test",
                None,
                None,
                Some("rust"),
                false,
                false,
                100,
                MetricsOptions::default(),
                SortMode::default(),
                None,
                None,
                None,
                false, // has_ast_table
                &[],   // ast_kinds
                None,  // min_depth
                None,  // max_depth
                None,  // inside_kind
                None,  // contains_kind
            None,  // symbol_set_filter
        );

            // Should filter by .rs extension
            assert!(sql.contains("f.file_path LIKE ? ESCAPE '\\'"));

            // Should have 5 params: 3 LIKE params + 1 language file param + 1 LIMIT
            assert_eq!(params.len(), 5);
        }

        #[test]
        fn test_build_search_query_with_unknown_language() {
            // Test that unknown language doesn't add filter
            let (_sql, params, _) = build_search_query(
                "test",
                None,
                None,
                Some("unknown_language"),
                false,
                false,
                100,
                MetricsOptions::default(),
                SortMode::default(),
                None,
                None,
                None,
                false, // has_ast_table
                &[],   // ast_kinds
                None,  // min_depth
                None,  // max_depth
                None,  // inside_kind
                None,  // contains_kind
            None,  // symbol_set_filter
        );

            // Should NOT add language filter for unknown language
            // Should have 4 params: 3 LIKE + 1 LIMIT (no extra language param)
            assert_eq!(params.len(), 4);
        }

        #[test]
        fn test_build_search_query_combined_language_and_kind() {
            let path = PathBuf::from("/src/module");
            let (sql, params, _strategy) = build_search_query(
                "test",
                Some(&path),
                Some("Function"),
                Some("python"),
                false,
                false,
                100,
                MetricsOptions::default(),
                SortMode::default(),
                None,
                None,
                None,
                false, // has_ast_table
                &[],   // ast_kinds
                None,  // min_depth
                None,  // max_depth
                None,  // inside_kind
                None,  // contains_kind
            None,  // symbol_set_filter
        );

            // Should have both path, kind, and language filters
            assert!(sql.contains("f.file_path LIKE ? ESCAPE '\\'"));
            assert!(sql.contains("s.kind_normalized = ? OR s.kind = ?"));

            // Should have 8 params: 3 LIKE + 1 path + 2 kind + 1 language + 1 LIMIT
            assert_eq!(params.len(), 8);
            assert_eq!(count_params(&sql), 8);
        }

        #[test]
        fn test_build_search_query_with_cpp_language() {
            // Test C++ language alias handling
            let (sql, params, _strategy) = build_search_query(
                "test",
                None,
                None,
                Some("cpp"),
                false,
                false,
                100,
                MetricsOptions::default(),
                SortMode::default(),
                None,
                None,
                None,
                false, // has_ast_table
                &[],   // ast_kinds
                None,  // min_depth
                None,  // max_depth
                None,  // inside_kind
                None,  // contains_kind
            None,  // symbol_set_filter
        );

            // Should filter by .cpp extension
            assert!(sql.contains("f.file_path LIKE ? ESCAPE '\\'"));

            // Should have 5 params: 3 LIKE + 1 language file + 1 LIMIT
            assert_eq!(params.len(), 5);
        }
    }
}
