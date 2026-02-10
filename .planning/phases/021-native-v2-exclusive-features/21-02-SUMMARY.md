---
phase: 021-native-v2-exclusive-features
plan: 02
subsystem: native-v2-backend
tags: [kv-store, fqn-lookup, magellan, llm-e112, sqlitegraph]

# Dependency graph
requires:
  - phase: 020-cli-integration
    provides: Backend enum, require_native_v2 helper, RequiresNativeV2Backend error
  - phase: 019-native-v2-backend-implementation
    provides: NativeV2Backend with CodeGraph API, KV store access
provides:
  - SymbolNotFound error (LLM-E112) for exact FQN lookup failures
  - lookup command for O(1) symbol resolution by fully-qualified name
  - Backend::lookup() method delegating to native-v2 and SQLite backends
  - NativeV2Backend::lookup() using magellan::kv::lookup_symbol_by_fqn
affects: [phase-21-03, phase-21-04, phase-21-05, phase-21-06]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - O(1) KV lookup for exact FQN resolution
    - Error field extraction (partial name from FQN) for helpful hints
    - SymbolNotFound error with remediation to complete command

key-files:
  created: []
  modified:
    - src/error.rs - Added SymbolNotFound variant with LLM-E112 code
    - src/main.rs - Added Command::Lookup and run_lookup() function
    - src/backend/mod.rs - Added Backend::lookup() method and BackendTrait::lookup()
    - src/backend/native_v2.rs - Implemented lookup using lookup_symbol_by_fqn
    - src/backend/sqlite.rs - Implemented lookup returning RequiresNativeV2Backend error

key-decisions:
  - "SymbolNotFound includes fqn, db, and partial fields for context and suggestions"
  - "lookup uses O(1) KV lookup via lookup_symbol_by_fqn, then SQL for full details"
  - "SQLite backend returns RequiresNativeV2Backend (not SymbolNotFound) for clarity"

patterns-established:
  - "Native-V2 Exclusive Command Pattern: define command unconditionally, use require_native_v2() at runtime"
  - "Error Remediation Pattern: suggest alternative commands (complete --partial) when lookup fails"
  - "Two-Phase Lookup Pattern: KV store for O(1) ID resolution, SQL for full metadata"

# Metrics
duration: 7min
completed: 2026-02-10
---

# Phase 21: Native-V2 Exclusive Features - Plan 02 Summary

**O(1) exact FQN lookup using magellan KV store with SymbolNotFound error handling**

## Performance

- **Duration:** 7 min
- **Started:** 2026-02-10T01:13:34Z
- **Completed:** 2026-02-10T01:20:32Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments

- Added `SymbolNotFound` error variant (LLM-E112) with helpful remediation hints
- Implemented `lookup` command for exact FQN symbol resolution
- NativeV2Backend uses O(1) KV lookup via `magellan::kv::lookup_symbol_by_fqn`
- SQLite backend returns clear `RequiresNativeV2Backend` error
- Full symbol details returned via SQL query after KV lookup

## Task Commits

Each task was committed atomically:

1. **Task 1: Add SymbolNotFound error variant** - `f94c08e` (feat)
2. **Task 2: Add lookup command with O(1) exact FQN lookup** - `42e5b8c` (feat)

## Files Created/Modified

- `src/error.rs` - Added `LlmError::SymbolNotFound` variant with fqn, db, partial fields; updated error_code(), severity(), remediation(), and module doc
- `src/main.rs` - Added `Command::Lookup` variant with --fqn argument; implemented `run_lookup()` function with backend delegation
- `src/backend/mod.rs` - Added `Backend::lookup()` method delegating to inner variants; added `BackendTrait::lookup()` signature
- `src/backend/native_v2.rs` - Implemented `lookup()` using `magellan::kv::lookup_symbol_by_fqn()` for O(1) ID resolution, then SQL query for full symbol details
- `src/backend/sqlite.rs` - Implemented `lookup()` returning `RequiresNativeV2Backend` error (SQLite cannot efficiently do exact FQN lookups)

## Implementation Details

### O(1) Lookup Implementation

The `lookup` command provides instant symbol resolution by fully-qualified name:

1. **KV Store Lookup:** Uses `magellan::kv::lookup_symbol_by_fqn()` for O(1) SymbolId resolution
2. **Metadata Query:** Queries `symbol_nodes` table by SymbolId for complete details
3. **Error Handling:** Returns `SymbolNotFound` (LLM-E112) when FQN doesn't exist, with hint to use `complete` command
4. **SQLite Fallback:** Returns `RequiresNativeV2Backend` (LLM-E111) when used with SQLite database

### Error Message Design

The `SymbolNotFound` error includes three fields:
- `fqn`: The exact FQN that was not found
- `db`: Database path for context
- `partial`: Extracted last segment (e.g., "function" from "crate::module::function") for suggestions

Error message suggests:
1. Use `complete` command with `--partial` flag
2. Check FQN spelling and module path
3. Verify database is up to date with `magellan status`

## Decisions Made

1. **Error field design:** Three fields (fqn, db, partial) provide context and actionable remediation hints
2. **Two-phase lookup:** KV store for O(1) ID resolution, then SQL for metadata - this leverages Magellan's architecture where KV stores map FQN→SymbolId
3. **SQLite returns different error:** SQLite returns `RequiresNativeV2Backend` instead of `SymbolNotFound` to clarify the limitation is backend capability, not missing data
4. **Unconditional command definition:** `Command::Lookup` is defined unconditionally (not cfg-gated) for consistent --help output, with runtime check via `require_native_v2()`

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

**Issue:** Build failed with mismatched type error - `run_complete` expected `prefix: String` but received `prefix: &String`
- **Resolution:** Added `.clone()` to pass owned String instead of reference
- **Note:** This was a pre-existing issue from 21-01 that surfaced when building 21-02

**Issue:** Extra closing brace in native_v2.rs after adding `lookup()` method
- **Resolution:** Removed duplicate closing brace on line 519

**Issue:** `parent` and `language` fields are `Option<String>` and can't be directly formatted
- **Resolution:** Added proper Option handling with if-let guards before printing

## Next Phase Readiness

- Lookup command complete and tested
- SymbolNotFound error infrastructure ready
- Ready for next native-v2 exclusive features (label-based search, performance metrics)

---
*Phase: 021-native-v2-exclusive-features*
*Plan: 02*
*Completed: 2026-02-10*
