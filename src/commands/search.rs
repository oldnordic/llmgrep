use crate::cli::{
    looks_like_regex, normalize_language, parse_fields, parse_kinds, resolve_db_path,
    split_auto_limit, validate_path, AutoLimitMode, Cli, Command, SearchMode, SearchParams,
};
use crate::display::{
    output_calls, output_docs, output_facts, output_implements, output_references, output_semantic,
    output_symbols,
};
use llmgrep::algorithm::AlgorithmOptions;
use llmgrep::ast::{expand_shorthand_with_language, expand_shorthands};
use llmgrep::backend::Backend;
use llmgrep::error::LlmError;
use llmgrep::output::{
    json_response_with_partial_and_performance, CombinedSearchResponse, OutputFormat,
    PerformanceMetrics,
};
use llmgrep::query::{
    AstOptions, ContextOptions, DepthOptions, FqnOptions, MetricsOptions, SearchOptions,
    SnippetOptions,
};

pub fn dispatch_search(cli: &Cli, cmd: &Command) -> Result<(), LlmError> {
    let params = match cmd {
        Command::Search {
            query,
            mode,
            path,
            kind,
            language,
            label,
            limit,
            regex,
            candidates,
            with_context,
            context_lines,
            max_context_lines,
            with_snippet,
            with_fqn,
            max_snippet_bytes,
            fields,
            sort_by,
            auto_limit,
            min_complexity,
            max_complexity,
            min_fan_in,
            min_fan_out,
            symbol_id,
            fqn,
            exact_fqn,
            ast_kind,
            with_ast_context,
            min_depth,
            max_depth,
            inside,
            contains,
            from_symbol_set,
            reachable_from,
            dead_code_in,
            in_cycle,
            slice_backward_from,
            slice_forward_from,
            condense,
            paths_from,
            paths_to,
            uncovered,
            covered,
            tags,
            wikilinks,
            source_kind,
            since,
            subject,
            predicate,
            object,
            status,
            subject_type,
        } => SearchParams {
            query: query.clone(),
            mode: *mode,
            path: path.clone(),
            kind: kind.clone(),
            language: language.clone(),
            label: label.clone(),
            limit: *limit,
            regex: *regex,
            candidates: *candidates,
            with_context: *with_context,
            context_lines: *context_lines,
            max_context_lines: *max_context_lines,
            with_snippet: *with_snippet,
            with_fqn: *with_fqn,
            max_snippet_bytes: *max_snippet_bytes,
            fields: fields.clone(),
            sort_by: *sort_by,
            auto_limit: *auto_limit,
            min_complexity: *min_complexity,
            max_complexity: *max_complexity,
            min_fan_in: *min_fan_in,
            min_fan_out: *min_fan_out,
            symbol_id: symbol_id.clone(),
            fqn: fqn.clone(),
            exact_fqn: exact_fqn.clone(),
            ast_kind: ast_kind.clone(),
            with_ast_context: *with_ast_context,
            min_depth: *min_depth,
            max_depth: *max_depth,
            inside: inside.clone(),
            contains: contains.clone(),
            from_symbol_set: from_symbol_set.clone(),
            reachable_from: reachable_from.clone(),
            dead_code_in: dead_code_in.clone(),
            in_cycle: in_cycle.clone(),
            slice_backward_from: slice_backward_from.clone(),
            slice_forward_from: slice_forward_from.clone(),
            condense: *condense,
            paths_from: paths_from.clone(),
            paths_to: paths_to.clone(),
            coverage_filter: if *uncovered {
                Some(llmgrep::query::CoverageFilter::Uncovered)
            } else if *covered {
                Some(llmgrep::query::CoverageFilter::Covered)
            } else {
                None
            },
            tags: tags.clone(),
            wikilinks: wikilinks.clone(),
            source_kind: source_kind.clone(),
            since: *since,
            subject: subject.clone(),
            predicate: predicate.clone(),
            object: object.clone(),
            fact_status_filter: status.clone(),
            subject_type: subject_type.clone(),
        },
        _ => unreachable!(),
    };
    run_search(cli, &params)
}

#[allow(clippy::too_many_arguments)]
pub fn run_search(cli: &Cli, params: &SearchParams) -> Result<(), LlmError> {
    if let Some(sid) = &params.symbol_id {
        let hex_regex =
            regex::Regex::new(r"^[0-9a-f]{32}$").map_err(|_| LlmError::InvalidQuery {
                query: "Failed to compile symbol_id validation regex".to_string(),
            })?;
        if !hex_regex.is_match(sid) {
            return Err(LlmError::InvalidQuery {
                query: format!(
                    "Invalid symbol_id format: '{}'. Expected 32 hex characters (0-9, a-f).",
                    sid
                ),
            });
        }
    }

    let normalized_language = params
        .language
        .as_ref()
        .map(|lang| normalize_language(lang));

    let expanded_ast_kind = if let Some(kind_input) = &params.ast_kind {
        let kinds = if normalized_language.is_some() {
            expand_shorthand_with_language(kind_input, normalized_language.as_deref())
        } else {
            expand_shorthands(kind_input)
        };
        if !kinds.is_empty() {
            Some(kinds.join(","))
        } else {
            None
        }
    } else {
        None
    };

    let normalized_kind = params.kind.as_ref().map(|k| {
        let kinds = parse_kinds(k);
        if kinds.is_empty() {
            k.to_lowercase()
        } else {
            kinds.join(",")
        }
    });

    let auto_regex = !params.regex && looks_like_regex(&params.query);
    let use_regex = params.regex || auto_regex;
    if auto_regex {
        eprintln!(
            "Note: Auto-enabled --regex mode for query '{}' (detected regex pattern)",
            params.query
        );
    }

    if params.fqn.is_some() && params.exact_fqn.is_some() {
        return Err(LlmError::InvalidQuery {
            query: "--fqn and --exact-fqn are mutually exclusive. Use only one.".to_string(),
        });
    }

    if params.coverage_filter == Some(llmgrep::query::CoverageFilter::Uncovered)
        && params.coverage_filter == Some(llmgrep::query::CoverageFilter::Covered)
    {
        return Err(LlmError::InvalidQuery {
            query: "--uncovered and --covered are mutually exclusive. Use only one.".to_string(),
        });
    }

    if params.symbol_id.is_some() {
        eprintln!(
            "Note: --symbol-id provided, using direct lookup. Query '{}' will be used as secondary filter if needed.",
            params.query
        );
    }

    if params.query.trim().is_empty()
        && params.symbol_id.is_none()
        && !params.condense
        && params.paths_from.is_none()
        && !matches!(params.mode, SearchMode::Docs | SearchMode::Facts)
    {
        return Err(LlmError::EmptyQuery);
    }

    let db_path = resolve_db_path(cli)?;

    let total_start = std::time::Instant::now();

    let detect_start = std::time::Instant::now();
    let backend = Backend::detect_and_open(&db_path)?;
    let backend_detection_ms = detect_start.elapsed().as_millis() as u64;

    let validated_path = if let Some(p) = &params.path {
        Some(validate_path(p, false)?)
    } else {
        None
    };
    let wants_json = matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty);
    let candidates = params.candidates.max(params.limit);
    let fields = if wants_json {
        params
            .fields
            .as_ref()
            .map(|value| parse_fields(value))
            .transpose()?
    } else {
        None
    };

    let include_context = wants_json && fields.as_ref().map_or(params.with_context, |f| f.context);
    let include_snippet = wants_json && fields.as_ref().map_or(params.with_snippet, |f| f.snippet);
    let include_score = if wants_json {
        fields.as_ref().is_none_or(|f| f.score)
    } else {
        true
    };

    let include_fqn = wants_json && fields.as_ref().map_or(params.with_fqn, |f| f.fqn);
    let include_canonical_fqn =
        wants_json && fields.as_ref().map_or(params.with_fqn, |f| f.canonical_fqn);
    let include_display_fqn =
        wants_json && fields.as_ref().map_or(params.with_fqn, |f| f.display_fqn);

    let metrics = MetricsOptions {
        min_complexity: params.min_complexity,
        max_complexity: params.max_complexity,
        min_fan_in: params.min_fan_in,
        min_fan_out: params.min_fan_out,
    };

    match params.mode {
        SearchMode::Symbols => {
            let options = SearchOptions {
                db_path: &db_path,
                query: &params.query,
                path_filter: validated_path.as_ref(),
                kind_filter: normalized_kind.as_deref(),
                language_filter: normalized_language.as_deref(),
                limit: params.limit,
                use_regex,
                candidates,
                context: ContextOptions {
                    include: include_context,
                    lines: params.context_lines,
                    max_lines: params.max_context_lines,
                },
                snippet: SnippetOptions {
                    include: include_snippet,
                    max_bytes: params.max_snippet_bytes,
                },
                fqn: FqnOptions {
                    fqn: include_fqn,
                    canonical_fqn: include_canonical_fqn,
                    display_fqn: include_display_fqn,
                },
                include_score,
                sort_by: params.sort_by,
                metrics,
                ast: AstOptions {
                    ast_kinds: expanded_ast_kind
                        .as_ref()
                        .map(|k| k.split(',').map(|s| s.trim().to_string()).collect())
                        .unwrap_or_default(),
                    with_ast_context: params.with_ast_context,
                    _phantom: std::marker::PhantomData,
                },
                depth: DepthOptions {
                    min_depth: params.min_depth,
                    max_depth: params.max_depth,
                    inside: params.inside.as_deref(),
                    contains: params.contains.as_deref(),
                },
                algorithm: AlgorithmOptions {
                    from_symbol_set: params.from_symbol_set.as_deref(),
                    reachable_from: params.reachable_from.as_deref(),
                    dead_code_in: params.dead_code_in.as_deref(),
                    in_cycle: params.in_cycle.as_deref(),
                    slice_backward_from: params.slice_backward_from.as_deref(),
                    slice_forward_from: params.slice_forward_from.as_deref(),
                    condense: params.condense,
                    paths_from: params.paths_from.as_deref(),
                    paths_to: params.paths_to.as_deref(),
                },
                symbol_id: params.symbol_id.as_deref(),
                fqn_pattern: params.fqn.as_deref(),
                exact_fqn: params.exact_fqn.as_deref(),
                coverage_filter: None,
            };

            let query_start = std::time::Instant::now();
            let (mut response, partial, paths_bounded) = backend.search_symbols(options)?;
            let query_execution_ms = query_start.elapsed().as_millis() as u64;

            let scc_count: usize = response
                .results
                .iter()
                .filter_map(|r| r.supernode_id.as_ref())
                .collect::<std::collections::HashSet<_>>()
                .len();

            if params.condense && scc_count == 0 {
                response.notice =
                    Some("No SCCs found - codebase is acyclic (no cycles detected)".to_string());
            }

            if paths_bounded {
                eprintln!("Warning: Path enumeration hit bounds (max-depth=100, max-paths=1000)");
                eprintln!("         Results may be incomplete. Use magellan paths directly with adjusted bounds for full enumeration.");
            }

            if response.total_count == 0 {
                if let Some(from) = &params.paths_from {
                    eprintln!("Note: No execution paths found from '{from}'");
                    if let Some(to) = &params.paths_to {
                        eprintln!("      to '{to}'. Symbols may be unreachable.");
                    }
                }
            }

            let format_start = std::time::Instant::now();
            let metrics = if cli.show_metrics {
                Some(PerformanceMetrics {
                    backend_detection_ms,
                    query_execution_ms,
                    output_formatting_ms: 0,
                    total_ms: 0,
                })
            } else {
                None
            };

            output_symbols(cli, response, partial, scc_count, metrics.as_ref())?;

            let output_formatting_ms = format_start.elapsed().as_millis() as u64;
            let total_ms = total_start.elapsed().as_millis() as u64;

            if cli.show_metrics {
                eprintln!("Performance metrics:");
                eprintln!("  Backend detection: {}ms", backend_detection_ms);
                eprintln!("  Query execution: {}ms", query_execution_ms);
                eprintln!("  Output formatting: {}ms", output_formatting_ms);
                eprintln!("  Total: {}ms", total_ms);
            }
        }
        SearchMode::References => {
            let options = SearchOptions {
                db_path: &db_path,
                query: &params.query,
                path_filter: validated_path.as_ref(),
                kind_filter: None,
                language_filter: None,
                limit: params.limit,
                use_regex,
                candidates,
                context: ContextOptions {
                    include: include_context,
                    lines: params.context_lines,
                    max_lines: params.max_context_lines,
                },
                snippet: SnippetOptions {
                    include: include_snippet,
                    max_bytes: params.max_snippet_bytes,
                },
                fqn: FqnOptions::default(),
                include_score,
                sort_by: params.sort_by,
                metrics,
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                coverage_filter: None,
            };

            let query_start = std::time::Instant::now();
            let (response, partial) = backend.search_references(options)?;
            let query_execution_ms = query_start.elapsed().as_millis() as u64;

            let format_start = std::time::Instant::now();
            let metrics = if cli.show_metrics {
                Some(PerformanceMetrics {
                    backend_detection_ms,
                    query_execution_ms,
                    output_formatting_ms: 0,
                    total_ms: 0,
                })
            } else {
                None
            };

            output_references(cli, response, partial, metrics.as_ref())?;

            let output_formatting_ms = format_start.elapsed().as_millis() as u64;
            let total_ms = total_start.elapsed().as_millis() as u64;

            if cli.show_metrics {
                eprintln!("Performance metrics:");
                eprintln!("  Backend detection: {}ms", backend_detection_ms);
                eprintln!("  Query execution: {}ms", query_execution_ms);
                eprintln!("  Output formatting: {}ms", output_formatting_ms);
                eprintln!("  Total: {}ms", total_ms);
            }
        }
        SearchMode::Calls => {
            let options = SearchOptions {
                db_path: &db_path,
                query: &params.query,
                path_filter: validated_path.as_ref(),
                kind_filter: None,
                language_filter: None,
                limit: params.limit,
                use_regex,
                candidates,
                context: ContextOptions {
                    include: include_context,
                    lines: params.context_lines,
                    max_lines: params.max_context_lines,
                },
                snippet: SnippetOptions {
                    include: include_snippet,
                    max_bytes: params.max_snippet_bytes,
                },
                fqn: FqnOptions::default(),
                include_score,
                sort_by: params.sort_by,
                metrics,
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                coverage_filter: None,
            };

            let query_start = std::time::Instant::now();
            let (response, partial) = backend.search_calls(options)?;
            let query_execution_ms = query_start.elapsed().as_millis() as u64;

            let format_start = std::time::Instant::now();
            let metrics = if cli.show_metrics {
                Some(PerformanceMetrics {
                    backend_detection_ms,
                    query_execution_ms,
                    output_formatting_ms: 0,
                    total_ms: 0,
                })
            } else {
                None
            };

            output_calls(cli, response, partial, metrics.as_ref())?;

            let output_formatting_ms = format_start.elapsed().as_millis() as u64;
            let total_ms = total_start.elapsed().as_millis() as u64;

            if cli.show_metrics {
                eprintln!("Performance metrics:");
                eprintln!("  Backend detection: {}ms", backend_detection_ms);
                eprintln!("  Query execution: {}ms", query_execution_ms);
                eprintln!("  Output formatting: {}ms", output_formatting_ms);
                eprintln!("  Total: {}ms", total_ms);
            }
        }
        SearchMode::Auto => {
            if !wants_json {
                return Err(LlmError::InvalidQuery {
                    query: "auto mode requires JSON output".to_string(),
                });
            }
            let (symbols_limit, references_limit, calls_limit) = match params.auto_limit {
                AutoLimitMode::PerMode => (params.limit, params.limit, params.limit),
                AutoLimitMode::Global => split_auto_limit(params.limit),
            };

            let (symbols, symbols_partial, _) = backend.search_symbols(SearchOptions {
                db_path: &db_path,
                query: &params.query,
                path_filter: validated_path.as_ref(),
                kind_filter: normalized_kind.as_deref(),
                language_filter: normalized_language.as_deref(),
                limit: symbols_limit,
                use_regex,
                candidates,
                context: ContextOptions {
                    include: include_context,
                    lines: params.context_lines,
                    max_lines: params.max_context_lines,
                },
                snippet: SnippetOptions {
                    include: include_snippet,
                    max_bytes: params.max_snippet_bytes,
                },
                fqn: FqnOptions {
                    fqn: include_fqn,
                    canonical_fqn: include_canonical_fqn,
                    display_fqn: include_display_fqn,
                },
                include_score,
                sort_by: params.sort_by,
                metrics,
                ast: AstOptions {
                    ast_kinds: expanded_ast_kind
                        .as_ref()
                        .map(|k| k.split(',').map(|s| s.trim().to_string()).collect())
                        .unwrap_or_default(),
                    with_ast_context: params.with_ast_context,
                    _phantom: std::marker::PhantomData,
                },
                depth: DepthOptions {
                    min_depth: params.min_depth,
                    max_depth: params.max_depth,
                    inside: params.inside.as_deref(),
                    contains: params.contains.as_deref(),
                },
                algorithm: AlgorithmOptions::default(),
                symbol_id: params.symbol_id.as_deref(),
                fqn_pattern: params.fqn.as_deref(),
                exact_fqn: params.exact_fqn.as_deref(),
                coverage_filter: None,
            })?;
            let (references, refs_partial) = backend.search_references(SearchOptions {
                db_path: &db_path,
                query: &params.query,
                path_filter: validated_path.as_ref(),
                kind_filter: None,
                language_filter: None,
                limit: references_limit,
                use_regex,
                candidates,
                context: ContextOptions {
                    include: include_context,
                    lines: params.context_lines,
                    max_lines: params.max_context_lines,
                },
                snippet: SnippetOptions {
                    include: include_snippet,
                    max_bytes: params.max_snippet_bytes,
                },
                fqn: FqnOptions::default(),
                include_score,
                sort_by: params.sort_by,
                metrics,
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                coverage_filter: None,
            })?;
            let (calls, calls_partial) = backend.search_calls(SearchOptions {
                db_path: &db_path,
                query: &params.query,
                path_filter: validated_path.as_ref(),
                kind_filter: None,
                language_filter: None,
                limit: calls_limit,
                use_regex,
                candidates,
                context: ContextOptions {
                    include: include_context,
                    lines: params.context_lines,
                    max_lines: params.max_context_lines,
                },
                snippet: SnippetOptions {
                    include: include_snippet,
                    max_bytes: params.max_snippet_bytes,
                },
                fqn: FqnOptions::default(),
                include_score,
                sort_by: params.sort_by,
                metrics,
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                coverage_filter: None,
            })?;
            let total_count = symbols.total_count + references.total_count + calls.total_count;
            let combined = CombinedSearchResponse {
                query: params.query.to_string(),
                path_filter: validated_path
                    .as_ref()
                    .map(|p| p.to_string_lossy().to_string()),
                symbols,
                references,
                calls,
                total_count,
                limit_mode: match params.auto_limit {
                    AutoLimitMode::PerMode => "per-mode".to_string(),
                    AutoLimitMode::Global => "global".to_string(),
                },
            };
            let partial = symbols_partial || refs_partial || calls_partial;

            let query_execution_ms =
                total_start.elapsed().as_millis() as u64 - backend_detection_ms;

            let format_start = std::time::Instant::now();
            let metrics = if cli.show_metrics {
                Some(PerformanceMetrics {
                    backend_detection_ms,
                    query_execution_ms,
                    output_formatting_ms: 0,
                    total_ms: 0,
                })
            } else {
                None
            };

            let payload = json_response_with_partial_and_performance(combined, partial, metrics);
            let rendered = if matches!(cli.output, OutputFormat::Pretty) {
                serde_json::to_string_pretty(&payload)
            } else {
                serde_json::to_string(&payload)
            }?;
            println!("{}", rendered);

            let output_formatting_ms = format_start.elapsed().as_millis() as u64;
            let total_ms = total_start.elapsed().as_millis() as u64;

            if cli.show_metrics {
                eprintln!("Performance metrics:");
                eprintln!("  Backend detection: {}ms", backend_detection_ms);
                eprintln!("  Query execution: {}ms", query_execution_ms);
                eprintln!("  Output formatting: {}ms", output_formatting_ms);
                eprintln!("  Total: {}ms", total_ms);
            }
        }
        SearchMode::Labels => {
            let label_name = params.label.clone().unwrap_or("test".to_string());

            let query_start = std::time::Instant::now();
            let db_path_str = db_path.to_str().ok_or_else(|| LlmError::SearchFailed {
                reason: format!("Database path {:?} is not valid UTF-8", db_path),
            })?;
            let (response, partial, _paths_bounded) =
                backend.search_by_label(&label_name, params.limit, db_path_str)?;
            let query_execution_ms = query_start.elapsed().as_millis() as u64;

            let format_start = std::time::Instant::now();
            let metrics = if cli.show_metrics {
                Some(PerformanceMetrics {
                    backend_detection_ms,
                    query_execution_ms,
                    output_formatting_ms: 0,
                    total_ms: 0,
                })
            } else {
                None
            };

            output_symbols(cli, response, partial, 0, metrics.as_ref())?;

            let output_formatting_ms = format_start.elapsed().as_millis() as u64;
            let total_ms = total_start.elapsed().as_millis() as u64;

            if cli.show_metrics {
                eprintln!("Performance metrics:");
                eprintln!("  Backend detection: {}ms", backend_detection_ms);
                eprintln!("  Query execution: {}ms", query_execution_ms);
                eprintln!("  Output formatting: {}ms", output_formatting_ms);
                eprintln!("  Total: {}ms", total_ms);
            }
        }
        SearchMode::Implements => {
            let options = SearchOptions {
                db_path: &db_path,
                query: &params.query,
                path_filter: validated_path.as_ref(),
                kind_filter: None,
                language_filter: None,
                limit: params.limit,
                use_regex,
                candidates,
                context: ContextOptions {
                    include: include_context,
                    lines: params.context_lines,
                    max_lines: params.max_context_lines,
                },
                snippet: SnippetOptions {
                    include: include_snippet,
                    max_bytes: params.max_snippet_bytes,
                },
                fqn: FqnOptions::default(),
                include_score,
                sort_by: params.sort_by,
                metrics,
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
                coverage_filter: None,
            };

            let query_start = std::time::Instant::now();
            let (response, partial) = backend.search_implements(options)?;
            let query_execution_ms = query_start.elapsed().as_millis() as u64;

            let format_start = std::time::Instant::now();
            let metrics = if cli.show_metrics {
                Some(PerformanceMetrics {
                    backend_detection_ms,
                    query_execution_ms,
                    output_formatting_ms: 0,
                    total_ms: 0,
                })
            } else {
                None
            };

            output_implements(cli, response, partial, metrics.as_ref())?;

            let output_formatting_ms = format_start.elapsed().as_millis() as u64;
            let total_ms = total_start.elapsed().as_millis() as u64;

            if cli.show_metrics {
                eprintln!("Performance metrics:");
                eprintln!("  Backend detection: {}ms", backend_detection_ms);
                eprintln!("  Query execution: {}ms", query_execution_ms);
                eprintln!("  Output formatting: {}ms", output_formatting_ms);
                eprintln!("  Total: {}ms", total_ms);
            }
        }
        SearchMode::Docs => {
            let docs_options = llmgrep::query::DocsSearchOptions {
                db_path: &db_path,
                limit: params.limit,
                tags: params.tags.as_deref(),
                wikilinks: params.wikilinks.as_deref(),
                source_kind: params.source_kind.as_deref(),
                since: params.since,
                path: validated_path.as_ref().and_then(|p| p.to_str()),
            };

            let query_start = std::time::Instant::now();
            let response = backend.search_docs(docs_options)?;
            let query_execution_ms = query_start.elapsed().as_millis() as u64;

            let format_start = std::time::Instant::now();
            let metrics = if cli.show_metrics {
                Some(PerformanceMetrics {
                    backend_detection_ms,
                    query_execution_ms,
                    output_formatting_ms: 0,
                    total_ms: 0,
                })
            } else {
                None
            };

            output_docs(cli, response, metrics.as_ref())?;

            let output_formatting_ms = format_start.elapsed().as_millis() as u64;
            let total_ms = total_start.elapsed().as_millis() as u64;

            if cli.show_metrics {
                eprintln!("Performance metrics:");
                eprintln!("  Backend detection: {}ms", backend_detection_ms);
                eprintln!("  Query execution: {}ms", query_execution_ms);
                eprintln!("  Output formatting: {}ms", output_formatting_ms);
                eprintln!("  Total: {}ms", total_ms);
            }
        }
        SearchMode::Facts => {
            let facts_options = llmgrep::query::FactsSearchOptions {
                db_path: &db_path,
                limit: params.limit,
                subject: params.subject.as_deref(),
                predicate: params.predicate.as_deref(),
                object: params.object.as_deref(),
                status: params.fact_status_filter.as_deref(),
                subject_type: params.subject_type.as_deref(),
            };

            let query_start = std::time::Instant::now();
            let response = backend.search_facts(facts_options)?;
            let query_execution_ms = query_start.elapsed().as_millis() as u64;

            let format_start = std::time::Instant::now();
            let metrics = if cli.show_metrics {
                Some(PerformanceMetrics {
                    backend_detection_ms,
                    query_execution_ms,
                    output_formatting_ms: 0,
                    total_ms: 0,
                })
            } else {
                None
            };

            output_facts(cli, response, metrics.as_ref())?;

            let output_formatting_ms = format_start.elapsed().as_millis() as u64;
            let total_ms = total_start.elapsed().as_millis() as u64;

            if cli.show_metrics {
                eprintln!("Performance metrics:");
                eprintln!("  Backend detection: {}ms", backend_detection_ms);
                eprintln!("  Query execution: {}ms", query_execution_ms);
                eprintln!("  Output formatting: {}ms", output_formatting_ms);
                eprintln!("  Total: {}ms", total_ms);
            }
        }
        SearchMode::Semantic => {
            let semantic_options = llmgrep::query::SemanticSearchOptions {
                db_path: &db_path,
                query: &params.query,
                limit: params.limit,
                path_filter: validated_path.as_ref().and_then(|p| p.to_str()),
            };

            let query_start = std::time::Instant::now();
            let response = llmgrep::query::search_semantic(semantic_options)?;
            let query_execution_ms = query_start.elapsed().as_millis() as u64;

            let format_start = std::time::Instant::now();
            let metrics = if cli.show_metrics {
                Some(PerformanceMetrics {
                    backend_detection_ms,
                    query_execution_ms,
                    output_formatting_ms: 0,
                    total_ms: 0,
                })
            } else {
                None
            };

            output_semantic(cli, response, metrics.as_ref())?;

            let output_formatting_ms = format_start.elapsed().as_millis() as u64;
            let total_ms = total_start.elapsed().as_millis() as u64;

            if cli.show_metrics {
                eprintln!("Performance metrics:");
                eprintln!("  Backend detection: {}ms", backend_detection_ms);
                eprintln!("  Query execution: {}ms", query_execution_ms);
                eprintln!("  Output formatting: {}ms", output_formatting_ms);
                eprintln!("  Total: {}ms", total_ms);
            }
        }
    }

    Ok(())
}
