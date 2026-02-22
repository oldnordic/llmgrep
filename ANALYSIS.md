# llmgrep Code Analysis Report

Generated: 2026-02-21
Tool: Magellan v2.4.7

## Executive Summary

llmgrep v3.0.9 is a smart grep tool built on top of Magellan. This analysis identified several architectural issues, complexity hotspots, and backend parity gaps that should be addressed.

---

## 📊 Codebase Statistics

| Metric | Count |
|--------|-------|
| Total Files | 34 |
| Total Symbols | 689 |
| Total References | 2,088 |
| Total Calls | 715 |
| Lines of Code | ~13,898 |

### File Size Breakdown

| File | Lines | Risk Level |
|------|-------|------------|
| src/query.rs | 6,131 | 🔴 Very High |
| src/main.rs | 2,678 | 🔴 Very High |
| src/algorithm.rs | 1,107 | 🟡 Medium |
| src/backend/native_v3.rs | 1,124 | 🟡 Medium |
| src/ast.rs | 988 | 🟢 Low |

---

## 🔴 Critical Issues

### 1. Function Parameter Explosion

**Location**: `src/main.rs:662` - `run_search()`

**Problem**: Function has **42 parameters** - this is a code smell indicating the function does too much and should be refactored.

```rust
fn run_search(
    cli: &Cli,
    query: &str,
    mode: SearchMode,
    path: &Option<PathBuf>,
    // ... 38 more parameters
    paths_to: Option<&String>,
) -> Result<(), LlmError>
```

**Impact**: 
- Hard to test
- Hard to call (the call site at line 604-646 is a mess)
- Violates single responsibility principle

**Recommendation**: 
- Create a `SearchContext` struct to group related parameters
- Split into smaller functions by search mode
- Use builder pattern for complex search configurations

---

### 2. Massive File Sizes

**Problem**: Two files are critically oversized:

- `src/query.rs` (6,131 lines) - Contains SQL building, search logic, caching
- `src/main.rs` (2,678 lines) - Contains CLI parsing, dispatch, all command handlers

**Recommendation**:
- Split `query.rs` into:
  - `query/mod.rs` - Public API
  - `query/symbols.rs` - Symbol search
  - `query/references.rs` - Reference search  
  - `query/calls.rs` - Call search
  - `query/builder.rs` - SQL query builders
  - `query/cache.rs` - File caching

- Split `main.rs` into:
  - `cmd/mod.rs` - Command traits
  - `cmd/search.rs` - Search command
  - `cmd/ast.rs` - AST commands
  - `cmd/complete.rs` - Completion command
  - etc.

---

### 3. Backend Feature Parity Gaps

**SQLite Backend** (`src/backend/sqlite.rs`):
- `complete()` - Returns error (requires native-v3)
- `lookup()` - Returns error (requires native-v3)
- `search_by_label()` - Returns error (requires native-v3)
- `ast()` - Shells out to `magellan` CLI (inefficient)
- `find_ast()` - Shells out to `magellan` CLI (inefficient)

**Native V3 Backend** (`src/backend/native_v3.rs`):
- `complete()` - TODO at line 319 (unimplemented)
- `lookup()` - TODO (unimplemented)
- `search_by_label()` - TODO (unimplemented)

**Impact**: Users get different capabilities depending on backend format.

**Recommendation**: 
- Implement missing SQLite functions using SQL queries
- Implement missing native-v3 functions using KV store
- Document backend differences clearly

---

## 🟡 Medium Issues

### 4. Unsafe Code Without Adequate Safety Comments

**Location**: `src/backend/native_v3.rs:178, 220, 307`

The unsafe code has some comments but could be more rigorous:

```rust
unsafe impl Send for NativeV3Backend {}

unsafe fn graph(&self) -> &mut CodeGraph {
    &mut *self.graph.get()
}
```

**Recommendation**: Add SAFETY comments explaining why each unsafe block is sound (see magellan's approach).

---

### 5. High unwrap() Count

**Count**: 257 unwrap() calls across codebase

**Risk**: Potential panic points in production code

**Locations with high density**:
- `src/query.rs` - Database operations
- `src/main.rs` - Test code (acceptable)

**Recommendation**: 
- Replace with proper error handling using `?` operator
- Use `expect()` with descriptive messages for truly invariant cases

---

### 6. Test Failures

**Status**: 2 tests failing in `cli_integration_test.rs`

```
test_ast_with_sqlite_backend - fails: "missing graph_meta table"
test_find_ast_with_sqlite_backend - fails: "missing graph_meta table"
```

**Root Cause**: Tests create databases without proper Magellan schema, then try to run `magellan ast` commands on them.

**Recommendation**: 
- Initialize test databases with proper schema before testing AST commands
- Or use mock magellan responses

---

### 7. Dependency Version Lag

**Current**: `magellan = "2.4.5"`
**Available**: `2.4.7`

**Changes in 2.4.7**:
- Fixed reference/call tracking
- Fixed metrics SQL column names
- Implemented cyclomatic complexity
- CLI refactoring

**Recommendation**: Update dependency to get bug fixes.

---

## 🟢 Low Priority Issues

### 8. Unused Feature Flag

`native-v3` feature is disabled in `Cargo.toml`:
```toml
# native-v3 disabled - use only sqlite-backend to avoid format conflicts
# native-v3 = ["magellan/native-v3", "sqlitegraph/native-v3"]
```

This means native-v3 backend code is never compiled.

### 9. Commented-Out Imports

Multiple files have commented imports that should be cleaned up:
- `src/backend/native_v3.rs:17, 24-25`

### 10. Code Duplication

Functions `load_file`, `span_context_from_file` exist in both:
- `src/backend/native_v3.rs`
- `src/query.rs`

Consider extracting to a common utility module.

---

## 📈 Architecture Recommendations

### Short Term (v3.1.0)

1. **Update magellan to 2.4.7**
2. **Fix failing tests** - Proper test database initialization
3. **Reduce run_search parameters** - Create SearchContext struct
4. **Document backend differences** - Clear feature parity matrix

### Medium Term (v3.2.0)

1. **Implement missing backend functions**:
   - SQLite: `complete()`, `lookup()`, `search_by_label()`
   - Native-V3: All unimplemented methods

2. **Refactor oversized files**:
   - Split query.rs into logical modules
   - Split main.rs command handlers

### Long Term (v4.0.0)

1. **Unified backend API** - Both backends should have 100% parity
2. **Async support** - Consider async/await for I/O operations
3. **Streaming results** - For large result sets

---

## ✅ Positive Findings

1. **Good error handling** - `LlmError` enum covers many cases
2. **Backend abstraction** - Clean `BackendTrait` interface
3. **Backend auto-detection** - Works well for transparent usage
4. **Comprehensive CLI** - Many useful flags and options
5. **Test coverage** - Good test file structure

---

## Cycle Analysis

No cycles detected in the call graph. The codebase has a clean DAG structure.

---

## Collision Analysis

Multiple symbols share names (expected in multi-language codebase):
- `TestClass` - exists in 5 test fixture files
- `main` - exists in 5 files (different languages)
- `insert_define_edge` - 4 test helper functions

These are test fixtures and are not problematic.

---

*Analysis generated by Magellan v2.4.7 - Deterministic Codebase Mapping*
