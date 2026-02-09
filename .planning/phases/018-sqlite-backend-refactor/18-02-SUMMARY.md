---
phase: 018-sqlite-backend-refactor
plan: 02
subsystem: database-abstraction
tags: [sqlite, backend, refactor, connection-separation]

# Dependency graph
requires:
  - phase: 018-sqlite-backend-refactor
    plan: 18-01
    provides: SqliteBackend with db_path field
provides:
  - search_symbols_impl() internal function with Connection parameter
  - Refactored search_symbols() as wrapper for backward compatibility
  - Foundation for SqliteBackend::search_symbols trait implementation
affects: [18-03-references-calls, 18-04-ast-commands]

# Tech tracking
tech-stack:
  added: []
  patterns: [connection-parameter-extraction, wrapper-function-for-backward-compat]

key-files:
  created: []
  modified: [src/query.rs]

key-decisions:
  - "search_symbols_impl takes explicit Connection parameter to enable trait method implementation"
  - "db_path passed as separate parameter to _impl for apply_algorithm_filters call"
  - "All SQL query logic preserved verbatim - no behavior changes"

patterns-established:
  - "Connection opening in wrapper, SQL logic in _impl function"
  - "pub(crate) visibility for internal implementation functions"

# Metrics
duration: 4min
completed: 2026-02-09
---

# Phase 18 Plan 2: search_symbols_impl Creation Summary

**Extracted search_symbols SQL query logic into search_symbols_impl() with explicit Connection parameter for trait method implementation**

## Performance

- **Duration:** 4 min
- **Started:** 2026-02-09T21:16:22Z
- **Completed:** 2026-02-09T21:20:00Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments

- Created `search_symbols_impl(conn: &Connection, db_path: &Path, options: &SearchOptions)` internal function
- Refactored `search_symbols()` to wrapper that opens connection and calls `_impl`
- Preserved all SQL query logic verbatim - no behavior changes
- All 151 unit tests pass (63 search-specific tests)

## Task Commits

Each task was committed atomically:

1. **Task 1: Create search_symbols_impl with Connection parameter** - `39000b3` (feat)

## Files Created/Modified

- `src/query.rs` - Added search_symbols_impl(), refactored search_symbols() as wrapper

## Decisions Made

None - followed plan as specified.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

**sccache wrapper issue:**
- Initial `cargo build` failed with sccache not found error
- Resolved by setting `RUSTC_WRAPPER=""` environment variable
- No code changes required, build environment issue only

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- search_symbols_impl() ready for SqliteBackend::search_symbols trait implementation
- Pattern established for search_references_impl and search_calls_impl in 18-03
- AST filtering functions already use Connection parameter - compatible pattern

## Verification

**Must-haves verified:**
- [x] search_symbols_impl() function exists taking &Connection parameter
- [x] search_symbols_impl() contains verbatim SQL query logic from search_symbols()
- [x] query.rs::search_symbols() calls search_symbols_impl() as wrapper
- [x] Code compiles after _impl() function creation

**Artifacts verified:**
- [x] src/query.rs provides search_symbols_impl() internal function
- [x] search_symbols_impl exported as pub(crate)

**Key links verified:**
- [x] src/query.rs::search_symbols calls search_symbols_impl(&conn, options.db_path, &options)
- [x] search_symbols_impl takes &Connection parameter
- [x] search_symbols_impl takes &Path for db_path (needed for apply_algorithm_filters)

**Tests:** 151/151 passing

---
*Phase: 018-sqlite-backend-refactor*
*Plan: 02*
*Completed: 2026-02-09*
