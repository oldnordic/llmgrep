# AGENTS.md - llmgrep

Guidelines for agentic coding assistants working on the llmgrep codebase.

## Build/Test/Lint Commands

```bash
# Build the project
cargo build --release

# Run all tests
cargo test

# Run a single test (useful for debugging)
cargo test test_name_here

# Run tests in a specific file
cargo test --test integration_tests

# Check for compilation errors without building
cargo check

# Run clippy lints
cargo clippy -- -D warnings

# Format code
cargo fmt

# Build with specific features
cargo build --features unstable-watch
```

## Code Style Guidelines

### Rust Edition
- Use Rust 2021 edition
- Minimum supported Rust version: latest stable

### Imports
```rust
// Standard library imports first
use std::path::Path;
use std::collections::HashMap;

// Third-party crate imports
use clap::Parser;
use rusqlite::Connection;
use serde::Serialize;

// Internal module imports (group by crate)
use crate::error::LlmError;
use crate::output::SearchResponse;
use crate::query::SearchOptions;
```

### Naming Conventions
- **Types**: PascalCase (`SearchOptions`, `SymbolMatch`)
- **Functions/Methods**: snake_case (`search_symbols`, `build_query`)
- **Constants**: SCREAMING_SNAKE_CASE (`MAX_REGEX_SIZE`)
- **Modules**: snake_case (`query`, `backend`)
- **Error variants**: PascalCase with descriptive names (`DatabaseNotFound`, `InvalidQuery`)

### Error Handling
- Use `thiserror` for error definitions (see `src/error.rs`)
- Error codes follow pattern: `LLM-E###`
- Propagate errors with `?` operator
- Use `anyhow` for CLI error context
- Provide remediation hints on errors

### Types & Structs
- Use `#[derive(Debug)]` on all public types
- Document all public items with `///`
- Use builder pattern for complex configuration
- Prefer `Option<T>` over nullable fields

### Testing
- Unit tests in `src/` files with `#[cfg(test)]`
- Integration tests in `tests/` directory
- Use descriptive test names: `test_symbol_search_with_filters`
- Create temporary databases in tests using `tempfile`

### SQL & Database
- Use parameterized queries with `params![]` or `params_from_iter`
- Check table existence before querying
- Use transactions for multi-step operations
- Index hints for performance-critical queries

### Documentation
- Module-level docs explain purpose and usage
- Function docs include examples where helpful
- Keep CLAUDE.md updated with new features
- Document feature flags in Cargo.toml comments

## Project Structure

```
src/
  main.rs          # CLI entry point
  lib.rs           # Library exports
  error.rs         # Error types (LlmError)
  output.rs        # Response types & formatting
  output_common.rs # Shared output utilities
  algorithm.rs     # Magellan algorithm integration
  ast.rs           # AST queries and context
  platform.rs      # Platform-specific code
  safe_extraction.rs # Safe UTF-8 extraction
  watch_cmd.rs     # Watch command (unstable feature)
  backend/         # Storage backends
    mod.rs         # Backend trait & dispatcher
    sqlite.rs      # SQLite implementation
    native_v3.rs   # Native-V3 implementation
  query/           # Search implementations
    mod.rs         # Module exports
    options.rs     # SearchOptions & related
    builder.rs     # SQL query builders
    symbols.rs     # Symbol search
    references.rs  # Reference search
    calls.rs       # Call search
    chunks.rs      # Code chunk retrieval
    util.rs        # Utility functions
    tests.rs       # Query module tests
```

## Feature Flags

- `default = ["unix"]` - Unix platform support
- `unix` / `windows` - Platform-specific features
- `unstable-watch` - Incomplete watch command
- `native-v3` - Native-V3 backend (disabled by default)

## Dependencies

Key crates:
- `clap` - CLI argument parsing
- `rusqlite` - SQLite database access
- `serde` / `serde_json` - Serialization
- `regex` - Pattern matching
- `thiserror` / `anyhow` - Error handling
- `magellan` / `sqlitegraph` - Code graph libraries

## Common Tasks

Adding a new search filter:
1. Add field to `SearchOptions` in `src/query/options.rs`
2. Update query builder in `src/query/builder.rs`
3. Add CLI flag in `src/main.rs` SearchParams
4. Add test in `tests/integration_tests.rs`

Adding a new output format:
1. Add variant to `OutputFormat` in `src/output.rs`
2. Implement formatter function
3. Update dispatch in `output_symbols()` etc.

## Notes

- Always validate paths before database operations
- Use `Backend::detect_and_open()` for automatic backend selection
- Native-V3 features require both feature flag AND `.v3` database file
- The project uses Magellan v3.0.0+ database schema
