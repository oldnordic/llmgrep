pub mod ast;
pub mod error;
pub mod output;
pub mod output_common;
pub mod query;
pub mod safe_extraction;

// Re-export safe extraction functions for testing and external use
pub use safe_extraction::*;

// Re-export AST types for external use
pub use ast::{AstContext, check_ast_table_exists};

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
}
