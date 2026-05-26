use crate::cli::Cli;
use llmgrep::error::LlmError;
use llmgrep::output::{
    json_response_with_partial_and_performance, CallSearchResponse, DocsSearchResponse,
    FactsSearchResponse, ImplementsSearchResponse, OutputFormat, PerformanceMetrics,
    ReferenceSearchResponse, SearchResponse,
};
use llmgrep::output_common::{format_partial_footer, format_total_header};

pub fn format_scc_summary(count: usize, supernode_count: usize) -> String {
    if supernode_count == 1 {
        format!("Found {} symbol in 1 SCC", count)
    } else {
        format!("Found {} symbols in {} SCCs", count, supernode_count)
    }
}

pub fn output_symbols(
    cli: &Cli,
    response: SearchResponse,
    partial: bool,
    scc_count: usize,
    metrics: Option<&PerformanceMetrics>,
) -> Result<(), LlmError> {
    match cli.output {
        OutputFormat::Human => {
            if scc_count > 0 {
                println!(
                    "{}",
                    format_scc_summary(response.total_count as usize, scc_count)
                );
            } else if let Some(notice) = &response.notice {
                eprintln!("Warning: {}", notice);
                println!("No symbols found - codebase contains no strongly connected components");
            }
            println!("{}", format_total_header(response.total_count));
            for item in response.results {
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
                println!(
                    "{}:{}:{} {} {} score={}{}",
                    item.span.file_path,
                    item.span.start_line,
                    item.span.start_col,
                    item.name,
                    item.kind,
                    item.score.unwrap_or(0),
                    coverage_str
                );
            }
            if partial {
                println!("{}", format_partial_footer());
            }
        }
        OutputFormat::Json | OutputFormat::Pretty => {
            let json_response =
                json_response_with_partial_and_performance(response, partial, metrics.cloned());
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
            let json_response =
                json_response_with_partial_and_performance(response, partial, metrics.cloned());
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
            let json_response =
                json_response_with_partial_and_performance(response, partial, metrics.cloned());
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
) -> Result<(), LlmError> {
    match cli.output {
        OutputFormat::Human => {
            println!("{}", format_total_header(response.total_count));
            for item in response.results {
                println!(
                    "{}:{}:{} {} impl {} score={}",
                    item.span.file_path,
                    item.span.start_line,
                    item.span.start_col,
                    item.type_name,
                    item.trait_name,
                    item.score.unwrap_or(0)
                );
            }
            if partial {
                println!("{}", format_partial_footer());
            }
        }
        OutputFormat::Json | OutputFormat::Pretty => {
            let json_response =
                json_response_with_partial_and_performance(response, partial, metrics.cloned());
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
) -> Result<(), LlmError> {
    match cli.output {
        OutputFormat::Human => {
            println!("{} documents", response.total_count);
            for item in &response.results {
                let kind = item.source_kind.as_deref().unwrap_or("unknown");
                let title = item.title.as_deref().unwrap_or("<untitled>");
                println!("{} [{}] {}", item.path_or_uri, kind, title);
                if let Some(tags) = &item.tags {
                    println!("  tags: {}", tags);
                }
                if let Some(links) = &item.wikilinks {
                    println!("  wikilinks: {}", links);
                }
            }
        }
        OutputFormat::Json | OutputFormat::Pretty => {
            let json_response =
                json_response_with_partial_and_performance(response, false, metrics.cloned());
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
) -> Result<(), LlmError> {
    match cli.output {
        OutputFormat::Human => {
            println!("{} facts", response.total_count);
            for item in &response.results {
                let subject = item.subject_key.as_deref().unwrap_or("?");
                let pred = item.predicate.as_deref().unwrap_or("?");
                let obj = item.object_key.as_deref().unwrap_or("?");
                let status = item.status.as_deref().unwrap_or("unknown");
                println!("[{}] {} --{}--> {}", status, subject, pred, obj);
                if let Some(stype) = &item.subject_type {
                    println!("  subject_type: {}", stype);
                }
                if let Some(otype) = &item.object_type {
                    println!("  object_type: {}", otype);
                }
            }
        }
        OutputFormat::Json | OutputFormat::Pretty => {
            let json_response =
                json_response_with_partial_and_performance(response, false, metrics.cloned());
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
