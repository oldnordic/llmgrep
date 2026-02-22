//! Search options and configuration types.
//!
//! This module defines all the option structs used for configuring search operations.

use crate::algorithm::AlgorithmOptions;
use crate::SortMode;
use std::path::PathBuf;

/// Options for all search functions
#[derive(Debug, Clone)]
pub struct SearchOptions<'a> {
    /// Database path
    pub db_path: &'a std::path::Path,
    /// Search query string
    pub query: &'a str,
    /// Optional path filter
    pub path_filter: Option<&'a PathBuf>,
    /// Optional kind filter (symbols only) - comma-separated values
    pub kind_filter: Option<&'a str>,
    /// Optional language filter (symbols only)
    pub language_filter: Option<&'a str>,
    /// Maximum results to return
    pub limit: usize,
    /// Use regex matching
    pub use_regex: bool,
    /// Candidate limit for filtering
    pub candidates: usize,
    /// Context options
    pub context: ContextOptions,
    /// Snippet options
    pub snippet: SnippetOptions,
    /// FQN options (symbols only)
    pub fqn: FqnOptions,
    /// Include score in results
    pub include_score: bool,
    /// Sorting mode for results
    pub sort_by: SortMode,
    /// Metrics filtering options
    pub metrics: MetricsOptions,
    /// AST filtering options
    pub ast: AstOptions<'a>,
    /// Depth filtering options
    pub depth: DepthOptions<'a>,
    /// Algorithm-based filtering options
    pub algorithm: AlgorithmOptions<'a>,
    /// SymbolId for direct BLAKE3 hash lookup (overrides name-based search)
    pub symbol_id: Option<&'a str>,
    /// FQN pattern filter (LIKE match on canonical_fqn)
    pub fqn_pattern: Option<&'a str>,
    /// Exact FQN filter (exact match on canonical_fqn)
    pub exact_fqn: Option<&'a str>,
}

/// Context extraction options
#[derive(Debug, Clone, Copy, Default)]
pub struct ContextOptions {
    /// Include context
    pub include: bool,
    /// Lines of context before/after
    pub lines: usize,
    /// Maximum context lines
    pub max_lines: usize,
}

/// Snippet extraction options
#[derive(Debug, Clone, Copy, Default)]
pub struct SnippetOptions {
    /// Include snippet
    pub include: bool,
    /// Maximum snippet bytes
    pub max_bytes: usize,
}

/// FQN inclusion options (symbols only)
#[derive(Debug, Clone, Copy, Default)]
pub struct FqnOptions {
    /// Include basic FQN
    pub fqn: bool,
    /// Include canonical FQN
    pub canonical_fqn: bool,
    /// Include display FQN
    pub display_fqn: bool,
}

/// Metrics-based filtering options
#[derive(Debug, Clone, Copy, Default)]
pub struct MetricsOptions {
    /// Minimum cyclomatic complexity
    pub min_complexity: Option<usize>,
    /// Maximum cyclomatic complexity
    pub max_complexity: Option<usize>,
    /// Minimum fan-in (incoming references)
    pub min_fan_in: Option<usize>,
    /// Minimum fan-out (outgoing calls)
    pub min_fan_out: Option<usize>,
}

/// AST-based filtering options
#[derive(Debug, Clone, Default)]
pub struct AstOptions<'a> {
    /// Filter by AST node kind(s) - can be multiple kinds
    /// When --ast-kind is specified with shorthands or comma-separated values,
    /// this contains the expanded list of node kind strings.
    pub ast_kinds: Vec<String>,
    /// Enable enriched AST context calculation (depth, parent_kind, children, decision_points)
    pub with_ast_context: bool,
    /// Phantom data for lifetime parameter (for future use if needed)
    pub _phantom: std::marker::PhantomData<&'a ()>,
}

impl<'a> AstOptions<'a> {
    /// Create empty AstOptions
    pub fn new() -> Self {
        Self {
            ast_kinds: Vec::new(),
            with_ast_context: false,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Check if any AST kinds are specified
    pub fn has_ast_kinds(&self) -> bool {
        !self.ast_kinds.is_empty()
    }

    /// Get the first AST kind (for backward compatibility)
    pub fn first_ast_kind(&self) -> Option<&str> {
        self.ast_kinds.first().map(|s| s.as_str())
    }
}

/// Depth-based filtering options
#[derive(Debug, Clone, Copy, Default)]
pub struct DepthOptions<'a> {
    /// Minimum nesting depth (decision points only)
    pub min_depth: Option<usize>,
    /// Maximum nesting depth (decision points only)
    pub max_depth: Option<usize>,
    /// Find nodes within parent of this kind (--inside)
    pub inside: Option<&'a str>,
    /// Find parents containing nodes of this kind (--contains)
    pub contains: Option<&'a str>,
}
