---
phase: 20-cli-integration
plan: 01
subsystem: cli
tags: [backend, detection, delegation, sqlite, native-v2]

# Dependency graph
requires:
  - phase: 17-backend-infrastructure
    provides: Backend enum, BackendTrait, detect_and_open()
  - phase: 18-sqlite-backend-refactor
    provides: SqliteBackend implementation
  - phase: 19-native-v2-backend-implementation
    provides: NativeV2Backend implementation
provides:
  - Automatic backend detection in CLI entry point
  - Transparent backend routing for search/ast/find-ast commands
affects: []

# Tech tracking
tech-stack:
  added: []
  patterns:
    - Backend delegation pattern via Backend enum
    - Automatic backend detection via detect_and_open()

key-files:
  created: []
  modified:
    - src/main.rs

key-decisions:
  - "No --backend flag added: detection is automatic per Phase 17 decision"
  - "Backend detection happens at start of each command before using database"
  - "Error messages for native-v2 without feature include remediation hints"

patterns-established:
  - "Backend::detect_and_open() called in run_search(), run_ast(), run_find_ast()"
  - "Search/AST/FindAst commands delegate to Backend enum methods"
  - "Error handling via LlmError::NativeV2BackendNotSupported with remediation"

# Metrics
duration: 4min
completed: 2026-02-10
---

# Phase 20 Plan 01: CLI Backend Integration Summary

**Backend enum integrated into CLI with automatic SQLite/Native-V2 detection and transparent delegation**

## Performance

- **Duration:** 4 minutes
- **Started:** 2026-02-09T23:53:29Z
- **Completed:** 2026-02-09T23:57:05Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments

- **Backend detection integrated**: All three CLI commands (search, ast, find-ast) now call `Backend::detect_and_open()` at startup
- **Transparent delegation**: Search operations delegate to `backend.search_symbols()`, `backend.search_references()`, `backend.search_calls()`
- **AST commands use backend**: `run_ast()` and `run_find_ast()` replaced magellan shell-out with `backend.ast()` and `backend.find_ast()`
- **Zero user-visible changes**: No --backend flag needed, detection is automatic

## Task Commits

1. **Task 1: Replace direct query module calls with Backend enum delegation** - `2888dcd` (feat)

## Files Created/Modified

- `src/main.rs` - CLI entry point updated with Backend detection and delegation
  - Added `use llmgrep::backend::Backend;`
  - Removed direct `search_symbols`, `search_references`, `search_calls` imports
  - Added `Backend::detect_and_open()` call in `run_search()` after db_path validation
  - Replaced all `search_*` function calls with `backend.search_*()` delegation
  - Updated `run_ast()` to use `backend.ast()` instead of magellan shell-out
  - Updated `run_find_ast()` to use `backend.find_ast()` instead of magellan shell-out
  - Removed `use std::process::Command;` import (no longer needed)

## Decisions Made

None - followed plan as specified. The decision to not add a --backend flag was made in Phase 17 and upheld here.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None - build and all 151 tests passed without issues.

## User Setup Required

None - no external service configuration required. The CLI now automatically detects whether the database is SQLite or Native-V2 format and dispatches accordingly.

## Next Phase Readiness

- CLI integration complete, ready for Phase 21 (Native-v2 storage implementation)
- All commands use backend abstraction, transparent to users
- Native-V2 databases without native-v2 feature enabled will show helpful error message with remediation steps

---
*Phase: 20-cli-integration*
*Completed: 2026-02-10*
