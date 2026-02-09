# Phase 18: SqliteBackend Refactor - Research

**Researched:** 2026-02-09
**Domain:** Rust trait implementation, SQL query refactoring, zero-breaking-change migration
**Confidence:** HIGH

## Summary

Phase 18 is a pure code-movement refactoring: move existing SQL query logic from `src/query.rs` into `SqliteBackend` trait implementation in `src/backend/sqlite.rs`. This is NOT a new feature development - it's an architectural reorganization to complete the dual-backend foundation established in Phase 17. The key constraint is **zero logic changes**: all existing tests must pass without modification, and output must be identical to the pre-refactor implementation.

**Primary recommendation:** Implement a three-step refactoring strategy: (1) Move helper functions from `query.rs` to `sqlite.rs` as private methods, (2) Create internal `_search_symbols_impl()` and similar functions that take `&Connection` parameter, (3) Implement `BackendTrait` methods by calling the internal functions with `&self.conn`. Run full test suite after each step to catch regressions immediately. The refactoring is low-risk because it's pure code movement - no SQL queries change, no business logic changes, only code organization changes.

The research confirms this is a straightforward refactoring. All three public functions (`search_symbols`, `search_references`, `search_calls`) are already implemented in `query.rs` with clear structure. The helper functions (`build_search_query`, `build_reference_query`, `build_call_query`, `score_match`, `span_id`, etc.) can be moved wholesale. The only adaptation needed is changing from `Connection::open_with_flags()` calls to using `&self.conn` from the backend struct.

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| **rusqlite** | 0.31 | SQLite database access | Already in dependencies. All existing code uses `Connection`, `params_from_iter`, `prepare_cached`. No changes needed. |
| **regex** | 1.10 | Pattern matching for search | Already in dependencies. Used for `--regex` flag support. No changes needed. |
| **serde_json** | 1.0 | JSON parsing for graph entities | Already in dependencies. Used for extracting symbol/reference/call data from JSON column. No changes needed. |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| **sha2** | 0.10 | SHA-256 for span/match IDs | Already in dependencies. Used for generating unique identifiers. No changes needed. |
| **hex** | 0.4 | Hex encoding for hashes | Already in dependencies. Used for encoding hash digests. No changes needed. |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| **Direct code movement** | Rewrite queries from scratch | Worse - introduces bugs, harder to verify. Code movement preserves exact behavior. |
| **Internal module functions** | Make query.rs functions public | Worse - exposes implementation details. Private internal functions keep clean API. |

**Installation:**
```bash
# No new dependencies needed
# All existing dependencies remain unchanged
```

## Architecture Patterns

### Recommended Project Structure
```
src/
├── backend/
│   ├── mod.rs           # BackendTrait, Backend enum, detect_and_open()
│   ├── sqlite.rs        # SqliteBackend with moved query logic (THIS PHASE)
│   └── native_v2.rs     # NativeV2Backend stub (Phase 19)
├── query.rs             # Existing public functions (delegate to Backend)
├── algorithm.rs         # Algorithm filtering (unchanged)
├── ast.rs               # AST context queries (unchanged)
├── safe_extraction.rs   # UTF-8 safety helpers (unchanged)
└── main.rs              # CLI (unchanged - uses Backend::detect_and_open)
```

### Pattern 1: Code Movement with Internal Functions
**What:** Move query logic into backend by creating internal `_impl()` functions that take `&Connection`, then wrap with trait methods.
**When to use:** Refactoring existing code into trait implementations without changing logic.
**Example:**
```rust
// Source: /home/feanor/Projects/llmgrep/src/backend/sqlite.rs (planned implementation)

impl SqliteBackend {
    // Internal implementation taking &Connection
    fn search_symbols_internal(
        conn: &Connection,
        options: SearchOptions,
    ) -> Result<(SearchResponse, bool, bool), LlmError> {
        // Move all logic from query::search_symbols here
        // Use `conn` parameter instead of opening new connection
        // ... (all existing SQL queries, result processing, etc.)
    }
}

impl BackendTrait for SqliteBackend {
    fn search_symbols(
        &self,
        options: SearchOptions,
    ) -> Result<(SearchResponse, bool, bool), LlmError> {
        // Delegate to internal implementation with &self.conn
        Self::search_symbols_internal(&self.conn, options)
    }
}
```

### Pattern 2: Gradual Migration with Delegation
**What:** Keep `query.rs` public functions working during refactor, have them delegate to Backend.
**When to use:** Maintaining testability during incremental refactoring.
**Example:**
```rust
// Source: /home/feanor/Projects/llmgrep/src/query.rs (planned transition)

// Existing public function - delegate to Backend
pub fn search_symbols(options: SearchOptions) -> Result<(SearchResponse, bool, bool), LlmError> {
    let backend = SqliteBackend::open(options.db_path)?;
    backend.search_symbols(options)
}

// After refactor: query.rs functions become thin wrappers
// This allows tests to keep using query::search_symbols()
```

### Pattern 3: Helper Function Movement
**What:** Move all helper functions from `query.rs` to `sqlite.rs` as private methods.
**When to use:** Helper functions are only used by the backend implementation.
**Example:**
```rust
// Source: /home/feanor/Projects/llmgrep/src/query.rs (existing helpers to move)

// Functions to move from query.rs to sqlite.rs:
// - build_search_query()
// - build_reference_query()
// - build_call_query()
// - like_pattern()
// - like_prefix()
// - score_match()
// - span_id()
// - match_id()
// - snippet_from_file()
// - span_context_from_file()
// - load_file()
// - search_chunks_by_symbol_name()
// - search_chunks_by_span()

// All become private functions in sqlite.rs
// Used only by internal _impl() functions
```

### Anti-Patterns to Avoid
- **Changing SQL queries:** Any modification to SQL logic breaks parity. Move code exactly as-is.
- **Modifying result structures:** All response types must remain identical. Changes break JSON output.
- **Adding new features:** This is a refactoring phase only. New functionality belongs in future phases.
- **Removing query.rs functions:** Keep public API during refactor. Deprecate after migration complete.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Connection pooling | Custom pool management | rusqlite::Connection (one per backend) | Single-threaded CLI doesn't need pooling. Each Backend owns one connection. |
| SQL query builders | Custom query construction | Keep existing `build_*_query()` functions | Existing code is correct. Reuse, don't rewrite. |
| Result pagination | Custom offset/limit logic | Keep existing SQL LIMIT/OFFSET | Current approach works. No changes needed. |
| File caching | Custom cache invalidation | Keep existing `FileCache` struct | Current HashMap-based cache is simple and effective. |

**Key insight:** This refactoring is about code organization, not new functionality. Every line of business logic already exists and works correctly. The goal is to move it into the right structure without breaking anything.

## Common Pitfalls

### Pitfall 1: Accidentally Changing Connection Handling
**What goes wrong:** Refactoring changes how database connections are opened/closed, causing "database is locked" errors or performance regressions.
**Why it happens:** Original code opens connection per query, refactored code might try to share connections incorrectly.
**How to avoid:** Open connection once in `SqliteBackend::open()`, store in struct, use same reference for all queries. DO NOT call `Connection::open_with_flags()` inside trait methods.
**Warning signs:** New `Connection::open()` calls in trait methods, connection lifetime errors, "database is locked" runtime errors.

### Pitfall 2: Breaking Test Isolation
**What goes wrong:** Tests that relied on `query::search_symbols()` now fail because function signature or behavior changed.
**Why it happens:** Tests import `llmgrep::query::search_symbols` directly. If this changes, tests break.
**How to avoid:** Keep `query.rs` public functions as thin wrappers during refactor. Only remove after explicit test migration. Run `cargo test` after each code movement step.
**Warning signs:** Test compilation errors, "function not found in crate::query", different output from test assertions.

### Pitfall 3: Misplacing Helper Functions
**What goes wrong:** Helper functions moved to wrong location, causing visibility issues or code duplication.
**Why it happens:** Unclear which helpers are backend-specific vs. shared utilities.
**How to avoid:**
- Backend-specific helpers: `build_*_query()`, `score_match()`, `span_id()`, `like_pattern()` → move to `sqlite.rs`
- Shared utilities: `extract_symbol_content_safe()`, file I/O helpers → stay in `safe_extraction.rs`
- Algorithm functions: `apply_algorithm_filters()` → stay in `algorithm.rs`
**Warning signs:** Compiler errors about missing functions, circular dependencies, duplicate code.

### Pitfall 4: Forgetting to Move File I/O Caching
**What goes wrong:** File content cache logic not properly migrated, causing performance regression or redundant file reads.
**Why it happens:** `FileCache` struct and `load_file()` function are easy to overlook.
**How to avoid:** Move entire file caching logic (`FileCache` struct, `load_file()`, `snippet_from_file()`, `span_context_from_file()`) to `sqlite.rs` as private helpers.
**Warning signs:** Slower query performance, multiple file reads for same query, eprintln warnings about file reading.

### Pitfall 5: Lifetime Errors with `&Connection`
**What goes wrong:** Compiler errors about borrowed values not living long enough.
**Why it happens:** Trying to store references in structs or return them from functions incorrectly.
**How to avoid:** All helper functions take `&Connection` as parameter (not `&self`). Return owned types (`Vec`, `String`, structs), never references. Use `serde_json::from_str` to parse JSON, not direct borrowing.
**Warning signs:** Lifetime parameters in function signatures, borrowed data errors, "does not live long enough" messages.

## Code Examples

Verified patterns from existing source code:

### Existing search_symbols Structure (to be moved)
```rust
// Source: /home/feanor/Projects/llmgrep/src/query.rs (lines 345-956)

pub fn search_symbols(options: SearchOptions) -> Result<(SearchResponse, bool, bool), LlmError> {
    // 1. Open connection (lines 346-364)
    let conn = match Connection::open_with_flags(options.db_path, OpenFlags::SQLITE_OPEN_READ_ONLY) {
        Ok(conn) => conn,
        Err(rusqlite::Error::SqliteFailure(err, msg)) => match err.code {
            ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase => {
                return Err(LlmError::DatabaseCorrupted { reason: ... });
            }
            ErrorCode::CannotOpen => {
                return Err(LlmError::DatabaseNotFound { path: ... });
            }
            _ => return Err(LlmError::from(rusqlite::Error::SqliteFailure(err, msg))),
        },
        Err(e) => return Err(LlmError::from(e)),
    };

    // 2. Validate schema (lines 366-385)
    conn.query_row("SELECT name FROM sqlite_master WHERE type='table' LIMIT 1", [], |_| Ok(()))
        .map_err(|e| match e {
            rusqlite::Error::SqliteFailure(err, ref msg) => match err.code {
                ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase => LlmError::DatabaseCorrupted { ... },
                _ => LlmError::from(e),
            },
            other => LlmError::from(other),
        })?;

    // 3. Apply algorithm filters (lines 388-399)
    let (algorithm_symbol_ids, supernode_map, paths_bounded) = if options.algorithm.is_active() {
        apply_algorithm_filters(options.db_path, &options.algorithm)?
    } else {
        (Vec::new(), HashMap::new(), false)
    };

    // 4. Build query (lines 401-421)
    let (sql, params, symbol_set_strategy) = build_search_query(...);

    // 5. Check AST table (lines 424-428)
    let has_ast_table = check_ast_table_exists(&conn)?;

    // 6. Rebuild with AST settings if needed (lines 430-454)
    let (sql, params, symbol_set_strategy) = if !options.ast.ast_kinds.is_empty() || ... {
        build_search_query(..., has_ast_table, &options.ast.ast_kinds, ...)
    } else {
        (sql, params, symbol_set_strategy)
    };

    // 7. Create temp table for symbol set filter (lines 457-465)
    let temp_table_name = if symbol_set_strategy == SymbolSetStrategy::TempTable {
        if let Some(ids) = symbol_set_filter {
            Some(create_symbol_set_temp_table(&conn, ids)?)
        } else {
            None
        }
    } else {
        None
    };

    // 8. Execute query and process results (lines 467-822)
    let mut stmt = conn.prepare_cached(&sql)?;
    let mut rows = stmt.query(params_from_iter(params))?;
    // ... result processing loop ...

    // 9. Cleanup temp table (lines 938-940)
    if let Some(table_name) = temp_table_name {
        let _ = conn.execute(&format!("DROP TABLE IF EXISTS {}", table_name), []);
    }

    // 10. Return response (lines 942-955)
    Ok((SearchResponse { ... }, partial, paths_bounded))
}

// REFACTOR: Move to SqliteBackend::search_symbols_internal(&conn, options)
// REMOVE: Connection::open_with_flags() call (use &self.conn instead)
// KEEP: All other logic exactly as-is
```

### Target Structure in sqlite.rs
```rust
// Source: /home/feanor/Projects/llmgrep/src/backend/sqlite.rs (planned implementation)

use crate::algorithm::{apply_algorithm_filters, AlgorithmOptions};
use crate::ast::{check_ast_table_exists, AstContext};
use crate::error::LlmError;
use crate::output::{CallSearchResponse, ReferenceSearchResponse, SearchResponse};
use crate::query::SearchOptions;
use crate::safe_extraction::extract_symbol_content_safe;
use crate::SortMode;
use regex::{Regex, RegexBuilder};
use rusqlite::{params_from_iter, Connection, ErrorCode, ToSql};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

// Move all helper types and functions from query.rs
// CodeChunk, SymbolNodeData, ReferenceNodeData, CallNodeData
// infer_language, normalize_kind_label
// build_search_query, build_reference_query, build_call_query
// like_pattern, like_prefix, referenced_symbol_from_name
// score_match, span_id, match_id
// FileCache, load_file, snippet_from_file, span_context_from_file
// search_chunks_by_symbol_name, search_chunks_by_span

impl SqliteBackend {
    // Internal implementation - ALL logic from query::search_symbols
    fn search_symbols_internal(
        conn: &Connection,
        options: SearchOptions,
    ) -> Result<(SearchResponse, bool, bool), LlmError> {
        // Validate schema (conn already open)
        conn.query_row("SELECT name FROM sqlite_master WHERE type='table' LIMIT 1", [], |_| Ok(()))
            .map_err(|e| match e {
                rusqlite::Error::SqliteFailure(err, ref msg) => match err.code {
                    ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase => LlmError::DatabaseCorrupted { ... },
                    _ => LlmError::from(e),
                },
                other => LlmError::from(other),
            })?;

        // ... rest of logic EXACTLY as in query.rs ...
        // Only change: use `conn` parameter instead of opening new connection

        Ok((SearchResponse { ... }, partial, paths_bounded))
    }
}

impl super::BackendTrait for SqliteBackend {
    fn search_symbols(
        &self,
        options: SearchOptions,
    ) -> Result<(SearchResponse, bool, bool), LlmError> {
        Self::search_symbols_internal(&self.conn, options)
    }

    fn search_references(
        &self,
        options: SearchOptions,
    ) -> Result<(ReferenceSearchResponse, bool), LlmError> {
        Self::search_references_internal(&self.conn, options)
    }

    fn search_calls(
        &self,
        options: SearchOptions,
    ) -> Result<(CallSearchResponse, bool), LlmError> {
        Self::search_calls_internal(&self.conn, options)
    }

    fn ast(
        &self,
        file: &Path,
        position: Option<usize>,
        limit: usize,
    ) -> Result<serde_json::Value, LlmError> {
        // Shell out to magellan ast (keep existing behavior from main.rs)
        use std::process::Command;

        let mut cmd = Command::new("magellan");
        cmd.arg("ast")
            .arg("--db")
            .arg(&self.db_path) // Need to store db_path in SqliteBackend
            .arg("--file")
            .arg(file)
            .arg("--output")
            .arg("json");

        if let Some(pos) = position {
            cmd.arg("--position").arg(pos.to_string());
        }

        // ... execute and parse output ...
    }

    fn find_ast(&self, kind: &str) -> Result<serde_json::Value, LlmError> {
        // Shell out to magellan find-ast (keep existing behavior from main.rs)
        use std::process::Command;

        let mut cmd = Command::new("magellan");
        cmd.arg("find-ast")
            .arg("--db")
            .arg(&self.db_path)
            .arg("--kind")
            .arg(kind)
            .arg("--output")
            .arg("json");

        // ... execute and parse output ...
    }
}
```

### SqliteBackend Enhancement: Store db_path
```rust
// Source: /home/feanor/Projects/llmgrep/src/backend/sqlite.rs (needs modification)

use std::path::PathBuf;

pub struct SqliteBackend {
    pub(crate) conn: Connection,
    db_path: PathBuf,  // ADD: Store path for magellan shell-out
}

impl SqliteBackend {
    pub fn open(db_path: &Path) -> Result<Self, LlmError> {
        let conn = Connection::open(db_path)?;
        Ok(Self {
            conn,
            db_path: db_path.to_path_buf(),  // ADD: Store path
        })
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| **query.rs with direct Connection::open** | **SqliteBackend with trait methods** | Phase 18 (this phase) | Enables dual backend support. Code is better organized. |
| **Single-function queries** | **Trait-based abstraction** | Phase 17-18 | Backend can be swapped at runtime without changing CLI code. |

**Deprecated/outdated:**
- **Direct Connection::open in public functions:** After refactor, all queries go through Backend trait. query.rs functions become thin wrappers.

## Open Questions

1. **Should query.rs public functions be removed or kept as wrappers?**
   - What we know: Tests currently use `llmgrep::query::search_symbols` directly.
   - What's unclear: Whether to break tests immediately or provide gradual migration.
   - Recommendation: Keep query.rs functions as wrappers during Phase 18. Have them delegate to SqliteBackend. Deprecate in future phase after explicit test migration. This preserves test compatibility during refactor.

2. **How to handle db_path for ast/find_ast methods?**
   - What we know: Current code shells out to `magellan ast` with `--db` flag. SqliteBackend only stores `conn`, not the path.
   - What's unclear: Whether to reconstruct path from Connection or store it separately.
   - Recommendation: Add `db_path: PathBuf` field to SqliteBackend struct. Store during `open()`, use for shell-out commands. This is cleaner than trying to extract path from Connection.

3. **Should algorithm filtering stay in algorithm.rs or move to sqlite.rs?**
   - What we know: `apply_algorithm_filters()` currently shells out to magellan and returns SymbolIds. It doesn't need Connection.
   - What's unclear: Whether to keep it separate or move into backend.
   - Recommendation: Keep in algorithm.rs. It's a higher-level operation that composes over backend results. Moving it would couple backend to algorithm logic.

## Sources

### Primary (HIGH confidence)
- `/home/feanor/Projects/llmgrep/src/query.rs` (full file, lines 1-2099) — Complete implementations of search_symbols, search_references, search_calls, plus all helper functions
- `/home/feanor/Projects/llmgrep/src/backend/mod.rs` (lines 1-186) — BackendTrait definition, Backend enum, detect_and_open()
- `/home/feanor/Projects/llmgrep/src/backend/sqlite.rs` (lines 1-84) — SqliteBackend stub with TODO comments
- `/home/feanor/Projects/llmgrep/src/main.rs` (lines 1-2052) — Current usage of query module functions
- `/home/feanor/Projects/llmgrep/src/lib.rs` (lines 1-51) — Public API re-exports
- `/home/feanor/Projects/llmgrep/src/error.rs` (lines 1-200) — LlmError enum with SearchFailed variant
- `/home/feanor/Projects/llmgrep/src/output.rs` (lines 1-200) — Response types: SearchResponse, ReferenceSearchResponse, CallSearchResponse
- `/home/feanor/Projects/llmgrep/src/algorithm.rs` (lines 1-150) — AlgorithmOptions, apply_algorithm_filters
- `/home/feanor/Projects/llmgrep/src/ast.rs` (lines 1-150) — check_ast_table_exists, AstContext
- `/home/feanor/Projects/llmgrep/src/safe_extraction.rs` (full file) — extract_symbol_content_safe re-export
- `/home/feanor/Projects/llmgrep/.planning/phases/017-backend-infrastructure/17-RESEARCH.md` (full file) — Phase 17 research with architecture decisions

### Secondary (MEDIUM confidence)
- [rusqlite documentation](https://docs.rs/rusqlite/) — Connection API, prepare_cached, params_from_iter
- [regex crate documentation](https://docs.rs/regex/) — RegexBuilder, size_limit

### Tertiary (LOW confidence)
- None — all findings verified with primary sources.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - All dependencies are existing and verified in code
- Architecture: HIGH - Code movement pattern is standard Rust refactoring
- Pitfalls: HIGH - All pitfalls based on common Rust refactoring issues
- Code examples: HIGH - All examples from actual source code

**Research date:** 2026-02-09
**Valid until:** 2026-03-11 (30 days - stable codebase)

**Key verification points:**
- ✅ All three search functions are in query.rs (search_symbols: 345-956, search_references: 958-1192, search_calls: 1194-1418)
- ✅ Helper functions identified and categorized (query building, scoring, file I/O, caching)
- ✅ SqliteBackend stub exists with conn: Connection field
- ✅ BackendTrait signatures match query.rs return types
- ✅ Zero breaking changes requirement understood
