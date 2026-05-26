use crate::cli::{resolve_db_path, validate_path, Cli, SearchMode};
use llmgrep::algorithm::AlgorithmOptions;
use llmgrep::error::LlmError;
use llmgrep::output::OutputFormat;
use llmgrep::query::{
    AstOptions, ContextOptions, DepthOptions, FqnOptions, MetricsOptions, SearchOptions,
    SnippetOptions,
};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

#[allow(clippy::too_many_arguments)]
pub fn run_watch(
    cli: &Cli,
    query: &str,
    mode: &SearchMode,
    path: &Option<PathBuf>,
    kind: &Option<String>,
    limit: usize,
    regex: bool,
) -> Result<(), LlmError> {
    let db_path = resolve_db_path(cli)?;

    if query.trim().is_empty() {
        return Err(LlmError::EmptyQuery);
    }

    let validated_path = if let Some(p) = path {
        Some(validate_path(p, false)?)
    } else {
        None
    };

    if !matches!(mode, SearchMode::Symbols) {
        return Err(LlmError::InvalidQuery {
            query: "Watch mode only supports symbols search. Use --mode symbols (default)."
                .to_string(),
        });
    }

    let options = SearchOptions {
        db_path: &db_path,
        query,
        path_filter: validated_path.as_ref(),
        kind_filter: kind.as_deref(),
        language_filter: None,
        limit,
        use_regex: regex,
        candidates: 1000,
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
        coverage_filter: None,
    };

    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_clone = shutdown.clone();

    #[cfg(unix)]
    {
        use signal_hook::consts::signal;
        use signal_hook::flag;

        let sig_flag = flag::register(signal::SIGINT, shutdown_clone.clone())?;
        let _ = flag::register(signal::SIGTERM, shutdown_clone)?;
        let _ = sig_flag;
    }

    llmgrep::watch_cmd::run_watch(db_path.clone(), options, cli.output, shutdown).map_err(|e| {
        LlmError::SearchFailed {
            reason: e.to_string(),
        }
    })?;
    Ok(())
}
