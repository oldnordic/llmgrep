use crate::cli::{resolve_db_path, Cli, Command};
use crate::commands;
use llmgrep::error::LlmError;
use llmgrep::output::OutputFormat;

pub fn command_name(cli: &Cli) -> &str {
    match &cli.command {
        None => "none",
        Some(cmd) => match cmd {
            Command::Search { .. } => "search",
            Command::Ast { .. } => "ast",
            Command::FindAst { .. } => "find-ast",
            Command::Complete { .. } => "complete",
            Command::Lookup { .. } => "lookup",
            Command::Explore { .. } => "explore",
            Command::Navigate { .. } => "navigate",
            Command::Stats => "stats",
            Command::Evolve { .. } => "evolve",
            #[cfg(feature = "unstable-watch")]
            Command::Watch { .. } => "watch",
            Command::VectorCreate { .. } => "vector-create",
            Command::VectorSearch { .. } => "vector-search",
        },
    }
}

pub fn dispatch(cli: &Cli) -> Result<(), LlmError> {
    if cli.detect_backend {
        let validated_db = resolve_db_path(cli)?;

        use magellan::migrate_backend_cmd::{detect_backend_format, BackendFormat};
        let format =
            detect_backend_format(&validated_db).map_err(|e| LlmError::BackendDetectionFailed {
                path: validated_db.display().to_string(),
                reason: e.to_string(),
            })?;

        let backend_str = match format {
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
        None => Err(LlmError::InvalidQuery {
            query: "No subcommand provided. Use --help for usage information.".to_string(),
        }),
        Some(cmd) => match cmd {
            Command::Ast {
                file,
                position,
                limit,
            } => commands::run_ast(cli, file, *position, *limit),

            Command::FindAst { kind } => commands::run_find_ast(cli, kind),

            Command::Complete { prefix, limit } => {
                commands::run_complete(cli, prefix.clone(), *limit)
            }

            Command::Lookup { fqn } => commands::run_lookup(cli, fqn),

            Command::Explore { intent, limit } => {
                let validated_db = resolve_db_path(cli)?;
                let output = match cli.output {
                    OutputFormat::Human => llmgrep::output::OutputFormat::Human,
                    OutputFormat::Json => llmgrep::output::OutputFormat::Json,
                    OutputFormat::Pretty => llmgrep::output::OutputFormat::Pretty,
                };
                llmgrep::query::run_explore(&validated_db, intent, *limit, output).map_err(|e| {
                    LlmError::InvalidQuery {
                        query: e.to_string(),
                    }
                })
            }

            Command::Navigate {
                symbol,
                id,
                edges,
                callers,
                callees,
                depth,
            } => {
                let validated_db = resolve_db_path(cli)?;
                let output = match cli.output {
                    OutputFormat::Human => llmgrep::output::OutputFormat::Human,
                    OutputFormat::Json => llmgrep::output::OutputFormat::Json,
                    OutputFormat::Pretty => llmgrep::output::OutputFormat::Pretty,
                };
                llmgrep::query::navigate::run_navigate(
                    &validated_db,
                    symbol,
                    *id,
                    *edges,
                    *callers,
                    *callees,
                    *depth,
                    output,
                )
                .map_err(|e| LlmError::InvalidQuery {
                    query: e.to_string(),
                })
            }

            Command::Search { .. } => commands::dispatch_search(cli, cmd),

            Command::Stats => commands::run_stats_cmd(cli),

            Command::Evolve {
                query,
                min_score,
                dry_run,
                limit,
            } => commands::run_evolve_cmd(cli, query, *min_score, *dry_run, *limit),

            #[cfg(feature = "unstable-watch")]
            Command::Watch {
                query,
                mode,
                path,
                kind,
                limit,
                regex,
            } => commands::run_watch(cli, query, *mode, path, kind, *limit, *regex),
            Command::VectorCreate { name, dim } => commands::run_vector_create(name, *dim),
            Command::VectorSearch {
                query,
                index,
                limit,
            } => commands::run_vector_search(query, index, *limit),
        },
    }
}
