//! Query module for symbol, reference, and call search operations.
//!
//! This module provides the core search functionality for llmgrep, including:
//!
//! - Symbol search with fuzzy matching and filtering
//! - Reference search (find incoming edges to symbols)
//! - Call search (find outgoing function calls from symbols)
//! - AST-based filtering by node kind
//! - Metrics-based filtering (complexity, fan-in, fan-out)
//! - Algorithm-based filtering (reachable, dead-code, cycles, etc.)
//!
//! # Search Options
//!
//! All search operations use `SearchOptions` to configure the query.
//! See the `SearchOptions` struct documentation for all available options.
//!
//! # Language Inference
//!
//! The `infer_language` function detects programming language from file extensions,
//! used for labeling symbols with their source language.

// Module declarations
mod backend;
mod builder;
mod calls;
mod chunks;
mod docs;
mod evolve;
mod explore;
mod facts;
mod implements;
pub mod navigate;
mod options;
mod references;
mod semantic;
mod symbols;
pub(crate) mod util;

// Re-exports for backward compatibility
// Options
pub use options::{
    AstOptions, ContextOptions, CoverageFilter, DepthOptions, FqnOptions, MetricsOptions,
    SearchOptions, SnippetOptions,
};

// Backend
pub use backend::{detect_backend_format, BackendFormat};

// Chunks
pub use chunks::{search_chunks_by_span, search_chunks_by_symbol_name, CodeChunk};

// Search functions (public wrappers)
pub use calls::search_calls;
pub use implements::search_implements;
pub use references::search_references;
pub use semantic::{search_semantic, SemanticSearchOptions};
pub use symbols::search_symbols;

mod stats;
pub use stats::{
    run_stats, CoverageStats, DeadCodeStats, HotspotSymbol, StatsResponse, SymbolStats,
};
pub mod telemetry;

// Internal implementations (pub(crate) for use within the crate)
pub(crate) use calls::search_calls_impl;
pub(crate) use docs::search_docs_impl;
pub use docs::DocsSearchOptions;
pub use evolve::{run_evolve, EvolveCandidate, EvolveOptions, EvolveResponse};
pub(crate) use facts::search_facts_impl;
pub use facts::FactsSearchOptions;
pub(crate) use implements::search_implements_impl;
pub(crate) use references::search_references_impl;
pub(crate) use symbols::search_symbols_impl;

// Explore
pub use explore::run_explore;

// Utilities
pub use util::infer_language;

// Internal exports for tests

// Tests
#[cfg(test)]
mod tests;
