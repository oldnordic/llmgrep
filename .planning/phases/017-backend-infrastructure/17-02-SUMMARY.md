---
phase: 017-backend-infrastructure
plan: 02
subsystem: runtime-backend-dispatch
tags: [enum, backend, runtime-dispatch, detection]

dependency_graph:
  requires:
    - "17-01 (Backend trait abstraction)"
  provides:
    - "Backend enum with runtime dispatch"
    - "SqliteBackend struct stub"
    - "NativeV2Backend struct stub (cfg-gated)"
    - "Backend error variants (LLM-E109, LLM-E110)"
  affects:
    - "17-05 through 17-21 (backend implementation plans)"

tech_stack:
  added:
    - "Backend enum for runtime dispatch"
    - "detect_and_open() function using magellan::detect_backend_format"
  patterns:
    - "Enum-based runtime dispatch"
    - "Feature-gated variants (#[cfg(feature = \"native-v2\")])"
    - "Trait method delegation via match arms"

key_files:
  created:
    - path: "src/backend/sqlite.rs"
      description: "SqliteBackend struct with Backend trait stubs"
    - path: "src/backend/native_v2.rs"
      description: "NativeV2Backend struct with Backend trait stubs (cfg-gated)"
  modified:
    - path: "src/backend/mod.rs"
      changes: "Added Backend enum, detect_and_open function, module declarations, 5 delegating methods"
    - path: "src/error.rs"
      changes: "Added NativeV2BackendNotSupported (LLM-E109) and BackendDetectionFailed (LLM-E110)"

decisions: []
metrics:
  duration_seconds: 180
  completed_date: "2026-02-09T19:37:00Z"
  tasks_completed: 4
  files_changed: 4
  commits: 4
---

# Phase 17 Plan 02: Backend Enum and Runtime Dispatch Summary

**Backend enum providing runtime dispatch to SqliteBackend or NativeV2Backend with database format detection via magellan::detect_backend_format**

## Performance

- **Duration:** 3 min (180 seconds)
- **Started:** 2026-02-09T19:33:58Z
- **Completed:** 2026-02-09T19:37:00Z
- **Tasks:** 4
- **Files changed:** 4

## Accomplishments

- Created Backend enum in src/backend/mod.rs with Sqlite and NativeV2 variants
- Added detect_and_open() function using magellan::detect_backend_format for runtime backend detection
- Implemented 5 delegating methods (search_symbols, search_references, search_calls, ast, find_ast)
- Created SqliteBackend stub in src/backend/sqlite.rs
- Created NativeV2Backend stub in src/backend/native_v2.rs (cfg-gated)
- Added error variants NativeV2BackendNotSupported (LLM-E109) and BackendDetectionFailed (LLM-E110)
- Default build compiles successfully

## Task Commits

Each task was committed atomically:

| Task | Name | Commit | Files |
| ---- | ---- | ---- | ----- |
| 1 | Extend src/backend/mod.rs with Backend enum and detect_and_open | `52f51b8` | src/backend/mod.rs |
| 2 | Create src/backend/sqlite.rs with SqliteBackend stub | `bf0caf4` | src/backend/sqlite.rs |
| 3 | Create src/backend/native_v2.rs with NativeV2Backend stub | `92f8601` | src/backend/native_v2.rs |
| 4 | Add backend error variants to src/error.rs | `5678ab9` | src/error.rs |

## Files Created/Modified

### Created
- `src/backend/sqlite.rs` - SqliteBackend struct with conn: Connection field, Backend trait implementation stubs
- `src/backend/native_v2.rs` - NativeV2Backend struct with graph: CodeGraph field, cfg-gated, Backend trait stubs

### Modified
- `src/backend/mod.rs` - Backend enum, detect_and_open function, module declarations, 5 delegating methods
- `src/error.rs` - NativeV2BackendNotSupported (LLM-E109), BackendDetectionFailed (LLM-E110)

## Deviations from Plan

### Deviation 1: CodeGraph is not Send + Sync

**Type:** Pre-existing dependency issue (discovered during verification)

**Found during:** Task 3 verification

**Issue:** `magellan::CodeGraph` contains `Rc` internally which is not `Send + Sync`. Our `Backend` trait requires `Send + Sync` bounds. This causes compilation errors when building with `--features native-v2`.

**Error:**
```
error: `Rc<(dyn sqlitegraph::backend::GraphBackend + 'static)>` cannot be sent between threads safely
```

**Impact:** Native-v2 backend cannot be fully implemented until magellan's CodeGraph is made thread-safe (Arc instead of Rc) or Backend trait Send + Sync bounds are relaxed.

**Workaround:** SQLite backend (primary backend) compiles and works correctly. Native-v2 feature flag exists but backend implementation is blocked by this magellan limitation.

**Resolution:** Document as known issue. Native-v2 backend implementation (Phase 19) will require magellan to be updated first, or an alternative approach (e.g., wrapper type, relaxed trait bounds).

## Issues Encountered

**sccache not found (non-blocking):**
- Build used sccache wrapper but binary was missing
- Workaround: Used `RUSTC_WRAPPER=""` to bypass
- Not a code issue, environment configuration

**CodeGraph Send + Sync limitation (see Deviation 1):**
- magellan::CodeGraph uses Rc internally
- Rc is not Send/Sync
- Backend trait requires Send + Sync
- Blocks native-v2 compilation

## Verification Results

### Success Criteria Met

- [x] Backend enum provides runtime dispatch to SqliteBackend or NativeV2Backend
- [x] NativeV2Backend variant is cfg-gated with #[cfg(feature = "native-v2")]
- [x] detect_and_open function uses magellan::detect_backend_format
- [x] Clear error message when native-v2 database detected but feature not enabled (LLM-E109)
- [x] SQLite backend stub compiles successfully

### Partial Success (Blocked by External Dependency)

- [ ] Native-v2 backend compiles with --features native-v2 (BLOCKED by CodeGraph Send/Sync)

**Build Results:**
```bash
# Default build (SQLite only) - SUCCESS
RUSTC_WRAPPER="" cargo build --lib
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.06s

# Native-v2 build - BLOCKED by CodeGraph limitation
RUSTC_WRAPPER="" cargo build --lib --features native-v2
error: `Rc<...>` cannot be sent between threads safely
```

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Backend enum foundation complete
- SQLite backend stub ready for implementation (Phase 18)
- Native-v2 backend stub created but blocked by magellan CodeGraph Send/Sync limitation
- Error handling in place for backend detection and unsupported format
- Native-v2 feature flag exists (from 17-03)

**Blocker for Phase 19:** magellan::CodeGraph must be made Send + Sync before NativeV2Backend can be fully implemented. Options:
1. Update magellan to use Arc instead of Rc
2. Relax Backend trait Send + Sync bounds (not recommended for thread safety)
3. Create thread-safe wrapper for CodeGraph

---
*Phase: 017-backend-infrastructure*
*Plan: 02*
*Completed: 2026-02-09*
