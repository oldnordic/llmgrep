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
mod options;
mod references;
mod symbols;
mod util;

// Re-exports for backward compatibility
// Options
pub use options::{
    AstOptions, ContextOptions, DepthOptions, FqnOptions, MetricsOptions, SearchOptions,
    SnippetOptions,
};

// Backend
pub use backend::{detect_backend_format, BackendFormat};

// Chunks
pub use chunks::{search_chunks_by_span, search_chunks_by_symbol_name, CodeChunk};

// Search functions (public wrappers)
pub use calls::search_calls;
pub use references::search_references;
pub use symbols::search_symbols;

// Internal implementations (pub(crate) for use within the crate)
pub(crate) use calls::search_calls_impl;
pub(crate) use references::search_references_impl;
pub(crate) use symbols::search_symbols_impl;

// Utilities
pub use util::infer_language;

// Internal exports for tests
pub(crate) use builder::{
    build_call_query, build_reference_query, build_search_query,
};
pub(crate) use util::{
    like_pattern, like_prefix, load_file, normalize_kind_label, score_match, FileCache,
};

// Tests
#[cfg(test)]
mod tests;
