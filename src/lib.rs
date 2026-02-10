//! llmgrep - Semantic code search powered by Magellan.
//!
//! This library provides semantic code search capabilities using Magellan's
//! code graph database. It supports symbol search, reference search, call search,
//! AST-based filtering, and algorithm-based analysis.
//!
//! # Features
//!
//! - **Symbol Search**: Find symbols by name with fuzzy matching
//! - **Reference Search**: Find incoming references to symbols
//! - **Call Search**: Find outgoing function calls from symbols
//! - **AST Filtering**: Filter by AST node kind (functions, loops, etc.)
//! - **Metrics Filtering**: Filter by complexity, fan-in, fan-out
//! - **Algorithm Integration**: Run Magellan algorithms (reachable, dead-code, cycles)
//!
//! # Quick Start
//!
//! ```no_run
//! use llmgrep::Backend;
//! use std::path::Path;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Open a Magellan database
//! let db_path = Path::new(".codemcp/codegraph.db");
//! let backend = Backend::detect_and_open(db_path)?;
//! # Ok(())
//! # }
//! ```
//!
//! See the [`query`](query) module for search functions and options.
//!
//! # Modules
//!
//! - [`algorithm`] - Magellan algorithm integration (reachable, dead-code, cycles)
//! - [`ast`] - AST node queries and context
//! - [`backend`] - Backend abstraction (SQLite, Native-V2)
//! - [`error`] - Error types
//! - [`output`] - Response types and formatting
//! - [`query`] - Search functions and options
//! - [`safe_extraction`] - Safe UTF-8 extraction

pub mod algorithm;
pub mod ast;
pub mod backend;
pub mod error;
pub mod output;
pub mod output_common;
pub mod platform;
pub mod query;
pub mod safe_extraction;
pub mod watch_cmd;

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
