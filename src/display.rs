use crate::cli::Cli;
use llmgrep::error::LlmError;
use llmgrep::output::{
    json_response_with_partial_and_performance, CallMatch, CallSearchResponse, DocsMatch,
    DocsSearchResponse, FactMatch, FactsSearchResponse, ImplementsMatch,
    ImplementsSearchResponse, OutputFormat, PerformanceMetrics, ReferenceMatch,
    ReferenceSearchResponse, SearchResponse, SemanticMatch, SemanticSearchResponse, SymbolMatch,
};
use llmgrep::output_common::{format_partial_footer, format_total_header};

pub fn format_scc_summary(count: usize, supernode_count: usize) -> String {
    if supernode_count == 1 {
        format!("Found {} symbol in 1 SCC", count)
    } else {
        format!("Found {} symbols in {} SCCs", count, supernode_count)
    }
}

/// Generic helper to prune results vector to fit token budget
pub(crate) fn truncate_response<T: Clone, F>(
    mut results: Vec<T>,
    token_limit: Option<usize>,
    format_fn: F,
) -> (Vec<T>, Option<usize>, bool)
where
    F: Fn(&[T]) -> String,
{
    let mut truncated = false;
    let mut tokens_estimated = None;

    if let Some(limit) = token_limit {
        if limit > 0 {
            loop {
                let formatted = format_fn(&results);
                let tokens_est = formatted.len() / 4;
                if tokens_est <= limit || results.is_empty() {
                    tokens_estimated = Some(tokens_est);
                    break;
                }
                truncated = true;
                let new_len = results.len() / 2;
                results.truncate(new_len);
            }
        }
    }

    if tokens_estimated.is_none() {
        let formatted = format_fn(&results);
        tokens_estimated = Some(formatted.len() / 4);
    }

    (results, tokens_estimated, truncated)
}

pub fn output_symbols(
    cli: &Cli,
    response: SearchResponse,
    partial: bool,
    scc_count: usize,
    metrics: Option<&PerformanceMetrics>,
    tokens: Option<usize>,
) -> Result<(), LlmError> {
    let results = response.results.clone();

    match cli.output {
        OutputFormat::Human => {
            let format_fn = |items: &[SymbolMatch]| {
                let mut human_out = String::new();
                if scc_count > 0 {
                    human_out.push_str(&format_scc_summary(response.total_count as usize, scc_count));
                    human_out.push('\n');
                } else if let Some(notice) = &response.notice {
                    human_out.push_str(&format!("Warning: {}\n", notice));
                    human_out.push_str("No symbols found - codebase contains no strongly connected components\n");
                }
                human_out.push_str(&format_total_header(response.total_count));
                human_out.push('\n');
                for item in items {
                    let coverage_str = item
                        .coverage
                        .as_ref()
                        .map(|c| {
                            format!(
                                " [{}/{} blocks {:.1}%]",
                                c.covered_blocks, c.total_blocks, c.block_percentage
                            )
                        })
                        .unwrap_or_default();
                    human_out.push_str(&format!(
                        "{}:{}:{} {} {} score={}{}\n",
                        item.span.file_path,
                        item.span.start_line,
                        item.span.start_col,
                        item.name,
                        item.kind,
                        item.score.unwrap_or(0),
                        coverage_str
                    ));
                }
                if partial {
                    human_out.push_str(format_partial_footer());
                    human_out.push('\n');
                }
                human_out
            };

            let (pruned_results, _tokens_est, truncated) = truncate_response(results, tokens, format_fn);
            let final_output = format_fn(&pruned_results);
            print!("{}", final_output);
            if truncated {
                if let Some(limit) = tokens {
                    println!("\n*[~{} tokens, truncated]*", limit);
                }
            }
        }
        OutputFormat::Json | OutputFormat::Pretty => {
            let format_fn = |items: &[SymbolMatch]| {
                let mut temp_resp = response.clone();
                temp_resp.results = items.to_vec();
                let json_response = json_response_with_partial_and_performance(temp_resp, partial, metrics.cloned());
                if matches!(cli.output, OutputFormat::Pretty) {
                    serde_json::to_string_pretty(&json_response).unwrap_or_default()
                } else {
                    serde_json::to_string(&json_response).unwrap_or_default()
                }
            };

            let (pruned_results, tokens_est, truncated) = truncate_response(results, tokens, format_fn);
            let mut final_resp = response;
            final_resp.results = pruned_results;

            let mut json_response = json_response_with_partial_and_performance(final_resp, partial, metrics.cloned());
            json_response.tokens_estimated = tokens_est;
            if truncated {
                json_response.truncated = Some(true);
            }

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

pub fn output_references(
    cli: &Cli,
    response: ReferenceSearchResponse,
    partial: bool,
    metrics: Option<&PerformanceMetrics>,
    tokens: Option<usize>,
) -> Result<(), LlmError> {
    let results = response.results.clone();

    match cli.output {
        OutputFormat::Human => {
            let format_fn = |items: &[ReferenceMatch]| {
                let mut human_out = String::new();
                human_out.push_str(&format_total_header(response.total_count));
                human_out.push('\n');
                for item in items {
                    human_out.push_str(&format!(
                        "{}:{}:{} {} score={}\n",
                        item.span.file_path,
                        item.span.start_line,
                        item.span.start_col,
                        item.referenced_symbol,
                        item.score.unwrap_or(0)
                    ));
                }
                if partial {
                    human_out.push_str(format_partial_footer());
                    human_out.push('\n');
                }
                human_out
            };

            let (pruned_results, _tokens_est, truncated) = truncate_response(results, tokens, format_fn);
            let final_output = format_fn(&pruned_results);
            print!("{}", final_output);
            if truncated {
                if let Some(limit) = tokens {
                    println!("\n*[~{} tokens, truncated]*", limit);
                }
            }
        }
        OutputFormat::Json | OutputFormat::Pretty => {
            let format_fn = |items: &[ReferenceMatch]| {
                let mut temp_resp = response.clone();
                temp_resp.results = items.to_vec();
                let json_response = json_response_with_partial_and_performance(temp_resp, partial, metrics.cloned());
                if matches!(cli.output, OutputFormat::Pretty) {
                    serde_json::to_string_pretty(&json_response).unwrap_or_default()
                } else {
                    serde_json::to_string(&json_response).unwrap_or_default()
                }
            };

            let (pruned_results, tokens_est, truncated) = truncate_response(results, tokens, format_fn);
            let mut final_resp = response;
            final_resp.results = pruned_results;

            let mut json_response = json_response_with_partial_and_performance(final_resp, partial, metrics.cloned());
            json_response.tokens_estimated = tokens_est;
            if truncated {
                json_response.truncated = Some(true);
            }

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

pub fn output_calls(
    cli: &Cli,
    response: CallSearchResponse,
    partial: bool,
    metrics: Option<&PerformanceMetrics>,
    tokens: Option<usize>,
) -> Result<(), LlmError> {
    let results = response.results.clone();

    match cli.output {
        OutputFormat::Human => {
            let format_fn = |items: &[CallMatch]| {
                let mut human_out = String::new();
                human_out.push_str(&format_total_header(response.total_count));
                human_out.push('\n');
                for item in items {
                    human_out.push_str(&format!(
                        "{}:{}:{} {} -> {} score={}\n",
                        item.span.file_path,
                        item.span.start_line,
                        item.span.start_col,
                        item.caller,
                        item.callee,
                        item.score.unwrap_or(0)
                    ));
                }
                if partial {
                    human_out.push_str(format_partial_footer());
                    human_out.push('\n');
                }
                human_out
            };

            let (pruned_results, _tokens_est, truncated) = truncate_response(results, tokens, format_fn);
            let final_output = format_fn(&pruned_results);
            print!("{}", final_output);
            if truncated {
                if let Some(limit) = tokens {
                    println!("\n*[~{} tokens, truncated]*", limit);
                }
            }
        }
        OutputFormat::Json | OutputFormat::Pretty => {
            let format_fn = |items: &[CallMatch]| {
                let mut temp_resp = response.clone();
                temp_resp.results = items.to_vec();
                let json_response = json_response_with_partial_and_performance(temp_resp, partial, metrics.cloned());
                if matches!(cli.output, OutputFormat::Pretty) {
                    serde_json::to_string_pretty(&json_response).unwrap_or_default()
                } else {
                    serde_json::to_string(&json_response).unwrap_or_default()
                }
            };

            let (pruned_results, tokens_est, truncated) = truncate_response(results, tokens, format_fn);
            let mut final_resp = response;
            final_resp.results = pruned_results;

            let mut json_response = json_response_with_partial_and_performance(final_resp, partial, metrics.cloned());
            json_response.tokens_estimated = tokens_est;
            if truncated {
                json_response.truncated = Some(true);
            }

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

pub fn output_implements(
    cli: &Cli,
    response: ImplementsSearchResponse,
    partial: bool,
    metrics: Option<&PerformanceMetrics>,
    tokens: Option<usize>,
) -> Result<(), LlmError> {
    let results = response.results.clone();

    match cli.output {
        OutputFormat::Human => {
            let format_fn = |items: &[ImplementsMatch]| {
                let mut human_out = String::new();
                human_out.push_str(&format_total_header(response.total_count));
                human_out.push('\n');
                for item in items {
                    human_out.push_str(&format!(
                        "{}:{}:{} {} impl {} score={}\n",
                        item.span.file_path,
                        item.span.start_line,
                        item.span.start_col,
                        item.type_name,
                        item.trait_name,
                        item.score.unwrap_or(0)
                    ));
                }
                if partial {
                    human_out.push_str(format_partial_footer());
                    human_out.push('\n');
                }
                human_out
            };

            let (pruned_results, _tokens_est, truncated) = truncate_response(results, tokens, format_fn);
            let final_output = format_fn(&pruned_results);
            print!("{}", final_output);
            if truncated {
                if let Some(limit) = tokens {
                    println!("\n*[~{} tokens, truncated]*", limit);
                }
            }
        }
        OutputFormat::Json | OutputFormat::Pretty => {
            let format_fn = |items: &[ImplementsMatch]| {
                let mut temp_resp = response.clone();
                temp_resp.results = items.to_vec();
                let json_response = json_response_with_partial_and_performance(temp_resp, partial, metrics.cloned());
                if matches!(cli.output, OutputFormat::Pretty) {
                    serde_json::to_string_pretty(&json_response).unwrap_or_default()
                } else {
                    serde_json::to_string(&json_response).unwrap_or_default()
                }
            };

            let (pruned_results, tokens_est, truncated) = truncate_response(results, tokens, format_fn);
            let mut final_resp = response;
            final_resp.results = pruned_results;

            let mut json_response = json_response_with_partial_and_performance(final_resp, partial, metrics.cloned());
            json_response.tokens_estimated = tokens_est;
            if truncated {
                json_response.truncated = Some(true);
            }

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

pub fn output_docs(
    cli: &Cli,
    response: DocsSearchResponse,
    metrics: Option<&PerformanceMetrics>,
    tokens: Option<usize>,
) -> Result<(), LlmError> {
    let results = response.results.clone();

    match cli.output {
        OutputFormat::Human => {
            let format_fn = |items: &[DocsMatch]| {
                let mut human_out = String::new();
                human_out.push_str(&format!("{} documents\n", response.total_count));
                for item in items {
                    let kind = item.source_kind.as_deref().unwrap_or("unknown");
                    let title = item.title.as_deref().unwrap_or("<untitled>");
                    human_out.push_str(&format!("{} [{}] {}\n", item.path_or_uri, kind, title));
                    if let Some(tags) = &item.tags {
                        human_out.push_str(&format!("  tags: {}\n", tags));
                    }
                    if let Some(links) = &item.wikilinks {
                        human_out.push_str(&format!("  wikilinks: {}\n", links));
                    }
                }
                human_out
            };

            let (pruned_results, _tokens_est, truncated) = truncate_response(results, tokens, format_fn);
            let final_output = format_fn(&pruned_results);
            print!("{}", final_output);
            if truncated {
                if let Some(limit) = tokens {
                    println!("\n*[~{} tokens, truncated]*", limit);
                }
            }
        }
        OutputFormat::Json | OutputFormat::Pretty => {
            let format_fn = |items: &[DocsMatch]| {
                let mut temp_resp = response.clone();
                temp_resp.results = items.to_vec();
                let json_response = json_response_with_partial_and_performance(temp_resp, false, metrics.cloned());
                if matches!(cli.output, OutputFormat::Pretty) {
                    serde_json::to_string_pretty(&json_response).unwrap_or_default()
                } else {
                    serde_json::to_string(&json_response).unwrap_or_default()
                }
            };

            let (pruned_results, tokens_est, truncated) = truncate_response(results, tokens, format_fn);
            let mut final_resp = response;
            final_resp.results = pruned_results;

            let mut json_response = json_response_with_partial_and_performance(final_resp, false, metrics.cloned());
            json_response.tokens_estimated = tokens_est;
            if truncated {
                json_response.truncated = Some(true);
            }

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

pub fn output_semantic(
    cli: &Cli,
    response: SemanticSearchResponse,
    metrics: Option<&PerformanceMetrics>,
    tokens: Option<usize>,
) -> Result<(), LlmError> {
    let results = response.results.clone();

    match cli.output {
        OutputFormat::Human => {
            let format_fn = |items: &[SemanticMatch]| {
                let mut human_out = String::new();
                human_out.push_str(&format!("{} semantic matches\n", response.total_count));
                for item in items {
                    let lang = item.language.as_deref().unwrap_or("?");
                    human_out.push_str(&format!(
                        "{}:{}:{} {} [{}] {} score={} distance={:.4}\n",
                        item.span.file_path,
                        item.span.start_line,
                        item.span.start_col,
                        item.name,
                        item.kind,
                        lang,
                        item.score,
                        item.distance
                    ));
                }
                human_out
            };

            let (pruned_results, _tokens_est, truncated) = truncate_response(results, tokens, format_fn);
            let final_output = format_fn(&pruned_results);
            print!("{}", final_output);
            if truncated {
                if let Some(limit) = tokens {
                    println!("\n*[~{} tokens, truncated]*", limit);
                }
            }
        }
        OutputFormat::Json | OutputFormat::Pretty => {
            let format_fn = |items: &[SemanticMatch]| {
                let mut temp_resp = response.clone();
                temp_resp.results = items.to_vec();
                let json_response = json_response_with_partial_and_performance(temp_resp, false, metrics.cloned());
                if matches!(cli.output, OutputFormat::Pretty) {
                    serde_json::to_string_pretty(&json_response).unwrap_or_default()
                } else {
                    serde_json::to_string(&json_response).unwrap_or_default()
                }
            };

            let (pruned_results, tokens_est, truncated) = truncate_response(results, tokens, format_fn);
            let mut final_resp = response;
            final_resp.results = pruned_results;

            let mut json_response = json_response_with_partial_and_performance(final_resp, false, metrics.cloned());
            json_response.tokens_estimated = tokens_est;
            if truncated {
                json_response.truncated = Some(true);
            }

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

pub fn output_facts(
    cli: &Cli,
    response: FactsSearchResponse,
    metrics: Option<&PerformanceMetrics>,
    tokens: Option<usize>,
) -> Result<(), LlmError> {
    let results = response.results.clone();

    match cli.output {
        OutputFormat::Human => {
            let format_fn = |items: &[FactMatch]| {
                let mut human_out = String::new();
                human_out.push_str(&format!("{} facts\n", response.total_count));
                for item in items {
                    let subject = item.subject_key.as_deref().unwrap_or("?");
                    let pred = item.predicate.as_deref().unwrap_or("?");
                    let obj = item.object_key.as_deref().unwrap_or("?");
                    let status = item.status.as_deref().unwrap_or("unknown");
                    human_out.push_str(&format!("[{}] {} --{}--> {}\n", status, subject, pred, obj));
                    if let Some(stype) = &item.subject_type {
                        human_out.push_str(&format!("  subject_type: {}\n", stype));
                    }
                    if let Some(otype) = &item.object_type {
                        human_out.push_str(&format!("  object_type: {}\n", otype));
                    }
                }
                human_out
            };

            let (pruned_results, _tokens_est, truncated) = truncate_response(results, tokens, format_fn);
            let final_output = format_fn(&pruned_results);
            print!("{}", final_output);
            if truncated {
                if let Some(limit) = tokens {
                    println!("\n*[~{} tokens, truncated]*", limit);
                }
            }
        }
        OutputFormat::Json | OutputFormat::Pretty => {
            let format_fn = |items: &[FactMatch]| {
                let mut temp_resp = response.clone();
                temp_resp.results = items.to_vec();
                let json_response = json_response_with_partial_and_performance(temp_resp, false, metrics.cloned());
                if matches!(cli.output, OutputFormat::Pretty) {
                    serde_json::to_string_pretty(&json_response).unwrap_or_default()
                } else {
                    serde_json::to_string(&json_response).unwrap_or_default()
                }
            };

            let (pruned_results, tokens_est, truncated) = truncate_response(results, tokens, format_fn);
            let mut final_resp = response;
            final_resp.results = pruned_results;

            let mut json_response = json_response_with_partial_and_performance(final_resp, false, metrics.cloned());
            json_response.tokens_estimated = tokens_est;
            if truncated {
                json_response.truncated = Some(true);
            }

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
