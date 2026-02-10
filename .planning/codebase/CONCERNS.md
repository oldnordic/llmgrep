# Codebase Concerns

**Analysis Date:** 2026-02-10
**Verified Against:** v3.0.1 (native-v2 feature enabled)

## Confirmed Production Issues

### 1. Hardcoded Language String - CONFIRMED BUG
- **Severity:** High (Misleading data)
- **Files:** `src/backend/native_v2.rs` (lines 121, 228)
- **Issue:** Native-V2 backend hardcodes `language: Some("Rust".to_string())` in all search results
- **Impact:** Non-Rust codebases show incorrect language. Python, JavaScript, TypeScript, Java, C, C++ files all report as "Rust"
- **Verification:** Tested with `test.py` file - returned `"language": "Rust"`
- **Fix approach:** Infer language from file extension using existing `infer_language()` function from `query.rs`

### 2. Production Debug Output Pollution - CONFIRMED BUG
- **Severity:** High (UX/Production readiness)
- **Files:** `src/backend/native_v2.rs` (lines 522-559 in `complete()` method)
- **Issue:** Production code contains 10+ `eprintln!("DEBUG: ...")` statements
- **Impact:** Every `llmgrep complete` command prints 15+ lines of debug output to stderr
- **Verification:** Running `llmgrep complete --prefix "hel" --limit 1` outputs:
  ```
  DEBUG: Found 5 total KV entries with 'sym:' prefix
  DEBUG:   "sym:fqn:hello" -> Integer(4)
  DEBUG: KV indexing 1 symbols for file /tmp/test_code/test.py
  DEBUG: populate_symbol_index called with 1 symbols, file_id=1
  [... 11 more DEBUG lines]
  ```
- **Fix approach:** Remove all debug statements or conditionally compile behind `#[cfg(debug_assertions)]`

### 3. Native-V2 Backend Missing Features - CONFIRMED
- **Severity:** Medium (Feature parity gap)
- **Files:** `src/backend/native_v2.rs`
- **Issues:**
  - `score` always returns `None` (line 113, 220)
  - `context` always returns `None` (line 103, 210)
  - `snippet` always returns `None` (line 119, 226)
  - `fan_in`, `fan_out`, `cyclomatic_complexity` always return `None` (lines 124-126, 231-233)
- **Impact:** `--with-context`, `--with-snippet`, `--min-complexity`, `--min-fan-in`, relevance sorting don't work with native-v2
- **Verification:** These fields are hardcoded to `None` in `symbol_node_to_match()` and search methods

## Technical Debt

### UnsafeCell Usage Pattern
- **Files:** `src/backend/native_v2.rs` (lines 33-73, 148, 264, 366, 481, 503)
- **Issue:** Uses `UnsafeCell` for interior mutability because `CodeGraph` requires `&mut self` but `BackendTrait` takes `&self`
- **Why fragile:** Manual verification required that no concurrent access occurs
- **Current safety:** Relies on exclusive ownership claim in documentation
- **Test coverage:** No tests for concurrent access scenarios
- **Mitigation:** `Send` is implemented but `Sync` is NOT - prevents multi-threaded access

### String Cloning in Hot Paths
- **Files:** `src/backend/native_v2.rs`
- **Issue:** Excessive `.to_string()` calls in search loops
- **Examples:**
  - Line 201: `let file_path_str = symbol.file_path.to_string_lossy().to_string();`
  - Line 316: `let file_path = reference.file_path.to_string_lossy().to_string();`
  - Line 426: `let file_path = call.file_path.to_string_lossy().to_string();`
- **Impact:** ~6 heap allocations per search result
- **Improvement path:** Use `Cow<str>` or reuse string buffers

### Test Code Using unwrap() Excessively
- **Files:** `src/query.rs` (lines 2926-3638)
- **Issue:** Test code uses `.unwrap()` instead of proper error assertions
- **Count:** ~40+ occurrences of `.unwrap()` in test module
- **Impact:** Tests panic with unclear messages instead of showing assertion failures
- **Examples:**
  ```rust
  let db_file = tempfile::NamedTempFile::new().unwrap();
  let conn = Connection::open(db_file.path()).unwrap();
  let (response, partial, _) = search_symbols(options).unwrap();
  ```
- **Fix approach:** Use `.expect("descriptive message")` or proper result assertions

## Shell-Out External Process Dependency

### Magellan CLI Dependency
- **Files:** `src/algorithm.rs`, `src/backend/sqlite.rs`
- **Issue:** SQLite backend shells out to `magellan` CLI for graph algorithms and AST queries
- **Impact:**
  - Requires magellan binary in PATH
  - Slower than native API (process spawn overhead)
  - Fragile to PATH changes
- **Commands used:** `magellan find`, `reachable`, `dead-code`, `cycles`, `condense`, `paths`
- **Note:** This is accepted as permanent hybrid approach for SQLite backend

## Known Limitations (By Design)

### Native-V2 Requires Feature Flag
- **Files:** `src/backend/native_v2.rs` (line 6: `#[cfg(feature = "native-v2")]`)
- **Issue:** Binary built without `--features native-v2` cannot read Magellan 2.x databases
- **Error:** `LLM-E109: Native-V2 backend detected but llmgrep was built without native-v2 support`
- **Workaround:** Rebuild with `cargo install llmgrep --features native-v2`
- **Note:** This is intentional - keeps default binary smaller

### Linear Scanning in Native-V2 Search
- **Files:** `src/backend/native_v2.rs` (lines 150-238)
- **Issue:** `search_symbols()` iterates through all files and all symbols within files
- **Cause:** No indexed symbol name lookup in CodeGraph API for substring matching
- **Impact:** O(n) where n = total symbols in database
- **Mitigation:** Path filtering reduces search space
- **Note:** This is a current limitation of the CodeGraph API, not a bug

## Missing Error Context

### Generic unwrap() in Main
- **Files:** `src/main.rs`
- **Issue:** Some `.expect()` calls have generic messages
- **Example:** Line 686: `let db_path = validated_db.as_ref().expect("validated db path missing");`
- **Impact:** Less helpful error messages
- **Priority:** Low (errors are rare in this path)

## Code Quality Issues

### Unused Variable Warning
- **Files:** `src/backend/sqlite.rs` (line 159)
- **Issue:** `let partial = fqn.rsplit("::").next().unwrap_or(fqn);` - variable defined but never used
- **Compiler warning:** `warning: unused variable: partial`
- **Fix:** Remove or prefix with underscore

### Dead Code Warning
- **Files:** `src/backend/sqlite.rs` (line 45)
- **Issue:** `pub(crate) fn db_path(&self)` method defined but never called
- **Compiler warning:** `warning: method 'db_path' is never used`
- **Fix:** Remove or make public if needed externally

---

## Summary by Priority

| Priority | Issue | Files | Status |
|----------|-------|-------|--------|
| **HIGH** | Hardcoded "Rust" language | `native_v2.rs:121, 228` | Confirmed Bug |
| **HIGH** | Debug output in production | `native_v2.rs:522-559` | Confirmed Bug |
| **MEDIUM** | Missing native-v2 features (score, context, snippet, metrics) | `native_v2.rs` | Feature Gap |
| **MEDIUM** | String cloning in hot paths | `native_v2.rs` | Performance |
| **LOW** | Test code using unwrap() | `query.rs` tests | Code Quality |
| **LOW** | Unused/dead code warnings | `sqlite.rs` | Code Quality |

*Concerns audit: 2026-02-10*
