use clap::builder::{RangedI64ValueParser, TypedValueParser};
use clap::{Parser, Subcommand, ValueEnum};
use llmgrep::algorithm::AlgorithmOptions;
use llmgrep::error::LlmError;
use llmgrep::output::{
    json_response, json_response_with_partial, CallSearchResponse, CombinedSearchResponse,
    ErrorResponse, OutputFormat, ReferenceSearchResponse, SearchResponse,
};
use llmgrep::output_common::{format_partial_footer, format_total_header, render_json_response};
use llmgrep::query::{
    search_calls, search_references, search_symbols, AstOptions, ContextOptions, DepthOptions,
    FqnOptions, MetricsOptions, SearchOptions, SnippetOptions,
};
use llmgrep::ast::{expand_shorthand_with_language, expand_shorthands};
use llmgrep::SortMode;
use std::path::{Path, PathBuf};

// Custom value parser for ranged usize - needed because clap doesn't provide RangedUsizeValueParser
fn ranged_usize(min: i64, max: i64) -> impl TypedValueParser<Value = usize> {
    let inner = RangedI64ValueParser::new().range(min..=max);
    // Map i64 to usize - this is safe because the range ensures valid values
    inner.map(|v: i64| v as usize)
}

#[derive(Parser)]
#[command(
    name = "llmgrep",
    version = env!("CARGO_PKG_VERSION"),
    about = "Smart grep backed by a Magellan code map"
)]
struct Cli {
    #[arg(long, global = true, default_value_t = OutputFormat::Human)]
    output: OutputFormat,

    #[arg(long, global = true)]
    db: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    #[command(after_help = SEARCH_EXAMPLES)]
    Search {
        #[arg(long)]
        query: String,

        #[arg(long, value_enum, default_value = "symbols")]
        mode: SearchMode,

        #[arg(long)]
        path: Option<PathBuf>,

        #[arg(long)]
        kind: Option<String>,

        #[arg(long)]
        language: Option<String>,

        #[arg(long, default_value_t = 50, value_parser = ranged_usize(1, 1000))]
        limit: usize,

        #[arg(long)]
        regex: bool,

        #[arg(long, default_value_t = 500, value_parser = ranged_usize(1, 10000))]
        candidates: usize,

        #[arg(long)]
        with_context: bool,

        #[arg(long, default_value_t = 3, value_parser = ranged_usize(1, 100))]
        context_lines: usize,

        #[arg(long, default_value_t = 20, value_parser = ranged_usize(1, 500))]
        max_context_lines: usize,

        #[arg(long)]
        with_snippet: bool,

        #[arg(long)]
        with_fqn: bool,

        #[arg(long, default_value_t = 200, value_parser = ranged_usize(1, 1_048_576))]
        max_snippet_bytes: usize,

        #[arg(long)]
        fields: Option<String>,

        #[arg(long, value_enum, default_value = "relevance")]
        sort_by: SortMode,

        #[arg(long, value_enum, default_value = "per-mode")]
        auto_limit: AutoLimitMode,

        // Metrics-based filtering flags
        #[arg(long, value_parser = ranged_usize(0, 1000))]
        min_complexity: Option<usize>,

        #[arg(long, value_parser = ranged_usize(0, 1000))]
        max_complexity: Option<usize>,

        #[arg(long, value_parser = ranged_usize(0, 10000))]
        min_fan_in: Option<usize>,

        #[arg(long, value_parser = ranged_usize(0, 10000))]
        min_fan_out: Option<usize>,

        // SymbolId and FQN flags
        #[arg(long)]
        symbol_id: Option<String>,

        #[arg(long)]
        fqn: Option<String>,

        #[arg(long)]
        exact_fqn: Option<String>,

        // AST filtering flag
        #[arg(long, value_name = "KIND")]
        ast_kind: Option<String>,

        // Enriched AST context flag
        #[arg(long)]
        with_ast_context: bool,

        // Depth filtering flags
        #[arg(long, value_parser = ranged_usize(0, 100))]
        min_depth: Option<usize>,

        #[arg(long, value_parser = ranged_usize(0, 100))]
        max_depth: Option<usize>,

        // Structural search flags
        #[arg(long, value_name = "KIND")]
        inside: Option<String>,

        #[arg(long, value_name = "KIND")]
        contains: Option<String>,

        // Algorithm-based filtering flags
        #[arg(long, value_name = "FILE")]
        from_symbol_set: Option<String>,

        #[arg(long, value_name = "SYMBOL")]
        reachable_from: Option<String>,

        #[arg(long, value_name = "SYMBOL")]
        dead_code_in: Option<String>,

        #[arg(long, value_name = "SYMBOL")]
        in_cycle: Option<String>,

        #[arg(long, value_name = "SYMBOL")]
        slice_backward_from: Option<String>,

        #[arg(long, value_name = "SYMBOL")]
        slice_forward_from: Option<String>,

        #[arg(long)]
        condense: bool,

        #[arg(long, value_name = "SYMBOL")]
        paths_from: Option<String>,

        #[arg(long, value_name = "SYMBOL")]
        paths_to: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum SearchMode {
    Symbols,
    References,
    Calls,
    Auto,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum AutoLimitMode {
    PerMode,
    Global,
}

const SEARCH_EXAMPLES: &str = r#"
EXAMPLES:
  # Basic symbol search
  llmgrep --db code.db search --query "parse"

  # Regex search for pattern matching
  llmgrep --db code.db search --query "^main" --regex

  # JSON output for programmatic use
  llmgrep --db code.db search --query "Parser" --output json

  # Search with path filter
  llmgrep --db code.db search --query "Error" --path src/

  # Reference search
  llmgrep --db code.db search --query "Token" --mode references

  # Calls search
  llmgrep --db code.db search --query "parse" --mode calls

  # Auto mode (all search modes combined, requires JSON output)
  llmgrep --db code.db search --query "parse" --mode auto --output json

  # Combined filters with regex
  llmgrep --db code.db search --query "^[A-Z]" --regex --kind Function --output pretty

V1.1 FEATURES:
  # SymbolId lookup (unambiguous reference)
  llmgrep --db code.db search --symbol-id abc123def456789abc123def456789ab

  # Filter by complexity and language
  llmgrep --db code.db search --query "handler" --min-complexity 10 --language rust

  # Sort by fan-in to find hotspots
  llmgrep --db code.db search --query ".*" --sort-by fan-in --limit 20

  # FQN pattern matching
  llmgrep --db code.db search --query "test" --fqn "%module::tests::%"

V2.0 AST FEATURES:
  # Filter by AST node kind using shorthands
  llmgrep --db code.db search --query ".*" --ast-kind loops

  # Filter by specific node kind
  llmgrep --db code.db search --query "parse" --ast-kind call_expression

  # Combine multiple shorthands and kinds
  llmgrep --db code.db search --query "process" --ast-kind loops,conditionals

  # Search with enriched AST context (depth, parent_kind, children, decision_points)
  llmgrep --db code.db search --query "process" --with-ast-context --output json

  # Find deeply nested code (complexity hotspots)
  llmgrep --db code.db search --query ".*" --min-depth 5 --output json

  # Find only top-level code
  llmgrep --db code.db search --query "process" --max-depth 1

  # Find closures within functions
  llmgrep --db code.db search --query ".*" --inside function_item --ast-kind closure_expression

  # Find functions containing async calls
  llmgrep --db code.db search --query ".*" --contains await_expression --ast-kind function_item

  # Find code at specific depth
  llmgrep --db code.db search --query ".*" --min-depth 2 --max-depth 3

  AST Shorthands (expand to multiple node kinds):
    loops              - Loop constructs (for_expression, while_expression, loop_expression)
    conditionals       - Conditionals (if_expression, match_expression, match_arm)
    functions          - Functions and closures (function_item, closure_expression)
    declarations       - Declarations (struct, enum, let, const, static, type_alias)
    unsafe             - Unsafe blocks
    types              - Type definitions (struct, enum, type_alias, union)
    macros             - Macro invocations and definitions
    mods               - Module declarations
    traits             - Trait items and impls
    impls              - Impl blocks

    Language-aware shorthands (with --language):
    --ast-kind functions --language python    - function_definition, lambda, async_function_definition
    --ast-kind functions --language javascript - function_declaration, arrow_function, etc.
    --ast-kind functions --language typescript - function_declaration, arrow_function, etc.

  Specific node kinds also supported:
    function_item, block, call_expression, let_declaration, expression_statement,
    attribute_item, mod_item, closure_expression, if_expression, match_expression,
    await_expression, and many more.

  Use --ast-kind multiple times or comma-separate for combined filters.
  See MANUAL.md for complete node kind reference per language.
"#;

fn validate_path(path: &Path, is_database: bool) -> Result<PathBuf, LlmError> {
    // Canonicalize the path to resolve symlinks and .. components
    let canonical = path
        .canonicalize()
        .map_err(|e| LlmError::PathValidationFailed {
            path: path.display().to_string(),
            reason: format!("Cannot resolve path: {}", e),
        })?;

    // For database paths, verify the file exists and is a regular file
    if is_database {
        if !canonical.exists() {
            return Err(LlmError::DatabaseNotFound {
                path: path.display().to_string(),
            });
        }
        if !canonical.is_file() {
            return Err(LlmError::PathValidationFailed {
                path: path.display().to_string(),
                reason: "Database path must be a file, not a directory".to_string(),
            });
        }
    }

    // Block access to sensitive system directories
    let sensitive_dirs = [
        "/etc", "/root", "/boot", "/sys", "/proc", "/dev", "/run", "/var/run", "/var/tmp",
    ];
    for sensitive in sensitive_dirs {
        if canonical.starts_with(sensitive) {
            return Err(LlmError::PathValidationFailed {
                path: path.display().to_string(),
                reason: format!("Access to {} is not allowed", sensitive),
            });
        }
    }

    // Block access to SSH/config directories
    if let Some(home) = std::env::var_os("HOME") {
        let home_path = PathBuf::from(&home);
        let ssh_dir = home_path.join(".ssh");
        let config_dir = home_path.join(".config");
        if canonical.starts_with(&ssh_dir) || canonical.starts_with(&config_dir) {
            return Err(LlmError::PathValidationFailed {
                path: path.display().to_string(),
                reason: "Access to sensitive home directories is not allowed".to_string(),
            });
        }
    }

    Ok(canonical)
}

fn main() {
    let cli = Cli::parse();
    if let Err(err) = dispatch(&cli) {
        emit_error(&cli, &err);
        std::process::exit(1);
    }
}

fn dispatch(cli: &Cli) -> Result<(), LlmError> {
    match &cli.command {
        Command::Search {
            query,
            mode,
            path,
            kind,
            language,
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
        } => run_search(
            cli,
            query,
            *mode,
            path,
            kind,
            language,
            *limit,
            *regex,
            *candidates,
            *with_context,
            *context_lines,
            *max_context_lines,
            *with_snippet,
            *with_fqn,
            *max_snippet_bytes,
            fields.as_ref(),
            *sort_by,
            *auto_limit,
            *min_complexity,
            *max_complexity,
            *min_fan_in,
            *min_fan_out,
            symbol_id.as_ref(),
            fqn.as_ref(),
            exact_fqn.as_ref(),
            ast_kind.as_ref(),
            *with_ast_context,
            *min_depth,
            *max_depth,
            inside.as_ref().map(|s| s.as_str()),
            contains.as_ref().map(|s| s.as_str()),
            from_symbol_set.as_ref(),
            reachable_from.as_ref(),
            dead_code_in.as_ref(),
            in_cycle.as_ref(),
            slice_backward_from.as_ref(),
            slice_forward_from.as_ref(),
            *condense,
            paths_from.as_ref(),
            paths_to.as_ref(),
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn run_search(
    cli: &Cli,
    query: &str,
    mode: SearchMode,
    path: &Option<PathBuf>,
    kind: &Option<String>,
    language: &Option<String>,
    limit: usize,
    regex: bool,
    candidates: usize,
    with_context: bool,
    context_lines: usize,
    max_context_lines: usize,
    with_snippet: bool,
    with_fqn: bool,
    max_snippet_bytes: usize,
    fields: Option<&String>,
    sort_by: SortMode,
    auto_limit: AutoLimitMode,
    min_complexity: Option<usize>,
    max_complexity: Option<usize>,
    min_fan_in: Option<usize>,
    min_fan_out: Option<usize>,
    symbol_id: Option<&String>,
    fqn: Option<&String>,
    exact_fqn: Option<&String>,
    ast_kind: Option<&String>,
    with_ast_context: bool,
    min_depth: Option<usize>,
    max_depth: Option<usize>,
    inside: Option<&str>,
    contains: Option<&str>,
    from_symbol_set: Option<&String>,
    reachable_from: Option<&String>,
    dead_code_in: Option<&String>,
    in_cycle: Option<&String>,
    slice_backward_from: Option<&String>,
    slice_forward_from: Option<&String>,
    condense: bool,
    paths_from: Option<&String>,
    paths_to: Option<&String>,
) -> Result<(), LlmError> {
    // Validate SymbolId format (32 hex characters)
    if let Some(sid) = symbol_id {
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

    // Normalize and validate language filter
    let normalized_language = language.as_ref().map(|lang| normalize_language(lang));

    // Expand AST shorthands with language-aware expansion
    let expanded_ast_kind = if let Some(kind_input) = ast_kind {
        // Use language-aware expansion if language is specified
        let kinds = if normalized_language.is_some() {
            expand_shorthand_with_language(
                kind_input,
                normalized_language.as_deref(),
            )
        } else {
            // Use Rust shorthands by default
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

    // Normalize kind value (handles comma-separated values for future use)
    // For now, we still pass single kind to SearchOptions but normalize it
    let normalized_kind = kind.as_ref().map(|k| {
        let kinds = parse_kinds(k);
        // For backward compatibility with current implementation,
        // use the first kind. Future enhancement: support multiple kinds.
        if !kinds.is_empty() {
            kinds[0].clone()
        } else {
            k.to_lowercase()
        }
    });

    // Validate mutual exclusivity of --fqn and --exact-fqn
    if fqn.is_some() && exact_fqn.is_some() {
        return Err(LlmError::InvalidQuery {
            query: "--fqn and --exact-fqn are mutually exclusive. Use only one.".to_string(),
        });
    }

    // Warn if both --query and --symbol-id are provided (symbol_id takes precedence)
    if symbol_id.is_some() {
        eprintln!(
            "Note: --symbol-id provided, using direct lookup. Query '{}' will be used as secondary filter if needed.",
            query
        );
    }

    if query.trim().is_empty() && symbol_id.is_none() && !condense {
        return Err(LlmError::EmptyQuery);
    }

    // Validate database path before any operations
    let validated_db = if let Some(db_path) = &cli.db {
        Some(validate_path(db_path, true)?)
    } else {
        return Err(LlmError::DatabaseNotFound {
            path: "none".to_string(),
        });
    };

    let db_path = validated_db.as_ref().expect("validated db path missing");

    // Validate path filter if provided
    let validated_path = if let Some(p) = path {
        Some(validate_path(p, false)?)
    } else {
        None
    };
    let wants_json = matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty);
    let candidates = candidates.max(limit);
    let fields = if wants_json {
        fields.map(|value| parse_fields(value)).transpose()?
    } else {
        None
    };

    let include_context = wants_json && fields.as_ref().map_or(with_context, |f| f.context);
    let include_snippet = wants_json && fields.as_ref().map_or(with_snippet, |f| f.snippet);
    let include_score = if wants_json {
        fields.as_ref().is_none_or(|f| f.score)
    } else {
        true
    };

    let include_fqn = wants_json && fields.as_ref().map_or(with_fqn, |f| f.fqn);
    let include_canonical_fqn = wants_json && fields.as_ref().map_or(with_fqn, |f| f.canonical_fqn);
    let include_display_fqn = wants_json && fields.as_ref().map_or(with_fqn, |f| f.display_fqn);

    let metrics = MetricsOptions {
        min_complexity,
        max_complexity,
        min_fan_in,
        min_fan_out,
    };

    match mode {
        SearchMode::Symbols => {
            let options = SearchOptions {
                db_path,
                query,
                path_filter: validated_path.as_ref(),
                kind_filter: normalized_kind.as_deref(),
                language_filter: normalized_language.as_deref(),
                limit,
                use_regex: regex,
                candidates,
                context: ContextOptions {
                    include: include_context,
                    lines: context_lines,
                    max_lines: max_context_lines,
                },
                snippet: SnippetOptions {
                    include: include_snippet,
                    max_bytes: max_snippet_bytes,
                },
                fqn: FqnOptions {
                    fqn: include_fqn,
                    canonical_fqn: include_canonical_fqn,
                    display_fqn: include_display_fqn,
                },
                include_score,
                sort_by,
                metrics,
                ast: AstOptions {
                    ast_kinds: expanded_ast_kind
                        .as_ref()
                        .map(|k| k.split(',').map(|s| s.trim().to_string()).collect())
                        .unwrap_or_default(),
                    with_ast_context,
                    _phantom: std::marker::PhantomData,
                },
                depth: DepthOptions {
                    min_depth,
                    max_depth,
                    inside,
                    contains,
                },
                algorithm: AlgorithmOptions {
                    from_symbol_set: from_symbol_set.map(|s| s.as_str()),
                    reachable_from: reachable_from.map(|s| s.as_str()),
                    dead_code_in: dead_code_in.map(|s| s.as_str()),
                    in_cycle: in_cycle.map(|s| s.as_str()),
                    slice_backward_from: slice_backward_from.map(|s| s.as_str()),
                    slice_forward_from: slice_forward_from.map(|s| s.as_str()),
                    condense,
                    paths_from: paths_from.as_ref().map(|s| s.as_str()),
                    paths_to: paths_to.as_ref().map(|s| s.as_str()),
                },
                symbol_id: symbol_id.map(|s| s.as_str()),
                fqn_pattern: fqn.map(|s| s.as_str()),
                exact_fqn: exact_fqn.map(|s| s.as_str()),
            };
            let (mut response, partial) = search_symbols(options)?;

            // Compute SCC count from results when condense was active
            let scc_count: usize = response
                .results
                .iter()
                .filter_map(|r| r.supernode_id.as_ref())
                .collect::<std::collections::HashSet<_>>()
                .len();

            // Populate notice when condense finds no SCCs
            if condense && scc_count == 0 {
                response.notice = Some(
                    "No SCCs found - codebase is acyclic (no cycles detected)".to_string(),
                );
            }

            output_symbols(cli, response, partial, scc_count)?;
        }
        SearchMode::References => {
            let options = SearchOptions {
                db_path,
                query,
                path_filter: validated_path.as_ref(),
                kind_filter: None,
                language_filter: None,
                limit,
                use_regex: regex,
                candidates,
                context: ContextOptions {
                    include: include_context,
                    lines: context_lines,
                    max_lines: max_context_lines,
                },
                snippet: SnippetOptions {
                    include: include_snippet,
                    max_bytes: max_snippet_bytes,
                },
                fqn: FqnOptions::default(),
                include_score,
                sort_by,
                metrics,
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
            };
            let (response, partial) = search_references(options)?;
            output_references(cli, response, partial)?;
        }
        SearchMode::Calls => {
            let options = SearchOptions {
                db_path,
                query,
                path_filter: validated_path.as_ref(),
                kind_filter: None,
                language_filter: None,
                limit,
                use_regex: regex,
                candidates,
                context: ContextOptions {
                    include: include_context,
                    lines: context_lines,
                    max_lines: max_context_lines,
                },
                snippet: SnippetOptions {
                    include: include_snippet,
                    max_bytes: max_snippet_bytes,
                },
                fqn: FqnOptions::default(),
                include_score,
                sort_by,
                metrics,
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
            };
            let (response, partial) = search_calls(options)?;
            output_calls(cli, response, partial)?;
        }
        SearchMode::Auto => {
            if !wants_json {
                return Err(LlmError::InvalidQuery {
                    query: "auto mode requires JSON output".to_string(),
                });
            }
            let (symbols_limit, references_limit, calls_limit) = match auto_limit {
                AutoLimitMode::PerMode => (limit, limit, limit),
                AutoLimitMode::Global => split_auto_limit(limit),
            };

            let (symbols, symbols_partial) = search_symbols(SearchOptions {
                db_path,
                query,
                path_filter: validated_path.as_ref(),
                kind_filter: normalized_kind.as_deref(),
                language_filter: normalized_language.as_deref(),
                limit: symbols_limit,
                use_regex: regex,
                candidates,
                context: ContextOptions {
                    include: include_context,
                    lines: context_lines,
                    max_lines: max_context_lines,
                },
                snippet: SnippetOptions {
                    include: include_snippet,
                    max_bytes: max_snippet_bytes,
                },
                fqn: FqnOptions {
                    fqn: include_fqn,
                    canonical_fqn: include_canonical_fqn,
                    display_fqn: include_display_fqn,
                },
                include_score,
                sort_by,
                metrics,
                ast: AstOptions {
                    ast_kinds: expanded_ast_kind
                        .as_ref()
                        .map(|k| k.split(',').map(|s| s.trim().to_string()).collect())
                        .unwrap_or_default(),
                    with_ast_context,
                    _phantom: std::marker::PhantomData,
                },
                depth: DepthOptions {
                    min_depth,
                    max_depth,
                    inside,
                    contains,
                },
                algorithm: AlgorithmOptions::default(),
                symbol_id: symbol_id.map(|s| s.as_str()),
                fqn_pattern: fqn.map(|s| s.as_str()),
                exact_fqn: exact_fqn.map(|s| s.as_str()),
            })?;
            let (references, refs_partial) = search_references(SearchOptions {
                db_path,
                query,
                path_filter: validated_path.as_ref(),
                kind_filter: None,
                language_filter: None,
                limit: references_limit,
                use_regex: regex,
                candidates,
                context: ContextOptions {
                    include: include_context,
                    lines: context_lines,
                    max_lines: max_context_lines,
                },
                snippet: SnippetOptions {
                    include: include_snippet,
                    max_bytes: max_snippet_bytes,
                },
                fqn: FqnOptions::default(),
                include_score,
                sort_by,
                metrics,
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
            })?;
            let (calls, calls_partial) = search_calls(SearchOptions {
                db_path,
                query,
                path_filter: validated_path.as_ref(),
                kind_filter: None,
                language_filter: None,
                limit: calls_limit,
                use_regex: regex,
                candidates,
                context: ContextOptions {
                    include: include_context,
                    lines: context_lines,
                    max_lines: max_context_lines,
                },
                snippet: SnippetOptions {
                    include: include_snippet,
                    max_bytes: max_snippet_bytes,
                },
                fqn: FqnOptions::default(),
                include_score,
                sort_by,
                metrics,
                ast: AstOptions::default(),
                depth: DepthOptions::default(),
                algorithm: AlgorithmOptions::default(),
                symbol_id: None,
                fqn_pattern: None,
                exact_fqn: None,
            })?;
            let total_count = symbols.total_count + references.total_count + calls.total_count;
            let combined = CombinedSearchResponse {
                query: query.to_string(),
                path_filter: validated_path
                    .as_ref()
                    .map(|p| p.to_string_lossy().to_string()),
                symbols,
                references,
                calls,
                total_count,
                limit_mode: match auto_limit {
                    AutoLimitMode::PerMode => "per-mode".to_string(),
                    AutoLimitMode::Global => "global".to_string(),
                },
            };
            let partial = symbols_partial || refs_partial || calls_partial;
            let payload = json_response_with_partial(combined, partial);
            let rendered = if matches!(cli.output, OutputFormat::Pretty) {
                serde_json::to_string_pretty(&payload)
            } else {
                serde_json::to_string(&payload)
            }?;
            println!("{}", rendered);
        }
    }

    Ok(())
}

/// Format SCC summary for human output
fn format_scc_summary(count: usize, supernode_count: usize) -> String {
    if supernode_count == 1 {
        format!("Found {} symbol in 1 SCC", count)
    } else {
        format!("Found {} symbols in {} SCCs", count, supernode_count)
    }
}

fn output_symbols(cli: &Cli, response: SearchResponse, partial: bool, scc_count: usize) -> Result<(), LlmError> {
    match cli.output {
        OutputFormat::Human => {
            if scc_count > 0 {
                println!("{}", format_scc_summary(response.total_count as usize, scc_count));
            } else if response.notice.is_some() {
                // Empty SCCs case - print warning from notice field
                eprintln!("Warning: {}", response.notice.as_ref().unwrap());
                println!("No symbols found - codebase contains no strongly connected components");
            }
            println!("{}", format_total_header(response.total_count));
            for item in response.results {
                println!(
                    "{}:{}:{} {} {} score={}",
                    item.span.file_path,
                    item.span.start_line,
                    item.span.start_col,
                    item.name,
                    item.kind,
                    item.score.unwrap_or(0)
                );
            }
            if partial {
                println!("{}", format_partial_footer());
            }
        }
        OutputFormat::Json | OutputFormat::Pretty => {
            println!("{}", render_json_response(&response, partial, cli.output)?);
        }
    }
    Ok(())
}

fn output_references(
    cli: &Cli,
    response: ReferenceSearchResponse,
    partial: bool,
) -> Result<(), LlmError> {
    match cli.output {
        OutputFormat::Human => {
            println!("{}", format_total_header(response.total_count));
            for item in response.results {
                println!(
                    "{}:{}:{} {} score={}",
                    item.span.file_path,
                    item.span.start_line,
                    item.span.start_col,
                    item.referenced_symbol,
                    item.score.unwrap_or(0)
                );
            }
            if partial {
                println!("{}", format_partial_footer());
            }
        }
        OutputFormat::Json | OutputFormat::Pretty => {
            println!("{}", render_json_response(&response, partial, cli.output)?);
        }
    }
    Ok(())
}

fn output_calls(cli: &Cli, response: CallSearchResponse, partial: bool) -> Result<(), LlmError> {
    match cli.output {
        OutputFormat::Human => {
            println!("{}", format_total_header(response.total_count));
            for item in response.results {
                println!(
                    "{}:{}:{} {} -> {} score={}",
                    item.span.file_path,
                    item.span.start_line,
                    item.span.start_col,
                    item.caller,
                    item.callee,
                    item.score.unwrap_or(0)
                );
            }
            if partial {
                println!("{}", format_partial_footer());
            }
        }
        OutputFormat::Json | OutputFormat::Pretty => {
            println!("{}", render_json_response(&response, partial, cli.output)?);
        }
    }
    Ok(())
}

#[derive(Debug, Default)]
struct FieldFlags {
    context: bool,
    snippet: bool,
    score: bool,
    fqn: bool,
    canonical_fqn: bool,
    display_fqn: bool,
}

fn parse_fields(value: &str) -> Result<FieldFlags, LlmError> {
    let mut flags = FieldFlags::default();
    let mut seen_any = false;
    for raw in value.split(',') {
        let field = raw.trim().to_lowercase();
        if field.is_empty() {
            continue;
        }
        seen_any = true;
        match field.as_str() {
            "all" => {
                flags.context = true;
                flags.snippet = true;
                flags.score = true;
                flags.fqn = true;
                flags.canonical_fqn = true;
                flags.display_fqn = true;
            }
            "context" => flags.context = true,
            "snippet" => flags.snippet = true,
            "score" => flags.score = true,
            "fqn" => flags.fqn = true,
            "canonical_fqn" => flags.canonical_fqn = true,
            "display_fqn" => flags.display_fqn = true,
            _ => {
                return Err(LlmError::InvalidField {
                    field: field.to_string(),
                })
            }
        }
    }

    if !seen_any {
        return Err(LlmError::InvalidQuery {
            query: "fields must include at least one field".to_string(),
        });
    }

    Ok(flags)
}

fn split_auto_limit(limit: usize) -> (usize, usize, usize) {
    if limit == 0 {
        return (0, 0, 0);
    }
    let base = limit / 3;
    let remainder = limit % 3;
    let symbols = base + if remainder > 0 { 1 } else { 0 };
    let references = base + if remainder > 1 { 1 } else { 0 };
    let calls = base;
    (symbols, references, calls)
}

/// Normalize language input to standard label names
///
/// Accepts common language names (case-insensitive) and maps to
/// Magellan's standard label names.
fn normalize_language(lang: &str) -> String {
    match lang.to_lowercase().as_str() {
        "rust" | "rs" => "rust".to_string(),
        "python" | "py" => "python".to_string(),
        "javascript" | "js" | "ecmascript" => "javascript".to_string(),
        "typescript" | "ts" => "typescript".to_string(),
        "c" => "c".to_string(),
        "c++" | "cpp" | "cxx" | "cc" => "cpp".to_string(),
        "java" => "java".to_string(),
        "go" | "golang" => "go".to_string(),
        other => other.to_string(), // Pass through unknown values for future compatibility
    }
}

/// Parse comma-separated kind values and normalize them
///
/// Converts "Function,Struct" to ["function", "struct"]
fn parse_kinds(kind: &str) -> Vec<String> {
    kind.split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .map(|k| normalize_kind(&k))
        .collect()
}

/// Normalize kind value to standard label name
///
/// Maps common variants to standard names: "function" -> "fn", etc.
fn normalize_kind(kind: &str) -> String {
    match kind.to_lowercase().as_str() {
        "function" | "fn" | "func" => "fn".to_string(),
        "method" => "method".to_string(),
        "struct" | "class" => "struct".to_string(), // Map class to struct for now
        "enum" | "enumeration" => "enum".to_string(),
        "interface" => "interface".to_string(),
        "module" | "namespace" | "package" => "module".to_string(),
        "union" => "union".to_string(),
        "typealias" | "type" | "alias" => "typealias".to_string(),
        "constant" | "const" => "constant".to_string(),
        "variable" | "var" | "field" => "variable".to_string(),
        other => other.to_string(),
    }
}

fn emit_error(cli: &Cli, err: &LlmError) {
    match cli.output {
        OutputFormat::Human => {
            eprintln!("ERROR [{}]: {}", err.error_code(), err);
            if let Some(hint) = err.remediation() {
                eprintln!("Hint: {}", hint);
            }
        }
        OutputFormat::Json | OutputFormat::Pretty => {
            let error = ErrorResponse {
                code: err.error_code().to_string(),
                error: err.severity().to_string(),
                message: err.to_string(),
                span: None,
                remediation: err.remediation().map(|s| s.to_string()),
            };
            let response = json_response(error);
            let result = if matches!(cli.output, OutputFormat::Pretty) {
                serde_json::to_string_pretty(&response)
            } else {
                serde_json::to_string(&response)
            };
            match result {
                Ok(payload) => println!("{}", payload),
                Err(ser_err) => eprintln!("ERROR: {}", ser_err),
            }
        }
    }
}

#[cfg(test)]
mod cli_tests {
    use super::*;

    // Helper function to create a temp db file for testing
    fn create_temp_db() -> std::io::Result<PathBuf> {
        let temp_file =
            std::env::temp_dir().join(format!("llmgrep_test_{}.db", std::process::id()));
        std::fs::File::create(&temp_file)?;
        Ok(temp_file)
    }

    #[test]
    fn test_basic_search_command() {
        let temp_db = create_temp_db().expect("Failed to create temp db");
        let args = [
            "llmgrep",
            "--db",
            temp_db.to_str().unwrap(),
            "search",
            "--query",
            "test",
        ];
        let result = Cli::try_parse_from(args);
        assert!(result.is_ok(), "Should parse basic search command");
        let cli = result.unwrap();
        assert_eq!(
            cli.db.as_ref().unwrap().to_str().unwrap(),
            temp_db.to_str().unwrap()
        );
        match cli.command {
            Command::Search { query, .. } => {
                assert_eq!(query, "test");
            }
        }
    }

    #[test]
    fn test_invalid_flag() {
        let args = ["llmgrep", "--invalid-flag", "search", "--query", "test"];
        let result = Cli::try_parse_from(args);
        assert!(result.is_err(), "Should reject unknown flag");
    }

    #[test]
    fn test_limit_validation_zero() {
        let temp_db = create_temp_db().expect("Failed to create temp db");
        let args = [
            "llmgrep",
            "--db",
            temp_db.to_str().unwrap(),
            "search",
            "--query",
            "test",
            "--limit",
            "0",
        ];
        let result = Cli::try_parse_from(args);
        assert!(result.is_err(), "Should reject limit=0 (range is 1..=1000)");
    }

    #[test]
    fn test_limit_valid() {
        let temp_db = create_temp_db().expect("Failed to create temp db");
        let args = [
            "llmgrep",
            "--db",
            temp_db.to_str().unwrap(),
            "search",
            "--query",
            "test",
            "--limit",
            "500",
        ];
        let result = Cli::try_parse_from(args);
        assert!(result.is_ok(), "Should accept valid limit");
        let cli = result.unwrap();
        match cli.command {
            Command::Search { limit, .. } => {
                assert_eq!(limit, 500);
            }
        }
    }

    #[test]
    fn test_regex_mode() {
        let temp_db = create_temp_db().expect("Failed to create temp db");
        let args = [
            "llmgrep",
            "--db",
            temp_db.to_str().unwrap(),
            "search",
            "--query",
            "test.*",
            "--regex",
        ];
        let result = Cli::try_parse_from(args);
        assert!(result.is_ok(), "Should parse regex flag");
        let cli = result.unwrap();
        match cli.command {
            Command::Search { regex, .. } => {
                assert!(regex, "Regex flag should be set");
            }
        }
    }

    #[test]
    fn test_field_parsing() {
        let temp_db = create_temp_db().expect("Failed to create temp db");
        let args = [
            "llmgrep",
            "--db",
            temp_db.to_str().unwrap(),
            "search",
            "--query",
            "test",
            "--fields",
            "context,snippet,score",
        ];
        let result = Cli::try_parse_from(args);
        assert!(result.is_ok(), "Should parse fields");
        let cli = result.unwrap();
        match cli.command {
            Command::Search { fields, .. } => {
                assert_eq!(fields.as_ref().unwrap(), "context,snippet,score");
            }
        }
    }

    #[test]
    fn test_search_mode_symbols() {
        let temp_db = create_temp_db().expect("Failed to create temp db");
        let args = [
            "llmgrep",
            "--db",
            temp_db.to_str().unwrap(),
            "search",
            "--query",
            "test",
            "--mode",
            "symbols",
        ];
        let result = Cli::try_parse_from(args);
        assert!(result.is_ok(), "Should parse symbols mode");
        let cli = result.unwrap();
        match cli.command {
            Command::Search { mode, .. } => {
                assert!(matches!(mode, SearchMode::Symbols));
            }
        }
    }

    #[test]
    fn test_search_mode_references() {
        let temp_db = create_temp_db().expect("Failed to create temp db");
        let args = [
            "llmgrep",
            "--db",
            temp_db.to_str().unwrap(),
            "search",
            "--query",
            "test",
            "--mode",
            "references",
        ];
        let result = Cli::try_parse_from(args);
        assert!(result.is_ok(), "Should parse references mode");
        let cli = result.unwrap();
        match cli.command {
            Command::Search { mode, .. } => {
                assert!(matches!(mode, SearchMode::References));
            }
        }
    }

    #[test]
    fn test_search_mode_calls() {
        let temp_db = create_temp_db().expect("Failed to create temp db");
        let args = [
            "llmgrep",
            "--db",
            temp_db.to_str().unwrap(),
            "search",
            "--query",
            "test",
            "--mode",
            "calls",
        ];
        let result = Cli::try_parse_from(args);
        assert!(result.is_ok(), "Should parse calls mode");
        let cli = result.unwrap();
        match cli.command {
            Command::Search { mode, .. } => {
                assert!(matches!(mode, SearchMode::Calls));
            }
        }
    }

    #[test]
    fn test_search_mode_auto() {
        let temp_db = create_temp_db().expect("Failed to create temp db");
        let args = [
            "llmgrep",
            "--db",
            temp_db.to_str().unwrap(),
            "search",
            "--query",
            "test",
            "--mode",
            "auto",
        ];
        let result = Cli::try_parse_from(args);
        assert!(result.is_ok(), "Should parse auto mode");
        let cli = result.unwrap();
        match cli.command {
            Command::Search { mode, .. } => {
                assert!(matches!(mode, SearchMode::Auto));
            }
        }
    }

    #[test]
    fn test_empty_query_accepted_by_clap() {
        let temp_db = create_temp_db().expect("Failed to create temp db");
        let args = [
            "llmgrep",
            "--db",
            temp_db.to_str().unwrap(),
            "search",
            "--query",
            "",
        ];
        let result = Cli::try_parse_from(args);
        // Clap should accept empty string (runtime validation happens later)
        assert!(result.is_ok(), "Clap should accept empty query string");
        let cli = result.unwrap();
        match cli.command {
            Command::Search { query, .. } => {
                assert_eq!(query, "");
            }
        }
    }

    #[test]
    fn test_output_format_human() {
        let args = ["llmgrep", "--output", "human", "search", "--query", "test"];
        let result = Cli::try_parse_from(args);
        assert!(result.is_ok(), "Should parse human output format");
        let cli = result.unwrap();
        assert!(matches!(cli.output, OutputFormat::Human));
    }

    #[test]
    fn test_output_format_json() {
        let args = ["llmgrep", "--output", "json", "search", "--query", "test"];
        let result = Cli::try_parse_from(args);
        assert!(result.is_ok(), "Should parse json output format");
        let cli = result.unwrap();
        assert!(matches!(cli.output, OutputFormat::Json));
    }

    #[test]
    fn test_output_format_pretty() {
        let args = ["llmgrep", "--output", "pretty", "search", "--query", "test"];
        let result = Cli::try_parse_from(args);
        assert!(result.is_ok(), "Should parse pretty output format");
        let cli = result.unwrap();
        assert!(matches!(cli.output, OutputFormat::Pretty));
    }

    #[test]
    fn test_candidates_validation_min() {
        let temp_db = create_temp_db().expect("Failed to create temp db");
        let args = [
            "llmgrep",
            "--db",
            temp_db.to_str().unwrap(),
            "search",
            "--query",
            "test",
            "--candidates",
            "0",
        ];
        let result = Cli::try_parse_from(args);
        assert!(
            result.is_err(),
            "Should reject candidates=0 (range is 1..=10000)"
        );
    }

    #[test]
    fn test_candidates_validation_max() {
        let temp_db = create_temp_db().expect("Failed to create temp db");
        let args = [
            "llmgrep",
            "--db",
            temp_db.to_str().unwrap(),
            "search",
            "--query",
            "test",
            "--candidates",
            "10001",
        ];
        let result = Cli::try_parse_from(args);
        assert!(
            result.is_err(),
            "Should reject candidates=10001 (range is 1..=10000)"
        );
    }

    #[test]
    fn test_max_snippet_bytes_validation_min() {
        let temp_db = create_temp_db().expect("Failed to create temp db");
        let args = [
            "llmgrep",
            "--db",
            temp_db.to_str().unwrap(),
            "search",
            "--query",
            "test",
            "--max-snippet-bytes",
            "0",
        ];
        let result = Cli::try_parse_from(args);
        assert!(
            result.is_err(),
            "Should reject max_snippet_bytes=0 (range is 1..=1MB)"
        );
    }

    #[test]
    fn test_context_lines_validation_min() {
        let temp_db = create_temp_db().expect("Failed to create temp db");
        let args = [
            "llmgrep",
            "--db",
            temp_db.to_str().unwrap(),
            "search",
            "--query",
            "test",
            "--context-lines",
            "0",
        ];
        let result = Cli::try_parse_from(args);
        assert!(
            result.is_err(),
            "Should reject context_lines=0 (range is 1..=100)"
        );
    }

    #[test]
    fn test_context_lines_validation_max() {
        let temp_db = create_temp_db().expect("Failed to create temp db");
        let args = [
            "llmgrep",
            "--db",
            temp_db.to_str().unwrap(),
            "search",
            "--query",
            "test",
            "--context-lines",
            "101",
        ];
        let result = Cli::try_parse_from(args);
        assert!(
            result.is_err(),
            "Should reject context_lines=101 (range is 1..=100)"
        );
    }

    #[test]
    fn test_max_context_lines_validation_min() {
        let temp_db = create_temp_db().expect("Failed to create temp db");
        let args = [
            "llmgrep",
            "--db",
            temp_db.to_str().unwrap(),
            "search",
            "--query",
            "test",
            "--max-context-lines",
            "0",
        ];
        let result = Cli::try_parse_from(args);
        assert!(
            result.is_err(),
            "Should reject max_context_lines=0 (range is 1..=500)"
        );
    }

    #[test]
    fn test_max_context_lines_validation_max() {
        let temp_db = create_temp_db().expect("Failed to create temp db");
        let args = [
            "llmgrep",
            "--db",
            temp_db.to_str().unwrap(),
            "search",
            "--query",
            "test",
            "--max-context-lines",
            "501",
        ];
        let result = Cli::try_parse_from(args);
        assert!(
            result.is_err(),
            "Should reject max_context_lines=501 (range is 1..=500)"
        );
    }

    #[test]
    fn test_path_validation_sensitive_etc() {
        let path = Path::new("/etc/passwd");
        let result = validate_path(path, true);
        assert!(result.is_err(), "Should reject /etc/passwd");
        match result {
            Err(LlmError::PathValidationFailed { reason, .. }) => {
                assert!(
                    reason.contains("not allowed"),
                    "Error should mention access denied"
                );
            }
            _ => panic!("Expected PathValidationFailed error"),
        }
    }

    #[test]
    fn test_path_validation_var_tmp() {
        let path = Path::new("/var/tmp/test");
        let result = validate_path(path, false);
        assert!(result.is_err(), "Should reject /var/tmp/test");
        match result {
            Err(LlmError::PathValidationFailed { .. }) => {
                // Expected - /var/tmp is blocked
            }
            _ => panic!("Expected PathValidationFailed error for /var/tmp"),
        }
    }

    #[test]
    fn test_path_validation_allowed_path() {
        let temp_db = create_temp_db().expect("Failed to create temp db");
        let result = validate_path(&temp_db, true);
        assert!(result.is_ok(), "Should allow temp db path");
        let canonical = result.unwrap();
        assert!(canonical.exists(), "Validated path should exist");
    }
}
