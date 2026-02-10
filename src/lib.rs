pub mod algorithm;
pub mod ast;
pub mod backend;
pub mod error;
pub mod output;
pub mod output_common;
pub mod platform;
pub mod query;
pub mod safe_extraction;

// Re-export safe extraction functions for testing and external use
pub use safe_extraction::*;

// Re-export query functions for external use
pub use query::infer_language;

// Re-export algorithm types for external use
pub use algorithm::{
    AlgorithmOptions, parse_symbol_set_file, resolve_fqn_to_symbol_id, run_magellan_algorithm,
    SymbolSet, SymbolSetStrategy,
};

// Re-export AST types for external use
pub use ast::{
    AstContext, calculate_decision_depth, check_ast_table_exists, expand_shorthand,
    expand_shorthand_with_language, expand_shorthands, get_node_kinds_for_language,
    get_supported_languages,
};

// Re-export backend types for external use
pub use backend::Backend;

// Re-export output types for external use
pub use output::PerformanceMetrics;

use clap::ValueEnum;

/// Sorting mode for search results
#[derive(Clone, Copy, Debug, Default, PartialEq, ValueEnum)]
pub enum SortMode {
    /// Relevance-based scoring (default, LLM-friendly)
    #[default]
    Relevance,
    /// Position-based ordering (faster, pure SQL ORDER BY)
    Position,
    /// Sort by fan-in (incoming references) descending
    FanIn,
    /// Sort by fan-out (outgoing calls) descending
    FanOut,
    /// Sort by cyclomatic complexity descending
    Complexity,
    /// Sort by AST nesting depth descending (deepest first)
    NestingDepth,
    /// Sort by AST complexity (decision points) descending
    AstComplexity,
}
