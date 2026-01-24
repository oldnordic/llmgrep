use clap::{Parser, Subcommand, ValueEnum, value_parser};
use llmgrep::error::LlmError;
use llmgrep::output::{
    json_response, json_response_with_partial, CallSearchResponse, CombinedSearchResponse,
    ErrorResponse, OutputFormat, ReferenceSearchResponse, SearchResponse,
};
use llmgrep::query::{search_calls, search_references, search_symbols};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "llmgrep", version, about = "Smart grep backed by a Magellan code map")]
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
    Search {
        #[arg(long)]
        query: String,

        #[arg(long, value_enum, default_value = "symbols")]
        mode: SearchMode,

        #[arg(long)]
        path: Option<PathBuf>,

        #[arg(long)]
        kind: Option<String>,

        #[arg(long, default_value_t = 50)]
        limit: usize,

        #[arg(long)]
        regex: bool,

        #[arg(long, default_value_t = 500)]
        candidates: usize,

        #[arg(long)]
        with_context: bool,

        #[arg(long, default_value_t = 3)]
        context_lines: usize,

        #[arg(long, default_value_t = 20)]
        max_context_lines: usize,

        #[arg(long)]
        with_snippet: bool,

        #[arg(long)]
        with_fqn: bool,

        #[arg(long, default_value_t = 200)]
        max_snippet_bytes: usize,

        #[arg(long)]
        fields: Option<String>,

        #[arg(long, value_enum, default_value = "per-mode")]
        auto_limit: AutoLimitMode,
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
            auto_limit,
        } => run_search(
            cli,
            query,
            *mode,
            path,
            kind,
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
            *auto_limit,
        ),
    }
}

fn run_search(
    cli: &Cli,
    query: &str,
    mode: SearchMode,
    path: &Option<PathBuf>,
    kind: &Option<String>,
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
    auto_limit: AutoLimitMode,
) -> Result<(), LlmError> {
    if cli.db.is_none() {
        return Err(LlmError::DatabaseNotFound {
            path: "none".to_string(),
        });
    }
    if query.trim().is_empty() {
        return Err(LlmError::EmptyQuery);
    }

    let db_path = cli.db.as_ref().expect("db path missing");
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
        fields.as_ref().map_or(true, |f| f.score)
    } else {
        true
    };

    let include_fqn = wants_json && fields.as_ref().map_or(with_fqn, |f| f.fqn);
    let include_canonical_fqn =
        wants_json && fields.as_ref().map_or(with_fqn, |f| f.canonical_fqn);
    let include_display_fqn =
        wants_json && fields.as_ref().map_or(with_fqn, |f| f.display_fqn);

    match mode {
        SearchMode::Symbols => {
            let (response, partial) = search_symbols(
                db_path,
                query,
                path.as_ref(),
                kind.as_deref(),
                limit,
                regex,
                candidates,
                include_context,
                context_lines,
                max_context_lines,
                include_snippet,
                include_fqn,
                include_canonical_fqn,
                include_display_fqn,
                include_score,
                max_snippet_bytes,
            )?;
            output_symbols(cli, response, partial)?;
        }
        SearchMode::References => {
            let (response, partial) = search_references(
                db_path,
                query,
                path.as_ref(),
                limit,
                regex,
                candidates,
                include_context,
                context_lines,
                max_context_lines,
                include_snippet,
                include_score,
                max_snippet_bytes,
            )?;
            output_references(cli, response, partial)?;
        }
        SearchMode::Calls => {
            let (response, partial) = search_calls(
                db_path,
                query,
                path.as_ref(),
                limit,
                regex,
                candidates,
                include_context,
                context_lines,
                max_context_lines,
                include_snippet,
                include_score,
                max_snippet_bytes,
            )?;
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

            let (symbols, symbols_partial) = search_symbols(
                db_path,
                query,
                path.as_ref(),
                kind.as_deref(),
                symbols_limit,
                regex,
                candidates,
                include_context,
                context_lines,
                max_context_lines,
                include_snippet,
                include_fqn,
                include_canonical_fqn,
                include_display_fqn,
                include_score,
                max_snippet_bytes,
            )?;
            let (references, refs_partial) = search_references(
                db_path,
                query,
                path.as_ref(),
                references_limit,
                regex,
                candidates,
                include_context,
                context_lines,
                max_context_lines,
                include_snippet,
                include_score,
                max_snippet_bytes,
            )?;
            let (calls, calls_partial) = search_calls(
                db_path,
                query,
                path.as_ref(),
                calls_limit,
                regex,
                candidates,
                include_context,
                context_lines,
                max_context_lines,
                include_snippet,
                include_score,
                max_snippet_bytes,
            )?;
            let total_count = symbols.total_count + references.total_count + calls.total_count;
            let combined = CombinedSearchResponse {
                query: query.to_string(),
                path_filter: path.as_ref().map(|p| p.to_string_lossy().to_string()),
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

fn output_symbols(cli: &Cli, response: SearchResponse, partial: bool) -> Result<(), LlmError> {
    match cli.output {
        OutputFormat::Human => {
            println!("total: {}", response.total_count);
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
                println!("partial: true");
            }
        }
        OutputFormat::Json | OutputFormat::Pretty => {
            let payload = json_response_with_partial(response, partial);
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

fn output_references(cli: &Cli, response: ReferenceSearchResponse, partial: bool) -> Result<(), LlmError> {
    match cli.output {
        OutputFormat::Human => {
            println!("total: {}", response.total_count);
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
                println!("partial: true");
            }
        }
        OutputFormat::Json | OutputFormat::Pretty => {
            let payload = json_response_with_partial(response, partial);
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

fn output_calls(cli: &Cli, response: CallSearchResponse, partial: bool) -> Result<(), LlmError> {
    match cli.output {
        OutputFormat::Human => {
            println!("total: {}", response.total_count);
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
                println!("partial: true");
            }
        }
        OutputFormat::Json | OutputFormat::Pretty => {
            let payload = json_response_with_partial(response, partial);
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
