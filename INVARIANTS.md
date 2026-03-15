# llmgrep Invariants

This document describes the behavioral guarantees and invariants that llmgrep maintains.

## Path Normalization

**Invariant:** All file paths are normalized before queries to Magellan.

```rust
use magellan::graph::path_utils::normalize_path;

// Paths are normalized before any query
let normalized = normalize_path(user_path, None)?;
backend.symbols_in_file(&normalized)?;
```

**Behaviors:**
- `./src/lib.rs` → `src/lib.rs` (relative prefix stripped)
- `src\\lib.rs` → `src/lib.rs` (backslash converted)
- `src//lib.rs` → `src/lib.rs` (duplicate separators removed)
- Existing paths → canonicalized to absolute path

## Path Comparison

**Invariant:** Path comparison uses normalized forms.

```rust
pub fn paths_equivalent(path1: &str, path2: &str) -> bool {
    let norm1 = normalize_path_for_query(path1);
    let norm2 = normalize_path_for_query(path2);
    norm1 == norm2
}
```

**Behaviors:**
- `paths_equivalent("./src/lib.rs", "src/lib.rs")` → `true`
- `paths_equivalent("src\\lib.rs", "src/lib.rs")` → `true`

## Ambiguity Handling

**Invariant:** Multiple symbol matches are surfaced explicitly, never silently resolved.

```rust
pub enum SymbolLookupResult {
    Unique(SymbolInfo),
    Ambiguous { path: String, name: String, candidates: Vec<SymbolInfo> },
    NotFound,
}
```

**Behaviors:**
- If 1 symbol matches → `Unique`
- If N symbols match (N > 1) → `Ambiguous` with all candidates
- If 0 symbols match → `NotFound`

**No silent first-match resolution.**

## Backend Detection

**Invariant:** Backend format is detected from file, not specified by user.

```rust
// Automatic by file extension and header bytes
// .geo → GeometricBackend
// "SQLTGF" header → NativeV3Backend (if feature enabled)
// "SQLite format 3" → SqliteBackend
```

## Error Consistency

**Invariant:** All errors include remediation hints.

```rust
impl LlmError {
    pub const fn remediation(&self) -> Option<&'static str> {
        match self {
            LlmError::ChunksNotAvailable { .. } => Some(
                "Reindex with chunking: magellan watch --root . --db code.geo --chunk"
            ),
            // ...
        }
    }
}
```

## Backend Contract Assumptions

llmgrep assumes the following about Magellan's GeometricBackend:

1. **Thread safety:** `GeometricBackend` can be accessed from a single thread
2. **Re-entrancy:** Multiple queries can be made sequentially
3. **Path handling:** `symbols_in_file()` accepts normalized paths
4. **Ambiguity:** `find_symbol_id_by_path_and_name()` returns `Err(AmbiguityError)` for multiple matches
5. **Chunks:** `get_code_chunks_for_symbol()` returns empty vec if chunking not enabled

## Delegated to Magellan

The following are **not** llmgrep's responsibility:

- **Database freshness** — Magellan's watch mode handles updates
- **Schema versioning** — Magellan manages migrations
- **File watching** — Magellan's indexer handles events
- **Persistence** — Magellan's storage layer handles writes

## Query Behavior

**Search by name:**
- Case-sensitive exact match on symbol name
- Returns all matching symbols across all files
- Use `--path-filter` to disambiguate

**Search by FQN:**
- Exact match only (no partial FQN search)
- Returns single symbol or `NotFound`

**Reference search:**
- Returns structured references from graph edges
- Falls back to textual search if no structured refs found

**Chunk retrieval:**
- Returns empty vec if chunking was not enabled during indexing
- Not an error condition — graceful degradation
