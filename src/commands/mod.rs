pub mod ast;
pub mod complete;
pub mod evolve;
pub mod export_symbols;
pub mod find_ast;
pub mod lookup;
pub mod search;
pub mod stats;
pub mod vector;

#[cfg(feature = "unstable-watch")]
pub mod watch;

pub use ast::run_ast;
pub use complete::run_complete;
pub use evolve::run_evolve_cmd;
pub use export_symbols::run_export_symbols;
pub use find_ast::run_find_ast;
pub use lookup::run_lookup;
pub use search::dispatch_search;
pub use stats::run_stats_cmd;
pub use vector::{run_vector_create, run_vector_search};

#[cfg(feature = "unstable-watch")]
pub use watch::run_watch;
