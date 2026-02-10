# Codebase Concerns

**Analysis Date:** 2026-02-10
**Last Updated:** 2026-02-10 (Phase 24 polish updates)
**Verified Against:** v3.0.1 (native-v2 feature enabled)

---

## Recently Resolved (Phase 24 - Code Quality Polish)

The following concerns were addressed during Phase 24:

| Issue | Phase 24 Plan | Resolution |
|-------|--------------|------------|
| Compiler warnings (unused/dead code) | 24-01 | Removed unused `db_path()` method, prefixed unused variables |
| Test code using `.unwrap()` | 24-02 | Replaced 266 `.unwrap()` with `.expect("descriptive message")` |
| Missing safety documentation | 24-03 | Added `# Safety` docs for all 8 unsafe blocks in native_v2.rs |
| Undocumented public APIs | 24-04 | Added module-level docs to lib.rs, query.rs; comprehensive docs to output.rs |
| Code consistency issues | 24-05 | Fixed redundant closures, added Default impl, fixed needless borrows |

---

## ~~Confirmed Production Issues~~ - ALL RESOLVED

### ~~1. Hardcoded Language String~~ - FIXED in Phase 22
- **Severity:** High (Misleading data)
- **Files:** `src/backend/native_v2.rs` (lines 121, 228)
- **Issue:** Native-V2 backend hardcodes `language: Some("Rust".to_string())` in all search results
- **Fix applied:** Language inference from file extension using `infer_language()` function
- **Resolution:** Phase 22-01 implemented language detection for 7+ languages

### ~~2. Production Debug Output Pollution~~ - FIXED in Phase 22
- **Severity:** High (UX/Production readiness)
- **Files:** `src/backend/native_v2.rs` (lines 522-559 in `complete()` method)
- **Issue:** Production code contained 10+ `eprintln!("DEBUG: ...")` statements
- **Fix applied:** Removed all debug statements (36 lines)
- **Resolution:** Phase 22-02 cleaned up production debug output

### ~~3. Native-V2 Backend Missing Features~~ - FIXED in Phase 23
- **Severity:** Medium (Feature parity gap)
- **Files:** `src/backend/native_v2.rs`
- **Issues resolved:**
  - `score` - Implemented with relevance scoring (Phase 23-03)
  - `context` - Implemented with context extraction (Phase 23-03)
  - `snippet` - Implemented with snippet extraction (Phase 23-03)
  - `fan_in`, `fan_out`, `cyclomatic_complexity` - Implemented via KV metrics (Phase 23-04)
- **Resolution:** Full feature parity achieved with SQLite backend

## Technical Debt

### UnsafeCell Usage Pattern - PARTIALLY DOCUMENTED
- **Files:** `src/backend/native_v2.rs` (lines 33-73, 148, 264, 366, 481, 503)
- **Issue:** Uses `UnsafeCell` for interior mutability because `CodeGraph` requires `&mut self` but `BackendTrait` takes `&self`
- **Status:** Safety documentation added (Phase 24-03)
- **Documentation:** All 8 unsafe blocks now have comprehensive `# Safety` comments explaining invariants
- **Remaining concern:** No tests for concurrent access scenarios
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

### ~~Test Code Using wrap() Excessively~~ - RESOLVED
- **Status:** Fixed in Phase 24-02
- **Resolution:** Replaced all 266 `.unwrap()` calls with `.expect("descriptive message")` in test code
- **Files modified:** src/query.rs, tests/algorithm_tests.rs, tests/ast_tests.rs, tests/backend_detection_test.rs, tests/backend_parity_extended_test.rs, tests/cli_integration_test.rs, tests/integration_tests.rs, tests/language_detection_test.rs, tests/native_v2_commands_test.rs, tests/shorthand_tests.rs, tests/unit_tests.rs

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

### ~~Unused Variable Warning~~ - RESOLVED
- **Status:** Fixed in Phase 24-01
- **Resolution:** Variable already prefixed with underscore (`_partial`), no changes needed

### ~~Dead Code Warning~~ - RESOLVED
- **Status:** Fixed in Phase 24-01
- **Resolution:** Removed unused `db_path()` method from src/backend/sqlite.rs

---

## Summary by Priority

| Priority | Issue | Files | Status |
|----------|-------|-------|--------|
| ~~**HIGH**~~ | Hardcoded "Rust" language | `native_v2.rs:121, 228` | ~~Confirmed Bug~~ Fixed in Phase 22 |
| ~~**HIGH**~~ | Debug output in production | `native_v2.rs:522-559` | ~~Confirmed Bug~~ Fixed in Phase 22 |
| ~~**MEDIUM**~~ | Missing native-v2 features (score, context, snippet, metrics) | `native_v2.rs` | ~~Feature Gap~~ Fixed in Phase 23 |
| **MEDIUM** | String cloning in hot paths | `native_v2.rs` | Performance |
| ~~**LOW**~~ | Test code using .unwrap() | `query.rs` tests | ~~Code Quality~~ Fixed in Phase 24-02 |
| ~~**LOW**~~ | Unused/dead code warnings | `sqlite.rs` | ~~Code Quality~~ Fixed in Phase 24-01 |
| ~~**LOW**~~ | Missing safety documentation | `native_v2.rs` | ~~Code Quality~~ Fixed in Phase 24-03 |
| ~~**LOW**~~ | Undocumented public APIs | `lib.rs`, `query.rs`, `output.rs` | ~~Code Quality~~ Fixed in Phase 24-04 |

*Concerns audit: 2026-02-10 (updated after Phase 24 polish)*
