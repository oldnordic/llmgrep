use clap::builder::{RangedI64ValueParser, TypedValueParser};
use clap::{Parser, Subcommand, ValueEnum};
use llmgrep::error::LlmError;
use llmgrep::output::{json_response, ErrorResponse, OutputFormat};
use llmgrep::SortMode;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct SearchParams {
    pub query: String,
    pub mode: SearchMode,
    pub path: Option<PathBuf>,
    pub kind: Option<String>,
    pub language: Option<String>,
    pub label: Option<String>,
    pub limit: usize,
    pub regex: bool,
    pub candidates: usize,
    pub with_context: bool,
    pub context_lines: usize,
    pub max_context_lines: usize,
    pub with_snippet: bool,
    pub with_fqn: bool,
    pub max_snippet_bytes: usize,
    pub fields: Option<String>,
    pub sort_by: SortMode,
    pub auto_limit: AutoLimitMode,
    pub min_complexity: Option<usize>,
    pub max_complexity: Option<usize>,
    pub min_fan_in: Option<usize>,
    pub min_fan_out: Option<usize>,
    pub symbol_id: Option<String>,
    pub fqn: Option<String>,
    pub exact_fqn: Option<String>,
    pub ast_kind: Option<String>,
    pub with_ast_context: bool,
    pub min_depth: Option<usize>,
    pub max_depth: Option<usize>,
    pub inside: Option<String>,
    pub contains: Option<String>,
    pub from_symbol_set: Option<String>,
    pub reachable_from: Option<String>,
    pub dead_code_in: Option<String>,
    pub in_cycle: Option<String>,
    pub slice_backward_from: Option<String>,
    pub slice_forward_from: Option<String>,
    pub condense: bool,
    pub paths_from: Option<String>,
    pub paths_to: Option<String>,
    pub coverage_filter: Option<llmgrep::query::CoverageFilter>,
    pub tags: Option<String>,
    pub wikilinks: Option<String>,
    pub source_kind: Option<String>,
    pub since: Option<i64>,
    pub subject: Option<String>,
    pub predicate: Option<String>,
    pub object: Option<String>,
    pub fact_status_filter: Option<String>,
    pub subject_type: Option<String>,
}

fn ranged_usize(min: i64, max: i64) -> impl TypedValueParser<Value = usize> {
    let inner = RangedI64ValueParser::new().range(min..=max);
    inner.map(|v: i64| v as usize)
}

#[derive(Parser)]
#[command(
    name = "llmgrep",
    version = env!("CARGO_PKG_VERSION"),
    about = "Smart grep backed by a Magellan code map"
)]
pub struct Cli {
    #[arg(long, global = true, default_value_t = OutputFormat::Human)]
    pub output: OutputFormat,

    #[arg(long, global = true)]
    pub db: Option<PathBuf>,

    #[arg(long, global = true)]
    pub show_metrics: bool,

    #[arg(long, global = true, default_value = "false")]
    pub detect_backend: bool,

    #[arg(long, global = true, default_value = "false")]
    pub record: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)]
pub enum Command {
    #[command(after_help = SEARCH_EXAMPLES)]
    Search {
        #[arg(long, default_value = ".*")]
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

        #[arg(long, value_parser = ranged_usize(0, 1000))]
        min_complexity: Option<usize>,

        #[arg(long, value_parser = ranged_usize(0, 1000))]
        max_complexity: Option<usize>,

        #[arg(long, value_parser = ranged_usize(0, 10000))]
        min_fan_in: Option<usize>,

        #[arg(long, value_parser = ranged_usize(0, 10000))]
        min_fan_out: Option<usize>,

        #[arg(long)]
        symbol_id: Option<String>,

        #[arg(long)]
        fqn: Option<String>,

        #[arg(long)]
        exact_fqn: Option<String>,

        #[arg(long, value_name = "KIND")]
        ast_kind: Option<String>,

        #[arg(long)]
        with_ast_context: bool,

        #[arg(long, value_parser = ranged_usize(0, 100))]
        min_depth: Option<usize>,

        #[arg(long, value_parser = ranged_usize(0, 100))]
        max_depth: Option<usize>,

        #[arg(long, value_name = "KIND")]
        inside: Option<String>,

        #[arg(long, value_name = "KIND")]
        contains: Option<String>,

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

        #[arg(long)]
        uncovered: bool,

        #[arg(long)]
        covered: bool,

        #[arg(long)]
        tags: Option<String>,

        #[arg(long)]
        wikilinks: Option<String>,

        #[arg(long)]
        source_kind: Option<String>,

        #[arg(long)]
        since: Option<i64>,

        #[arg(long)]
        subject: Option<String>,

        #[arg(long)]
        predicate: Option<String>,

        #[arg(long)]
        object: Option<String>,

        #[arg(long)]
        status: Option<String>,

        #[arg(long, name = "subject-type")]
        subject_type: Option<String>,
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

    Explore {
        #[arg(long)]
        intent: String,

        #[arg(long, default_value_t = 10, value_parser = ranged_usize(1, 100))]
        limit: usize,
    },

    Stats,

    Evolve {
        #[arg(long, default_value = ".*")]
        query: String,

        #[arg(long, default_value_t = 8, value_parser = ranged_usize(1, 10000))]
        min_score: usize,

        #[arg(long, default_value = "false")]
        dry_run: bool,

        #[arg(long, default_value_t = 50, value_parser = ranged_usize(1, 1000))]
        limit: usize,
    },

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

    VectorCreate {
        #[arg(long)]
        name: String,

        #[arg(long)]
        dim: usize,
    },

    VectorSearch {
        #[arg(long)]
        query: String,

        #[arg(long)]
        index: String,

        #[arg(long, default_value_t = 10, value_parser = ranged_usize(1, 1000))]
        limit: usize,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum SearchMode {
    Symbols,
    References,
    Calls,
    Implements,
    Auto,
    Labels,
    Docs,
    Facts,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum AutoLimitMode {
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

  # Implements search (type-trait relationships)
  llmgrep --db code.db search --query "SideTables" --mode implements
  llmgrep --db code.db search --query "AmbiguityOps" --mode implements --output json

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
  llmgrep --db .magellan/llmgrep.db lookup --fqn "crate::backend::SqliteBackend" --output json

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

pub fn validate_path(path: &Path, is_database: bool) -> Result<PathBuf, LlmError> {
    let canonical = path
        .canonicalize()
        .map_err(|e| LlmError::PathValidationFailed {
            path: path.display().to_string(),
            reason: format!("Cannot resolve path: {}", e),
        })?;

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

const DEFAULT_DB_FILENAME: &str = "llmgrep.db";
const MAGELLAN_DIR: &str = ".magellan";

pub fn resolve_db_path(cli: &Cli) -> Result<PathBuf, LlmError> {
    if let Some(db_path) = &cli.db {
        return validate_path(db_path, true);
    }

    let cwd = std::env::current_dir().map_err(|e| LlmError::PathValidationFailed {
        path: "CWD".to_string(),
        reason: format!("Cannot determine current directory: {}", e),
    })?;

    let candidates: Vec<PathBuf> = [
        Some(cwd.join(MAGELLAN_DIR).join(DEFAULT_DB_FILENAME)),
        find_git_root_db(&cwd),
    ]
    .into_iter()
    .flatten()
    .collect();

    for candidate in candidates {
        if candidate.is_file() {
            eprintln!(
                "Note: Using auto-detected database: {}",
                candidate.display()
            );
            return validate_path(&candidate, true);
        }
    }

    Err(LlmError::DatabaseNotFound {
        path: format!(
            "No --db flag and no .magellan/{} found in CWD or git root. \
             Run 'magellan watch --root ./src --db .magellan/llmgrep.db --scan-initial' to create one.",
            DEFAULT_DB_FILENAME
        ),
    })
}

pub fn find_git_root_db(cwd: &Path) -> Option<PathBuf> {
    let mut dir = cwd;
    loop {
        if dir.join(".git").exists() {
            return Some(dir.join(MAGELLAN_DIR).join(DEFAULT_DB_FILENAME));
        }
        dir = dir.parent()?;
    }
}

#[derive(Debug, Default)]
pub struct FieldFlags {
    pub context: bool,
    pub snippet: bool,
    pub score: bool,
    pub fqn: bool,
    pub canonical_fqn: bool,
    pub display_fqn: bool,
}

pub fn parse_fields(value: &str) -> Result<FieldFlags, LlmError> {
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

pub fn split_auto_limit(limit: usize) -> (usize, usize, usize) {
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

pub fn normalize_language(lang: &str) -> String {
    match lang.to_lowercase().as_str() {
        "rust" | "rs" => "rust".to_string(),
        "python" | "py" => "python".to_string(),
        "javascript" | "js" | "ecmascript" => "javascript".to_string(),
        "typescript" | "ts" => "typescript".to_string(),
        "c" => "c".to_string(),
        "c++" | "cpp" | "cxx" | "cc" => "cpp".to_string(),
        "java" => "java".to_string(),
        "go" | "golang" => "go".to_string(),
        other => other.to_string(),
    }
}

pub fn parse_kinds(kind: &str) -> Vec<String> {
    kind.split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .map(|k| normalize_kind(&k))
        .collect()
}

fn normalize_kind(kind: &str) -> String {
    match kind.to_lowercase().as_str() {
        "function" | "fn" | "func" => "fn".to_string(),
        "method" => "method".to_string(),
        "struct" | "class" => "struct".to_string(),
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

pub fn looks_like_regex(query: &str) -> bool {
    if query == ".*" || query == ".+" {
        return true;
    }

    if query.starts_with('^') || query.ends_with('$') {
        return true;
    }

    if query.contains('[') && query.contains(']') {
        return true;
    }

    let has_quantifier = query.contains(".*")
        || query.contains(".+")
        || query.contains(".?")
        || query.matches('{').count() > 0;

    if has_quantifier {
        return true;
    }

    if query.contains("\\d")
        || query.contains("\\w")
        || query.contains("\\s")
        || query.contains("\\.")
        || query.contains("\\(")
        || query.contains("\\)")
    {
        return true;
    }

    if query.contains('|') && !query.starts_with('|') && !query.ends_with('|') {
        return true;
    }

    false
}

pub fn emit_error(cli: &Cli, err: &LlmError) {
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
