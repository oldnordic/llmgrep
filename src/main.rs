use clap::builder::{RangedI64ValueParser, TypedValueParser};
use clap::{Parser, Subcommand, ValueEnum};
use llmgrep::algorithm::AlgorithmOptions;
use llmgrep::error::LlmError;
use llmgrep::output::{
    json_response, json_response_with_partial_and_performance, CallSearchResponse,
    CombinedSearchResponse, ErrorResponse, OutputFormat, PerformanceMetrics,
    ReferenceSearchResponse, SearchResponse,
};
use llmgrep::output_common::{format_partial_footer, format_total_header};
use llmgrep::backend::Backend;
use llmgrep::query::{
    AstOptions, ContextOptions, DepthOptions, FqnOptions, MetricsOptions, SearchOptions,
    SnippetOptions,
};

/// Search parameters bundled into a single struct.
/// 
/// This eliminates the 42-parameter explosion in run_search() and makes
/// the code easier to maintain, test, and call.
#[derive(Debug)]
struct SearchParams {
    query: String,
    mode: SearchMode,
    path: Option<PathBuf>,
    kind: Option<String>,
    language: Option<String>,
    label: Option<String>,
    limit: usize,
    regex: bool,
    candidates: usize,
    with_context: bool,
    context_lines: usize,
    max_context_lines: usize,
    with_snippet: bool,
    with_fqn: bool,
    max_snippet_bytes: usize,
    fields: Option<String>,
    sort_by: SortMode,
    auto_limit: AutoLimitMode,
    min_complexity: Option<usize>,
    max_complexity: Option<usize>,
    min_fan_in: Option<usize>,
    min_fan_out: Option<usize>,
    symbol_id: Option<String>,
    fqn: Option<String>,
    exact_fqn: Option<String>,
    ast_kind: Option<String>,
    with_ast_context: bool,
    min_depth: Option<usize>,
    max_depth: Option<usize>,
    inside: Option<String>,
    contains: Option<String>,
    from_symbol_set: Option<String>,
    reachable_from: Option<String>,
    dead_code_in: Option<String>,
    in_cycle: Option<String>,
    slice_backward_from: Option<String>,
    slice_forward_from: Option<String>,
    condense: bool,
    paths_from: Option<String>,
    paths_to: Option<String>,
}

impl SearchParams {
    /// Extract search parameters from the Command::Search variant.
    /// 
    /// Returns None if the command is not a Search variant.
    fn from_command(cmd: &Command) -> Option<Self> {
        match cmd {
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
            } => Some(Self {
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
            }),
            _ => None,
        }
    }
}
use llmgrep::ast::{expand_shorthand_with_language, expand_shorthands};
use llmgrep::SortMode;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

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

    #[arg(long, global = true)]
    show_metrics: bool,

    #[arg(long, global = true, default_value = "false")]
    detect_backend: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)] // Command variants have different sizes but that's acceptable for CLI
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

        #[arg(long, alias = "purpose")]
        label: Option<String>,

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

    #[command(after_help = AST_EXAMPLES)]
    Ast {
        #[arg(long)]
        file: PathBuf,

        #[arg(long)]
        position: Option<usize>,

        #[arg(long, default_value_t = 10000, value_parser = ranged_usize(1, 100000))]
        limit: usize,
    },

    #[command(after_help = FIND_AST_EXAMPLES)]
    FindAst {
        #[arg(long)]
        kind: String,
    },

    #[command(after_help = COMPLETE_EXAMPLES)]
    Complete {
        #[arg(long)]
        prefix: String,

        #[arg(long, default_value_t = 50, value_parser = ranged_usize(1, 1000))]
        limit: usize,
    },

    #[command(after_help = LOOKUP_EXAMPLES)]
    Lookup {
        #[arg(long)]
        fqn: String,
    },

    // Watch command is incomplete and requires unstable Mag APIs
    // Enable with: --features unstable-watch
    #[cfg(feature = "unstable-watch")]
    #[command(after_help = WATCH_EXAMPLES)]
    Watch {
        #[arg(long)]
        query: String,

        #[arg(long, value_enum, default_value = "symbols")]
        mode: SearchMode,

        #[arg(long)]
        path: Option<PathBuf>,

        #[arg(long)]
        kind: Option<String>,

        #[arg(long, default_value_t = 50, value_parser = ranged_usize(1, 1000))]
        limit: usize,

        #[arg(long)]
        regex: bool,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum SearchMode {
    Symbols,
    References,
    Calls,
    Auto,
    Labels,
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

V2.1 PATHS FEATURES:
  # Find all symbols on execution paths from main
  llmgrep --db code.db search --paths-from main --query ".*"

  # Find symbols on paths between two functions
  llmgrep --db code.db search --paths-from parse --paths-to execute --output json

  # Combine path filtering with query
  llmgrep --db code.db search --paths-from main --query "error" --output json

  # Path filtering with other filters
  llmgrep --db code.db search --paths-from main --kind Function --output json
"#;

const AST_EXAMPLES: &str = r#"
EXAMPLES:
  # Get full AST tree for a file
  llmgrep --db code.db ast --file src/main.rs

  # Get AST node at specific byte offset
  llmgrep --db code.db ast --file src/main.rs --position 3000

  # Limit output for large files
  llmgrep --db code.db ast --file src/lib.rs --limit 100

  # Pretty-print AST structure
  llmgrep --db code.db ast --file src/main.rs --output pretty
"#;

const FIND_AST_EXAMPLES: &str = r#"
EXAMPLES:
  # Find all function definitions
  llmgrep --db code.db find-ast --kind function_item

  # Find all if expressions
  llmgrep --db code.db find-ast --kind if_expression

  # Find all loops as pretty JSON
  llmgrep --db code.db find-ast --kind for_expression --output pretty

  # Common node kinds:
    function_item, struct_item, enum_item, impl_item
    if_expression, while_expression, for_expression, match_expression
    block, call_expression, let_declaration
"#;

const COMPLETE_EXAMPLES: &str = r#"
EXAMPLES:
  # Complete symbol names starting with prefix
  llmgrep --db code.db complete --prefix "llmgrep::"

  # Complete with custom limit
  llmgrep --db code.db complete --prefix "std::collections" --limit 20

  # JSON output for programmatic use
  llmgrep --db code.db complete --prefix "parse" --output json

  # Use for interactive autocomplete
  llmgrep --db code.db complete --prefix "crate::backend::" --limit 10
"#;

const LOOKUP_EXAMPLES: &str = r#"
EXAMPLES:
  # Lookup symbol by exact FQN
  llmgrep --db code.db lookup --fqn "llmgrep::main"

  # Lookup with full module path
  llmgrep --db code.db lookup --fqn "std::collections::HashMap::new"

  # JSON output for programmatic use
  llmgrep --db code.v3 lookup --fqn "crate::backend::NativeV3Backend" --output json

  # Get all symbol metadata in one query
  llmgrep --db code.db lookup --fqn "parse" --output pretty
"#;

#[cfg(feature = "unstable-watch")]
const WATCH_EXAMPLES: &str = r#"
EXAMPLES:
  # Watch for symbols matching "Widget"
  llmgrep --db code.db watch --query "Widget"

  # Watch with path filter
  llmgrep --db code.db watch --query "parse" --path src/

  # Watch for references
  llmgrep --db code.db watch --query "Token" --mode references

  # Watch with regex pattern
  llmgrep --db code.db watch --query "^test_" --regex
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
    // Check platform and warn about limitations
    llmgrep::platform::check_platform_support();

    let cli = Cli::parse();
    if let Err(err) = dispatch(&cli) {
        emit_error(&cli, &err);
        std::process::exit(1);
    }
}

fn dispatch(cli: &Cli) -> Result<(), LlmError> {
    // Handle --detect-backend flag before command dispatch
    if cli.detect_backend {
        let db_path = cli.db.as_ref().ok_or(LlmError::DatabaseNotFound {
            path: "none".to_string(),
        })?;
        let validated_db = validate_path(db_path, true)?;

        use magellan::migrate_backend_cmd::{detect_backend_format, BackendFormat};
        let format = detect_backend_format(&validated_db).map_err(|e| {
            LlmError::BackendDetectionFailed {
                path: validated_db.display().to_string(),
                reason: e.to_string(),
            }
        })?;

        // Check file extension for V3 backend detection
        let path_str = validated_db.to_string_lossy();
        let is_v3 = path_str.ends_with(".v3");
        
        let backend_str = match format {
            BackendFormat::Sqlite if is_v3 => "native-v3",
            BackendFormat::Sqlite => "sqlite",
        };

        if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
            use serde_json::json;
            let output = json!({
                "backend": backend_str,
                "database": validated_db.to_string_lossy(),
            });
            println!("{}", serde_json::to_string(&output)?);
        } else {
            println!("{}", backend_str);
        }
        return Ok(());
    }

    match &cli.command {
        None => {
            // No subcommand provided and --detect-backend not set
            Err(LlmError::InvalidQuery {
                query: "No subcommand provided. Use --help for usage information.".to_string(),
            })
        }
        Some(cmd) => match cmd {
        Command::Ast {
            file,
            position,
            limit,
        } => run_ast(cli, file, *position, *limit),

        Command::FindAst { kind } => run_find_ast(cli, kind),

        Command::Complete { prefix, limit } => run_complete(cli, prefix.clone(), *limit),

        Command::Lookup { fqn } => run_lookup(cli, fqn),

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
        } => {
            let params = SearchParams {
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
            };
            run_search(cli, &params)
        }

        #[cfg(feature = "unstable-watch")]
        Command::Watch {
            query,
            mode,
            path,
            kind,
            limit,
            regex,
        } => run_watch(cli, query, *mode, path, kind, *limit, *regex),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn run_search(cli: &Cli, params: &SearchParams) -> Result<(), LlmError> {
    // Validate SymbolId format (32 hex characters)
    if let Some(sid) = &params.symbol_id {
        let hex_regex =
            regex::Regex::new(r"^[0-9a-f]{32}$").map_err(|_| LlmError::InvalidQuery {
                query: "Failed to compile symbol_id validation regex".to_string(),
            })?;
        if !hex_regex.is_match(&sid) {
            return Err(LlmError::InvalidQuery {
                query: format!(
                    "Invalid symbol_id format: '{}'. Expected 32 hex characters (0-9, a-f).",
                    sid
                ),
            });
        }
    }

    // Normalize and validate language filter
    let normalized_language = params.language.as_ref().map(|lang| normalize_language(lang));

    // Expand AST shorthands with language-aware expansion
    let expanded_ast_kind = if let Some(kind_input) = &params.ast_kind {
        // Use language-aware expansion if language is specified
        let kinds = if normalized_language.is_some() {
            expand_shorthand_with_language(
                &kind_input,
                normalized_language.as_deref(),
            )
        } else {
            // Use Rust shorthands by default
            expand_shorthands(&kind_input)
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
    let normalized_kind = params.kind.as_ref().map(|k| {
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
    if params.fqn.is_some() && params.exact_fqn.is_some() {
        return Err(LlmError::InvalidQuery {
            query: "--fqn and --exact-fqn are mutually exclusive. Use only one.".to_string(),
        });
    }

    // Warn if both --query and --symbol-id are provided (symbol_id takes precedence)
    if params.symbol_id.is_some() {
        eprintln!(
            "Note: --symbol-id provided, using direct lookup. Query '{}' will be used as secondary filter if needed.",
            params.query
        );
    }

    if params.query.trim().is_empty() && params.symbol_id.is_none() && !params.condense && params.paths_from.is_none() {
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

    // Start total timing
    let total_start = std::time::Instant::now();

    // Detect and open backend automatically (SQLite or Native-V2)
    let detect_start = std::time::Instant::now();
    let backend = Backend::detect_and_open(db_path)?;
    let backend_detection_ms = detect_start.elapsed().as_millis() as u64;

    // Validate path filter if provided
    let validated_path = if let Some(p) = &params.path {
        Some(validate_path(p, false)?)
    } else {
        None
    };
    let wants_json = matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty);
    let candidates = params.candidates.max(params.limit);
    let fields = if wants_json {
        params.fields.as_ref().map(|value| parse_fields(value)).transpose()?
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
    let include_canonical_fqn = wants_json && fields.as_ref().map_or(params.with_fqn, |f| f.canonical_fqn);
    let include_display_fqn = wants_json && fields.as_ref().map_or(params.with_fqn, |f| f.display_fqn);

    let metrics = MetricsOptions {
        min_complexity: params.min_complexity,
        max_complexity: params.max_complexity,
        min_fan_in: params.min_fan_in,
        min_fan_out: params.min_fan_out,
    };

    match params.mode {
        SearchMode::Symbols => {
            let options = SearchOptions {
                db_path,
                query: &params.query,
                path_filter: validated_path.as_ref(),
                kind_filter: normalized_kind.as_deref(),
                language_filter: normalized_language.as_deref(),
                limit: params.limit,
                use_regex: params.regex,
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
                    from_symbol_set: params.from_symbol_set.as_ref().map(|s| s.as_str()),
                    reachable_from: params.reachable_from.as_ref().map(|s| s.as_str()),
                    dead_code_in: params.dead_code_in.as_ref().map(|s| s.as_str()),
                    in_cycle: params.in_cycle.as_ref().map(|s| s.as_str()),
                    slice_backward_from: params.slice_backward_from.as_ref().map(|s| s.as_str()),
                    slice_forward_from: params.slice_forward_from.as_ref().map(|s| s.as_str()),
                    condense: params.condense,
                    paths_from: params.paths_from.as_ref().map(|s| s.as_str()),
                    paths_to: params.paths_to.as_ref().map(|s| s.as_str()),
                },
                symbol_id: params.symbol_id.as_ref().map(|s| s.as_str()),
                fqn_pattern: params.fqn.as_ref().map(|s| s.as_str()),
                exact_fqn: params.exact_fqn.as_ref().map(|s| s.as_str()),
            };

            // Time query execution
            let query_start = std::time::Instant::now();
            let (mut response, partial, paths_bounded) = backend.search_symbols(options)?;
            let query_execution_ms = query_start.elapsed().as_millis() as u64;

            // Compute SCC count from results when condense was active
            let scc_count: usize = response
                .results
                .iter()
                .filter_map(|r| r.supernode_id.as_ref())
                .collect::<std::collections::HashSet<_>>()
                .len();

            // Populate notice when condense finds no SCCs
            if params.condense && scc_count == 0 {
                response.notice = Some(
                    "No SCCs found - codebase is acyclic (no cycles detected)".to_string(),
                );
            }

            // Warn if path enumeration hit bounds
            if paths_bounded {
                eprintln!("Warning: Path enumeration hit bounds (max-depth=100, max-paths=1000)");
                eprintln!("         Results may be incomplete. Use magellan paths directly with adjusted bounds for full enumeration.");
            }

            // Note: Empty paths case
            if response.total_count == 0 {
                if let Some(from) = &params.paths_from {
                    eprintln!("Note: No execution paths found from '{from}'");
                    if let Some(to) = &params.paths_to {
                        eprintln!("      to '{to}'. Symbols may be unreachable.");
                    }
                }
            }

            // Time output formatting
            let format_start = std::time::Instant::now();
            let metrics = if cli.show_metrics {
                Some(PerformanceMetrics {
                    backend_detection_ms,
                    query_execution_ms,
                    output_formatting_ms: 0, // Will update after formatting
                    total_ms: 0, // Will update after formatting
                })
            } else {
                None
            };

            output_symbols(cli, response, partial, scc_count, metrics.as_ref())?;

            let output_formatting_ms = format_start.elapsed().as_millis() as u64;
            let total_ms = total_start.elapsed().as_millis() as u64;

            // Print metrics to stderr for human output
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
                db_path,
                query: &params.query,
                path_filter: validated_path.as_ref(),
                kind_filter: None,
                language_filter: None,
                limit: params.limit,
                use_regex: params.regex,
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
            };

            // Time query execution
            let query_start = std::time::Instant::now();
            let (response, partial) = backend.search_references(options)?;
            let query_execution_ms = query_start.elapsed().as_millis() as u64;

            // Time output formatting
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

            // Print metrics to stderr for human output
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
                db_path,
                query: &params.query,
                path_filter: validated_path.as_ref(),
                kind_filter: None,
                language_filter: None,
                limit: params.limit,
                use_regex: params.regex,
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
            };

            // Time query execution
            let query_start = std::time::Instant::now();
            let (response, partial) = backend.search_calls(options)?;
            let query_execution_ms = query_start.elapsed().as_millis() as u64;

            // Time output formatting
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

            // Print metrics to stderr for human output
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
                db_path,
                query: &params.query,
                path_filter: validated_path.as_ref(),
                kind_filter: normalized_kind.as_deref(),
                language_filter: normalized_language.as_deref(),
                limit: symbols_limit,
                use_regex: params.regex,
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
                symbol_id: params.symbol_id.as_ref().map(|s| s.as_str()),
                fqn_pattern: params.fqn.as_ref().map(|s| s.as_str()),
                exact_fqn: params.exact_fqn.as_ref().map(|s| s.as_str()),
            })?;
            let (references, refs_partial) = backend.search_references(SearchOptions {
                db_path,
                query: &params.query,
                path_filter: validated_path.as_ref(),
                kind_filter: None,
                language_filter: None,
                limit: references_limit,
                use_regex: params.regex,
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
            })?;
            let (calls, calls_partial) = backend.search_calls(SearchOptions {
                db_path,
                query: &params.query,
                path_filter: validated_path.as_ref(),
                kind_filter: None,
                language_filter: None,
                limit: calls_limit,
                use_regex: params.regex,
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

            let query_execution_ms = total_start.elapsed().as_millis() as u64 - backend_detection_ms;

            // Time output formatting
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

            // Print metrics to stderr for human output
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

            // Time query execution
            let query_start = std::time::Instant::now();
            let db_path_str = db_path.to_str()
                .ok_or_else(|| LlmError::SearchFailed {
                    reason: format!("Database path {:?} is not valid UTF-8", db_path),
                })?;
            let (response, partial, _paths_bounded) = backend.search_by_label(&label_name, params.limit, db_path_str)?;
            let query_execution_ms = query_start.elapsed().as_millis() as u64;

            // Time output formatting
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

            // Print metrics to stderr for human output
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

fn run_ast(
    cli: &Cli,
    file: &Path,
    position: Option<usize>,
    limit: usize,
) -> Result<(), LlmError> {
    // Validate database path
    let db_path = if let Some(db_path) = &cli.db {
        validate_path(db_path, true)?
    } else {
        return Err(LlmError::DatabaseNotFound {
            path: "none".to_string(),
        });
    };

    // Validate file path
    let validated_file = validate_path(file, false)?;

    // Check if file exists
    if !validated_file.exists() {
        return Err(LlmError::PathValidationFailed {
            path: file.display().to_string(),
            reason: "File does not exist".to_string(),
        });
    }

    // Start total timing
    let total_start = std::time::Instant::now();

    // Detect and open backend automatically (SQLite or Native-V2)
    let detect_start = std::time::Instant::now();
    let backend = Backend::detect_and_open(&db_path)?;
    let backend_detection_ms = detect_start.elapsed().as_millis() as u64;

    // Query AST via backend
    let query_start = std::time::Instant::now();
    let json_value = backend.ast(&validated_file, position, limit)?;
    let query_execution_ms = query_start.elapsed().as_millis() as u64;

    // Check for truncation warning (file mode only, not position mode)
    if position.is_none() {
        if let Some(data) = json_value.get("data") {
            if let Some(count) = data.get("count").and_then(|c| c.as_u64()) {
                if count > limit as u64 {
                    eprintln!(
                        "Warning: AST output truncated to {} nodes (total: {})",
                        limit, count
                    );
                    eprintln!(
                        "         Use --limit {} to see all nodes.",
                        count
                    );
                }
            }
        }
    }

    // Render output based on format
    let format_start = std::time::Instant::now();
    let rendered = if matches!(cli.output, OutputFormat::Pretty) {
        serde_json::to_string_pretty(&json_value)?
    } else {
        serde_json::to_string(&json_value)?
    };
    let output_formatting_ms = format_start.elapsed().as_millis() as u64;
    let total_ms = total_start.elapsed().as_millis() as u64;

    println!("{}", rendered);

    // Print metrics to stderr if requested
    if cli.show_metrics {
        eprintln!("Performance metrics:");
        eprintln!("  Backend detection: {}ms", backend_detection_ms);
        eprintln!("  Query execution: {}ms", query_execution_ms);
        eprintln!("  Output formatting: {}ms", output_formatting_ms);
        eprintln!("  Total: {}ms", total_ms);
    }

    Ok(())
}

fn run_find_ast(
    cli: &Cli,
    kind: &str,
) -> Result<(), LlmError> {
    // Validate database path
    let db_path = if let Some(db_path) = &cli.db {
        validate_path(db_path, true)?
    } else {
        return Err(LlmError::DatabaseNotFound {
            path: "none".to_string(),
        });
    };

    // Validate kind is not empty
    if kind.trim().is_empty() {
        return Err(LlmError::InvalidQuery {
            query: "--kind cannot be empty".to_string(),
        });
    }

    // Start total timing
    let total_start = std::time::Instant::now();

    // Detect and open backend automatically (SQLite or Native-V2)
    let detect_start = std::time::Instant::now();
    let backend = Backend::detect_and_open(&db_path)?;
    let backend_detection_ms = detect_start.elapsed().as_millis() as u64;

    // Query AST nodes by kind via backend
    let query_start = std::time::Instant::now();
    let json_value = backend.find_ast(kind)?;
    let query_execution_ms = query_start.elapsed().as_millis() as u64;

    // Check for empty nodes array (not an error, just no results)
    let nodes = if json_value["data"]["nodes"].is_array() {
        json_value["data"]["nodes"].as_array()
    } else {
        json_value["nodes"].as_array()
    };

    if let Some(node_array) = nodes {
        if node_array.is_empty() {
            eprintln!("No AST nodes found with kind '{}'", kind);
            eprintln!("Hint: Check available kinds with: magellan label --list");
            return Ok(());
        }
    }

    // Render output based on format
    let format_start = std::time::Instant::now();
    let rendered = if matches!(cli.output, OutputFormat::Pretty) {
        serde_json::to_string_pretty(&json_value)?
    } else {
        serde_json::to_string(&json_value)?
    };
    let output_formatting_ms = format_start.elapsed().as_millis() as u64;
    let total_ms = total_start.elapsed().as_millis() as u64;

    println!("{}", rendered);

    // Print metrics to stderr if requested
    if cli.show_metrics {
        eprintln!("Performance metrics:");
        eprintln!("  Backend detection: {}ms", backend_detection_ms);
        eprintln!("  Query execution: {}ms", query_execution_ms);
        eprintln!("  Output formatting: {}ms", output_formatting_ms);
        eprintln!("  Total: {}ms", total_ms);
    }

    Ok(())
}

fn run_complete(
    cli: &Cli,
    prefix: String,
    limit: usize,
) -> Result<(), LlmError> {
    // Validate database path
    let db_path = if let Some(db_path) = &cli.db {
        validate_path(db_path, true)?
    } else {
        return Err(LlmError::DatabaseNotFound {
            path: "none".to_string(),
        });
    };

    // Validate prefix is not empty
    if prefix.trim().is_empty() {
        return Err(LlmError::InvalidQuery {
            query: "--prefix cannot be empty".to_string(),
        });
    }

    // Start total timing
    let total_start = std::time::Instant::now();

    // Detect and open backend automatically (SQLite or Native-V2)
    let detect_start = std::time::Instant::now();
    let backend = Backend::detect_and_open(&db_path)?;
    let backend_detection_ms = detect_start.elapsed().as_millis() as u64;

    // Check if backend supports complete command (native-v3 only)
    require_native_v3(&backend, "complete", &db_path)?;

    // Get completions via backend
    let query_start = std::time::Instant::now();
    let completions = backend.complete(&prefix, limit)?;
    let query_execution_ms = query_start.elapsed().as_millis() as u64;

    // Render output based on format
    let format_start = std::time::Instant::now();
    match cli.output {
        OutputFormat::Human => {
            for completion in &completions {
                println!("{}", completion);
            }
        }
        OutputFormat::Json | OutputFormat::Pretty => {
            use serde_json::json;
            let response = json!({
                "completions": completions,
                "prefix": prefix,
                "count": completions.len()
            });
            let rendered = if matches!(cli.output, OutputFormat::Pretty) {
                serde_json::to_string_pretty(&response)?
            } else {
                serde_json::to_string(&response)?
            };
            println!("{}", rendered);
        }
    }
    let output_formatting_ms = format_start.elapsed().as_millis() as u64;
    let total_ms = total_start.elapsed().as_millis() as u64;

    // Print metrics to stderr if requested
    if cli.show_metrics {
        eprintln!("Performance metrics:");
        eprintln!("  Backend detection: {}ms", backend_detection_ms);
        eprintln!("  Query execution: {}ms", query_execution_ms);
        eprintln!("  Output formatting: {}ms", output_formatting_ms);
        eprintln!("  Total: {}ms", total_ms);
    }

    Ok(())
}

fn run_lookup(
    cli: &Cli,
    fqn: &str,
) -> Result<(), LlmError> {
    // Validate database path
    let db_path = if let Some(db_path) = &cli.db {
        validate_path(db_path, true)?
    } else {
        return Err(LlmError::DatabaseNotFound {
            path: "none".to_string(),
        });
    };

    // Validate FQN is not empty
    if fqn.trim().is_empty() {
        return Err(LlmError::InvalidQuery {
            query: "--fqn cannot be empty".to_string(),
        });
    }

    // Start total timing
    let total_start = std::time::Instant::now();

    // Detect and open backend automatically (SQLite or Native-V2)
    let detect_start = std::time::Instant::now();
    let backend = Backend::detect_and_open(&db_path)?;
    let backend_detection_ms = detect_start.elapsed().as_millis() as u64;

    // Check if backend supports lookup command (native-v3 only)
    require_native_v3(&backend, "lookup", &db_path)?;

    // Lookup symbol by FQN via backend
    let query_start = std::time::Instant::now();
    let symbol = backend.lookup(fqn, &db_path.to_string_lossy())?;
    let query_execution_ms = query_start.elapsed().as_millis() as u64;

    // Render output based on format
    let format_start = std::time::Instant::now();
    match cli.output {
        OutputFormat::Human => {
            println!("Symbol: {}", symbol.name);
            println!("Kind: {}", symbol.kind);
            println!("FQN: {}", symbol.fqn.as_deref().unwrap_or("<none>"));
            if let Some(canonical_fqn) = &symbol.canonical_fqn {
                println!("Canonical FQN: {}", canonical_fqn);
            }
            if let Some(display_fqn) = &symbol.display_fqn {
                println!("Display FQN: {}", display_fqn);
            }
            println!("Location: {}:{}:{}",
                symbol.span.file_path,
                symbol.span.start_line,
                symbol.span.start_col
            );
            if let Some(parent) = &symbol.parent {
                println!("Parent: {}", parent);
            }
            if let Some(language) = &symbol.language {
                println!("Language: {}", language);
            }
        }
        OutputFormat::Json | OutputFormat::Pretty => {
            // Create a response with just the single symbol
            let response = vec![symbol];
            let rendered = if matches!(cli.output, OutputFormat::Pretty) {
                serde_json::to_string_pretty(&response)?
            } else {
                serde_json::to_string(&response)?
            };
            println!("{}", rendered);
        }
    }
    let output_formatting_ms = format_start.elapsed().as_millis() as u64;
    let total_ms = total_start.elapsed().as_millis() as u64;

    // Print metrics to stderr if requested
    if cli.show_metrics {
        eprintln!("Performance metrics:");
        eprintln!("  Backend detection: {}ms", backend_detection_ms);
        eprintln!("  Query execution: {}ms", query_execution_ms);
        eprintln!("  Output formatting: {}ms", output_formatting_ms);
        eprintln!("  Total: {}ms", total_ms);
    }

    Ok(())
}

/// Run watch command with signal handling.
#[cfg(feature = "unstable-watch")]
fn run_watch(
    cli: &Cli,
    query: &str,
    mode: SearchMode,
    path: &Option<PathBuf>,
    kind: &Option<String>,
    limit: usize,
    regex: bool,
) -> Result<(), LlmError> {
    use llmgrep::algorithm::AlgorithmOptions;
    use llmgrep::query::{
        AstOptions, ContextOptions, DepthOptions, FqnOptions,
        MetricsOptions, SearchOptions, SnippetOptions,
    };

    // Validate database path
    let db_path = if let Some(db_path) = &cli.db {
        validate_path(db_path, true)?
    } else {
        return Err(LlmError::DatabaseNotFound {
            path: "none".to_string(),
        });
    };

    // Validate query is not empty
    if query.trim().is_empty() {
        return Err(LlmError::EmptyQuery);
    }

    // Validate path filter if provided
    let validated_path = if let Some(p) = path {
        Some(validate_path(p, false)?)
    } else {
        None
    };

    // For watch command, we only support symbols mode for now
    if !matches!(mode, SearchMode::Symbols) {
        return Err(LlmError::InvalidQuery {
            query: "Watch mode only supports symbols search. Use --mode symbols (default).".to_string(),
        });
    }

    // Build SearchOptions from command args
    let options = SearchOptions {
        db_path: &db_path,
        query,
        path_filter: validated_path.as_ref(),
        kind_filter: kind.as_deref(),
        language_filter: None,
        limit,
        use_regex: regex,
        candidates: 1000, // Default for watch
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions::default(),
        include_score: true,
        sort_by: llmgrep::SortMode::Relevance,
        metrics: MetricsOptions::default(),
        ast: AstOptions {
            ast_kinds: Vec::new(),
            with_ast_context: false,
            _phantom: std::marker::PhantomData,
        },
        depth: DepthOptions {
            min_depth: None,
            max_depth: None,
            inside: None,
            contains: None,
        },
        algorithm: AlgorithmOptions {
            from_symbol_set: None,
            reachable_from: None,
            dead_code_in: None,
            in_cycle: None,
            slice_backward_from: None,
            slice_forward_from: None,
            condense: false,
            paths_from: None,
            paths_to: None,
        },
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
    };

    // Create shutdown flag for signal handling
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_clone = shutdown.clone();

    // Register signal handlers for SIGINT and SIGTERM
    #[cfg(unix)]
    {
        use signal_hook::consts::signal;
        use signal_hook::flag;

        let sig_flag = flag::register(signal::SIGINT, shutdown_clone.clone())?;
        let _ = flag::register(signal::SIGTERM, shutdown_clone)?;
        // Keep sig_flag alive to prevent signal handler from being unregistered
        let _ = sig_flag;
    }

    // Run the watch command
    llmgrep::watch_cmd::run_watch(db_path.clone(), options, cli.output, shutdown)
        .map_err(|e| LlmError::SearchFailed { reason: e.to_string() })?;
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

fn output_symbols(
    cli: &Cli,
    response: SearchResponse,
    partial: bool,
    scc_count: usize,
    metrics: Option<&PerformanceMetrics>,
) -> Result<(), LlmError> {
    match cli.output {
        OutputFormat::Human => {
            if scc_count > 0 {
                println!("{}", format_scc_summary(response.total_count as usize, scc_count));
            } else if let Some(notice) = &response.notice {
                // Empty SCCs case - print warning from notice field
                eprintln!("Warning: {}", notice);
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
            let json_response = json_response_with_partial_and_performance(
                response,
                partial,
                metrics.cloned(),
            );
            let rendered = if matches!(cli.output, OutputFormat::Pretty) {
                serde_json::to_string_pretty(&json_response)?
            } else {
                serde_json::to_string(&json_response)?
            };
            println!("{}", rendered);
        }
    }
    Ok(())
}

fn output_references(
    cli: &Cli,
    response: ReferenceSearchResponse,
    partial: bool,
    metrics: Option<&PerformanceMetrics>,
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
            let json_response = json_response_with_partial_and_performance(
                response,
                partial,
                metrics.cloned(),
            );
            let rendered = if matches!(cli.output, OutputFormat::Pretty) {
                serde_json::to_string_pretty(&json_response)?
            } else {
                serde_json::to_string(&json_response)?
            };
            println!("{}", rendered);
        }
    }
    Ok(())
}

fn output_calls(
    cli: &Cli,
    response: CallSearchResponse,
    partial: bool,
    metrics: Option<&PerformanceMetrics>,
) -> Result<(), LlmError> {
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
            let json_response = json_response_with_partial_and_performance(
                response,
                partial,
                metrics.cloned(),
            );
            let rendered = if matches!(cli.output, OutputFormat::Pretty) {
                serde_json::to_string_pretty(&json_response)?
            } else {
                serde_json::to_string(&json_response)?
            };
            println!("{}", rendered);
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

/// Check if backend is native-v3, return error if SQLite
///
/// This helper function is used by commands that require native-v3 storage.
/// When native-v3 feature is disabled, SQLite backend is the only variant,
/// so this function always returns RequiresNativeV3Backend error.
fn require_native_v3(backend: &Backend, command: &str, db_path: &Path) -> Result<(), LlmError> {
    #[cfg(feature = "native-v3")]
    {
        match backend {
            Backend::NativeV3(_) => Ok(()),
            Backend::Sqlite(_) => Err(LlmError::RequiresNativeV3Backend {
                command: command.to_string(),
                path: db_path.display().to_string(),
            }),
        }
    }
    #[cfg(not(feature = "native-v3"))]
    {
        // When native-v3 feature is disabled, all backends are SQLite
        let _ = (backend, command);
        Err(LlmError::RequiresNativeV3Backend {
            command: command.to_string(),
            path: db_path.display().to_string(),
        })
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
            Some(Command::Search { query, .. }) => {
                assert_eq!(query, "test");
            }
            _ => panic!("Expected Command::Search"),
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
            Some(Command::Search { limit, .. }) => {
                assert_eq!(limit, 500);
            }
            _ => panic!("Expected Command::Search"),
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
            Some(Command::Search { regex, .. }) => {
                assert!(regex, "Regex flag should be set");
            }
            _ => panic!("Expected Command::Search"),
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
            Some(Command::Search { fields, .. }) => {
                assert_eq!(fields.as_ref().unwrap(), "context,snippet,score");
            }
            _ => panic!("Expected Command::Search"),
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
            Some(Command::Search { mode, .. }) => {
                assert!(matches!(mode, SearchMode::Symbols));
            }
            _ => panic!("Expected Command::Search"),
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
            Some(Command::Search { mode, .. }) => {
                assert!(matches!(mode, SearchMode::References));
            }
            _ => panic!("Expected Command::Search"),
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
            Some(Command::Search { mode, .. }) => {
                assert!(matches!(mode, SearchMode::Calls));
            }
            _ => panic!("Expected Command::Search"),
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
            Some(Command::Search { mode, .. }) => {
                assert!(matches!(mode, SearchMode::Auto));
            }
            _ => panic!("Expected Command::Search"),
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
            Some(Command::Search { query, .. }) => {
                assert_eq!(query, "");
            }
            _ => panic!("Expected Command::Search"),
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

    // AST command tests
    #[test]
    fn test_ast_command_basic() {
        let temp_db = create_temp_db().expect("Failed to create temp db");
        let temp_file = std::env::temp_dir().join("test_ast.rs");
        std::fs::write(&temp_file, "fn main() {}").expect("Failed to create temp file");

        let args = [
            "llmgrep",
            "--db",
            temp_db.to_str().unwrap(),
            "ast",
            "--file",
            temp_file.to_str().unwrap(),
        ];
        let result = Cli::try_parse_from(args);
        assert!(result.is_ok(), "Should parse ast command");

        let cli = result.unwrap();
        match cli.command {
            Some(Command::Ast { file, position, limit }) => {
                assert_eq!(file, temp_file);
                assert_eq!(position, None);
                assert_eq!(limit, 10000); // default
            }
            _ => panic!("Expected Command::Ast"),
        }

        // Clean up
        std::fs::remove_file(&temp_file).ok();
    }

    #[test]
    fn test_ast_command_with_position() {
        let temp_db = create_temp_db().expect("Failed to create temp db");
        let temp_file = std::env::temp_dir().join("test_ast.rs");
        std::fs::write(&temp_file, "fn main() {}").expect("Failed to create temp file");

        let args = [
            "llmgrep",
            "--db",
            temp_db.to_str().unwrap(),
            "ast",
            "--file",
            temp_file.to_str().unwrap(),
            "--position",
            "100",
        ];
        let result = Cli::try_parse_from(args);
        assert!(result.is_ok(), "Should parse ast command with position");

        let cli = result.unwrap();
        match cli.command {
            Some(Command::Ast { position, .. }) => {
                assert_eq!(position, Some(100));
            }
            _ => panic!("Expected Command::Ast"),
        }

        // Clean up
        std::fs::remove_file(&temp_file).ok();
    }

    #[test]
    fn test_ast_command_with_limit() {
        let temp_db = create_temp_db().expect("Failed to create temp db");
        let temp_file = std::env::temp_dir().join("test_ast.rs");
        std::fs::write(&temp_file, "fn main() {}").expect("Failed to create temp file");

        let args = [
            "llmgrep",
            "--db",
            temp_db.to_str().unwrap(),
            "ast",
            "--file",
            temp_file.to_str().unwrap(),
            "--limit",
            "500",
        ];
        let result = Cli::try_parse_from(args);
        assert!(result.is_ok(), "Should parse ast command with limit");

        let cli = result.unwrap();
        match cli.command {
            Some(Command::Ast { limit, .. }) => {
                assert_eq!(limit, 500);
            }
            _ => panic!("Expected Command::Ast"),
        }

        // Clean up
        std::fs::remove_file(&temp_file).ok();
    }

    #[test]
    fn test_ast_limit_validation_min() {
        let temp_db = create_temp_db().expect("Failed to create temp db");
        let temp_file = std::env::temp_dir().join("test_ast.rs");
        std::fs::write(&temp_file, "fn main() {}").expect("Failed to create temp file");

        let args = [
            "llmgrep",
            "--db",
            temp_db.to_str().unwrap(),
            "ast",
            "--file",
            temp_file.to_str().unwrap(),
            "--limit",
            "0",
        ];
        let result = Cli::try_parse_from(args);
        assert!(
            result.is_err(),
            "Should reject limit=0 (range is 1..=100000)"
        );

        // Clean up
        std::fs::remove_file(&temp_file).ok();
    }

    #[test]
    fn test_ast_limit_validation_max() {
        let temp_db = create_temp_db().expect("Failed to create temp db");
        let temp_file = std::env::temp_dir().join("test_ast.rs");
        std::fs::write(&temp_file, "fn main() {}").expect("Failed to create temp file");

        let args = [
            "llmgrep",
            "--db",
            temp_db.to_str().unwrap(),
            "ast",
            "--file",
            temp_file.to_str().unwrap(),
            "--limit",
            "100001",
        ];
        let result = Cli::try_parse_from(args);
        assert!(
            result.is_err(),
            "Should reject limit=100001 (range is 1..=100000)"
        );

        // Clean up
        std::fs::remove_file(&temp_file).ok();
    }

    // Find-ast command tests
    #[test]
    fn test_find_ast_command_basic() {
        let temp_db = create_temp_db().expect("Failed to create temp db");

        let args = [
            "llmgrep",
            "--db",
            temp_db.to_str().unwrap(),
            "find-ast",
            "--kind",
            "function_item",
        ];
        let result = Cli::try_parse_from(args);
        assert!(result.is_ok(), "Should parse find-ast command");

        let cli = result.unwrap();
        match cli.command {
            Some(Command::FindAst { kind }) => {
                assert_eq!(kind, "function_item");
            }
            _ => panic!("Expected Command::FindAst"),
        }
    }

    #[test]
    fn test_find_ast_command_with_various_kinds() {
        let temp_db = create_temp_db().expect("Failed to create temp db");

        let test_kinds = [
            "if_expression",
            "while_expression",
            "for_expression",
            "struct_item",
        ];

        for kind in test_kinds {
            let args = [
                "llmgrep",
                "--db",
                temp_db.to_str().unwrap(),
                "find-ast",
                "--kind",
                kind,
            ];
            let result = Cli::try_parse_from(args);
            assert!(result.is_ok(), "Should parse find-ast with kind {}", kind);

            let cli = result.unwrap();
            match cli.command {
                Some(Command::FindAst { kind: k }) => {
                    assert_eq!(k, kind);
                }
                _ => panic!("Expected Command::FindAst"),
            }
        }
    }
}
