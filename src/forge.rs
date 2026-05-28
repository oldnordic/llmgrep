//! High-level API for external consumers (forge, agents, tool integrations).
//!
//! This module provides convenience functions that wrap `Backend` and
//! `SearchOptions` construction into single-call functions. Designed for
//! programmatic use where the caller doesn't want to manage option structs
//! or backend lifecycle directly.
//!
//! # Example
//!
//! ```no_run
//! use std::path::Path;
//! use llmgrep::forge;
//!
//! let db = Path::new(".magellan/myproject.db");
//!
//! // Search for symbols by name
//! let matches = forge::search_symbols("my_function", db, 10).unwrap();
//! for m in &matches {
//!     println!("{} ({}) at {}:{}", m.name, m.kind, m.span.file_path, m.span.start_line);
//! }
//!
//! // Find all references to a symbol
//! let refs = forge::search_references("my_function", db, 50).unwrap();
//!
//! // Find all calls from a function
//! let calls = forge::search_calls("my_function", db, 50).unwrap();
//!
//! // Lookup a symbol by fully-qualified name
//! let sym = forge::lookup_symbol("my_crate::my_module::my_function", db).unwrap();
//! ```

use crate::backend::Backend;
use crate::error::LlmError;
use crate::output::{CallMatch, ReferenceMatch, SymbolMatch};
use crate::query::{
    AstOptions, ContextOptions, DepthOptions, FqnOptions, MetricsOptions, SearchOptions,
    SnippetOptions,
};
use crate::SortMode;
use std::path::Path;

/// Search for symbols by name or pattern.
///
/// Opens the database, runs a symbol search with default options, and returns
/// up to `limit` matches. Supports regex patterns.
///
/// # Arguments
///
/// * `query` - Symbol name or regex pattern
/// * `db_path` - Path to the Magellan database
/// * `limit` - Maximum number of results
///
/// # Errors
///
/// Returns `LlmError` if the database cannot be opened or the query fails.
pub fn search_symbols(
    query: &str,
    db_path: &Path,
    limit: usize,
) -> Result<Vec<SymbolMatch>, LlmError> {
    let backend = Backend::detect_and_open(db_path)?;
    let options = SearchOptions {
        db_path,
        query,
        path_filter: None,
        kind_filter: None,
        language_filter: None,
        limit,
        use_regex: false,
        candidates: limit * 10,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions {
            fqn: true,
            canonical_fqn: true,
            display_fqn: false,
        },
        include_score: false,
        sort_by: SortMode::default(),
        metrics: MetricsOptions::default(),
        ast: AstOptions::default(),
        depth: DepthOptions::default(),
        algorithm: Default::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
        coverage_filter: None,
    };
    let (response, _, _) = backend.search_symbols(options)?;
    Ok(response.results)
}

/// Search for symbols using a regex pattern.
///
/// Like [`search_symbols`] but treats the query as a regular expression.
pub fn search_symbols_regex(
    pattern: &str,
    db_path: &Path,
    limit: usize,
) -> Result<Vec<SymbolMatch>, LlmError> {
    let backend = Backend::detect_and_open(db_path)?;
    let options = SearchOptions {
        db_path,
        query: pattern,
        path_filter: None,
        kind_filter: None,
        language_filter: None,
        limit,
        use_regex: true,
        candidates: limit * 10,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions {
            fqn: true,
            canonical_fqn: true,
            display_fqn: false,
        },
        include_score: false,
        sort_by: SortMode::default(),
        metrics: MetricsOptions::default(),
        ast: AstOptions::default(),
        depth: DepthOptions::default(),
        algorithm: Default::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
        coverage_filter: None,
    };
    let (response, _, _) = backend.search_symbols(options)?;
    Ok(response.results)
}

/// Search for references to a symbol.
///
/// Returns all incoming references (call sites, type usages, etc.) to the
/// named symbol across the codebase.
///
/// # Arguments
///
/// * `symbol_name` - Symbol name to find references for
/// * `db_path` - Path to the Magellan database
/// * `limit` - Maximum number of results
///
/// # Errors
///
/// Returns `LlmError` if the database cannot be opened or the query fails.
pub fn search_references(
    symbol_name: &str,
    db_path: &Path,
    limit: usize,
) -> Result<Vec<ReferenceMatch>, LlmError> {
    let backend = Backend::detect_and_open(db_path)?;
    let options = SearchOptions {
        db_path,
        query: symbol_name,
        path_filter: None,
        kind_filter: None,
        language_filter: None,
        limit,
        use_regex: false,
        candidates: limit * 10,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions::default(),
        include_score: false,
        sort_by: SortMode::default(),
        metrics: MetricsOptions::default(),
        ast: AstOptions::default(),
        depth: DepthOptions::default(),
        algorithm: Default::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
        coverage_filter: None,
    };
    let (response, _) = backend.search_references(options)?;
    Ok(response.results)
}

/// Search for calls from (or to) a symbol.
///
/// Returns call relationships involving the named symbol.
///
/// # Arguments
///
/// * `symbol_name` - Symbol name to find calls for
/// * `db_path` - Path to the Magellan database
/// * `limit` - Maximum number of results
///
/// # Errors
///
/// Returns `LlmError` if the database cannot be opened or the query fails.
pub fn search_calls(
    symbol_name: &str,
    db_path: &Path,
    limit: usize,
) -> Result<Vec<CallMatch>, LlmError> {
    let backend = Backend::detect_and_open(db_path)?;
    let options = SearchOptions {
        db_path,
        query: symbol_name,
        path_filter: None,
        kind_filter: None,
        language_filter: None,
        limit,
        use_regex: false,
        candidates: limit * 10,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions::default(),
        include_score: false,
        sort_by: SortMode::default(),
        metrics: MetricsOptions::default(),
        ast: AstOptions::default(),
        depth: DepthOptions::default(),
        algorithm: Default::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
        coverage_filter: None,
    };
    let (response, _) = backend.search_calls(options)?;
    Ok(response.results)
}

/// Lookup a symbol by fully-qualified name.
///
/// Uses the `Backend::lookup` method for precise FQN-based resolution.
///
/// # Arguments
///
/// * `fqn` - Fully-qualified symbol name (e.g. "my_crate::Module::function")
/// * `db_path` - Path to the Magellan database
///
/// # Errors
///
/// Returns `LlmError` if the database cannot be opened or the symbol is not found.
pub fn lookup_symbol(fqn: &str, db_path: &Path) -> Result<SymbolMatch, LlmError> {
    let backend = Backend::detect_and_open(db_path)?;
    backend.lookup(fqn, db_path.to_str().unwrap_or("."))
}

/// Search for symbols filtered by language.
///
/// Like [`search_symbols`] but restricts results to a specific language.
pub fn search_symbols_by_language(
    query: &str,
    language: &str,
    db_path: &Path,
    limit: usize,
) -> Result<Vec<SymbolMatch>, LlmError> {
    let backend = Backend::detect_and_open(db_path)?;
    let options = SearchOptions {
        db_path,
        query,
        path_filter: None,
        kind_filter: None,
        language_filter: Some(language),
        limit,
        use_regex: false,
        candidates: limit * 10,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions {
            fqn: true,
            canonical_fqn: true,
            display_fqn: false,
        },
        include_score: false,
        sort_by: SortMode::default(),
        metrics: MetricsOptions::default(),
        ast: AstOptions::default(),
        depth: DepthOptions::default(),
        algorithm: Default::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
        coverage_filter: None,
    };
    let (response, _, _) = backend.search_symbols(options)?;
    Ok(response.results)
}
