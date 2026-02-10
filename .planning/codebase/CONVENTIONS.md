# Coding Conventions

**Analysis Date:** 2026-02-10

## Naming Patterns

**Files:**
- `snake_case.rs` for all Rust source files
- `snake_case_tests.rs` for integration tests in `tests/` directory
- Module directories use `snake_case` with `mod.rs` if needed

**Functions:**
- `snake_case` for all functions and methods
- Pub functions use descriptive names: `search_symbols`, `check_ast_table_exists`, `expand_shorthands`
- Internal helpers may be shorter: `normalize_kind`, `infer_language`

**Variables:**
- `snake_case` for all variables
- Short names for loop iterators: `conn`, `row`, `id`, `kind`
- Descriptive names for structs: `search_options`, `ast_context`, `symbol_match`

**Types:**
- `PascalCase` for structs, enums, and type aliases
- `PascalCase` for traits
- Module-level constants: `SCREAMING_SNAKE_CASE` (e.g., `MAX_REGEX_SIZE`, `SCHEMA_VERSION`)

## Code Style

**Formatting:**
- Standard `rustfmt` formatting (no explicit config detected)
- 4-space indentation (Rust default)
- Line length: Default Rust limits (no custom `rustfmt.toml`)

**Linting:**
- No explicit `clippy.toml` configuration
- Uses `#[allow(dead_code)]` and `#[allow(unused_imports)]` attributes strategically
- `#[allow(clippy::too_many_arguments)]` used for functions with many params

**Comments:**
- Module-level `//!` doc comments at top of files
- Function-level `///` doc comments for public APIs
- `///` doc comments include Example sections with `# Ok<(), ...>` signature
- Inline `//` comments for explanations

## Import Organization

**Order:**
1. Standard library: `use std::...`
2. External crates: `use regex::...`, `use rusqlite::...`
3. Internal modules: `use crate::...`
4. `#[allow(unused_imports)]` before suppressed imports

**Path Aliases:**
- No explicit path aliases configured
- Re-exports in `lib.rs`: `pub use safe_extraction::*;`

**Grouping:**
```rust
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use regex::{Regex, RegexBuilder};
use rusqlite::{params_from_iter, Connection, ErrorCode, OpenFlags, ToSql};
use serde::Deserialize;

use crate::algorithm::{...};
use crate::ast::{...};
use crate::error::LlmError;
```

## Error Handling

**Patterns:**
- Central error type: `LlmError` enum in `src/error.rs`
- Uses `thiserror` for error derives: `#[derive(Error, Debug)]`
- Error conversion via `From` trait: `#[error("...")]` attributes
- `?` operator for propagation throughout

**Error structure:**
```rust
#[derive(Error, Debug)]
pub enum LlmError {
    #[error("Database not found: {path}")]
    DatabaseNotFound { path: String },

    #[error("IoError: {0}")]
    IoError(#[from] std::io::Error),

    // With custom error codes
    #[error("LLM-E105: Magellan CLI not found...")]
    MagellanNotFound,
}
```

**Error codes:**
- Structured error codes: `LLM-E001` to `LLM-E999`
- `error_code()` method returns code for each error variant
- `remediation()` method provides fix hints

**Validation:**
- Input validation at CLI layer (`src/main.rs`)
- Path validation via `validate_path()` function
- Query validation: empty checks, regex size limits

## Logging

**Framework:** No structured logging framework
- `eprintln!` for user-facing warnings
- `println!` for output
- Stderr for metrics when `--show-metrics` flag is used

**Patterns:**
```rust
// User-facing warnings
eprintln!("Warning: Path enumeration hit bounds...");
eprintln!("Note: No execution paths found from '{from}'");

// Metrics (stderr)
if cli.show_metrics {
    eprintln!("Performance metrics:");
    eprintln!("  Backend detection: {}ms", backend_detection_ms);
}
```

## Comments

**When to Comment:**
- All public APIs must have `///` doc comments
- Complex SQL queries explained before execution
- Non-obvious algorithms explained inline
- Edge cases and safety considerations documented

**JSDoc/TSDoc:**
- Rustdoc format with `///` for documentation
- Example sections with code: `/// # Example\n/// \`\`\`no_run`
- Panics section in docs when applicable: `/// # Panics`

**Comment style:**
```rust
/// Search symbols by name with optional filters.
///
/// # Arguments
///
/// * `db_path` - Path to the Magellan code graph database
/// * `query` - Search query string
///
/// # Returns
///
/// A `SearchResponse` containing matching symbols.
///
/// # Errors
///
/// Returns `LlmError::DatabaseNotFound` if database cannot be opened.
///
/// # Example
///
/// ```no_run
/// use llmgrep::query::search_symbols;
/// # Ok::<(), llmgrep::error::LlmError>(())
/// ```
pub fn search_symbols(options: SearchOptions) -> Result<...>
```

## Function Design

**Size:**
- No strict size limit
- Large functions broken into helper functions when >100 lines
- CLI dispatch functions are long due to parameter handling

**Parameters:**
- Many parameters bundled into structs: `SearchOptions`, `ContextOptions`, `MetricsOptions`
- Builder pattern not used - direct struct construction
- Options structs use `&'a str` lifetime for borrowed strings

**Return Values:**
- `Result<T, LlmError>` for fallible operations
- `Option<T>` for nullable returns
- Tuple returns for multiple values: `(SearchResponse, bool)` for (response, partial)
- Newtype wrappers not used

## Module Design

**Exports:**
```rust
// In lib.rs - explicit re-exports for public API
pub mod algorithm;
pub mod ast;
pub mod backend;
pub mod error;
pub mod output;
pub mod query;

// Re-export commonly used types
pub use safe_extraction::*;
pub use algorithm::{AlgorithmOptions, SymbolSet, SymbolSetStrategy};
pub use ast::{AstContext, check_ast_table_exists, ...};
```

**Barrel Files:**
- `src/lib.rs` serves as main barrel file
- `src/backend/mod.rs` for backend module

**Feature Gates:**
```rust
#[cfg(feature = "native-v2")]
{
    // Native-V2 backend code
}
#[cfg(not(feature = "native-v2"))]
{
    // Fallback or error
}
```

## Struct Design

**Patterns:**
- Public fields for data structs: `pub struct SearchOptions { pub db_path: ..., pub query: ..., }`
- Builder pattern not used
- `#[derive(Debug, Clone)]` on most structs
- `#[derive(Serialize)]` for output structs
- `#[derive(Deserialize)]` for internal data structs

**Serialization:**
```rust
#[derive(Serialize)]
pub struct SymbolMatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fqn: Option<String>,
}
```

## Constants

**Declaration:**
```rust
const MAX_REGEX_SIZE: usize = 10_000;
const SCHEMA_VERSION: &str = "1.0.0";
const SYMBOL_SET_TEMP_TABLE_THRESHOLD: usize = 1000;
```

**Usage:**
- Named constants for magic numbers
- Threshold values for algorithm decisions

## Async/Promise Patterns

**Not applicable** - This is a synchronous Rust CLI tool.

## SQL Patterns

**Parameter binding:**
```rust
// Use params! macro from rusqlite
conn.execute("INSERT INTO table (col) VALUES (?)", params![value])?;

// For prepared statements
let mut stmt = conn.prepare("SELECT ... WHERE col = ?")?;
stmt.query_map([arg], |row| { ... })?;
```

**String interpolation:**
- Never interpolate user input into SQL
- Only interpolate trusted table/column identifiers
```rust
let sql = format!("CREATE TEMP TABLE {} (symbol_id TEXT PRIMARY KEY)", table_name);
conn.execute(&sql, []).map_err(LlmError::SqliteError)?;
```

---

*Convention analysis: 2026-02-10*
