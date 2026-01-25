pub mod error;
pub mod output;
pub mod output_common;
pub mod query;

use clap::ValueEnum;

/// Sorting mode for search results
#[derive(Clone, Copy, Debug, Default, PartialEq, ValueEnum)]
pub enum SortMode {
    /// Relevance-based scoring (default, LLM-friendly)
    #[default]
    Relevance,
    /// Position-based ordering (faster, pure SQL ORDER BY)
    Position,
}
