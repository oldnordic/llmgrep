---
phase: 20-cli-integration
plan: 02
subsystem: error-handling
tags: [error-handling, native-v2, remediation, cli-integration]

# Dependency graph
requires:
  - phase: 17-backend-infrastructure
    provides: NativeV2BackendNotSupported error type, Backend::detect_and_open method
  - phase: 19-native-v2-backend-implementation
    provides: NativeV2Backend implementation
provides:
  - Verified error handling for native-v2 database without native-v2 feature
  - Integration test covering error message format and remediation steps
  - Manual test verification of both human and JSON error output
affects: []

# Tech tracking
tech-stack:
  added: []
  patterns:
    - Error remediation hints displayed in human output with "Hint: " prefix
    - JSON error responses include remediation field
    - Backend detection returns feature-specific errors when native-v2 not available

key-files:
  created: []
  modified:
    - tests/backend_detection_test.rs - Integration test for native-v2 error handling
    - src/main.rs - CLI already integrated with Backend abstraction from Phase 20-01

key-decisions:
  - "Error handling already complete from Phase 17 - no changes needed to error.rs or backend/mod.rs"
  - "CLI integration already complete from Phase 20-01 - uses Backend::detect_and_open"

patterns-established:
  - "Error code LLM-E109 for native-v2 not supported scenarios"
  - "Remediation includes both cargo install and cargo build commands"
  - "Database path displayed in error message for clarity"

# Metrics
duration: 4min
completed: 2026-02-10
---

# Phase 20 Plan 02: Native-V2 Error Handling Summary

**Verified LLM-E109 error with remediation hints when native-v2 database detected but feature not enabled**

## Performance

- **Duration:** 4 min
- **Started:** 2026-02-09T23:53:18Z
- **Completed:** 2026-02-09T23:57:51Z
- **Tasks:** 1 completed
- **Files modified:** 0 (verification only)

## Accomplishments

- **Verified NativeV2BackendNotSupported error** has proper error code (LLM-E109), database path, remediation with cargo install/build commands, and documentation link
- **Verified backend detection** returns NativeV2BackendNotSupported error when native-v2 format detected but feature not enabled (cfg(not(feature = "native-v2")) guard)
- **Verified emit_error() function** displays remediation hint for human output with "Hint: " prefix and includes remediation in JSON output
- **Verified integration test** `test_native_v2_not_supported_error` passes and validates error message format
- **Manual test verified** error output includes all required elements for both human and JSON formats

## Task Commits

This was a verification plan - no commits required. All error handling infrastructure was already in place from Phase 17.

## Files Created/Modified

None - this was a verification plan only.

**Files verified:**
- `src/error.rs` - Lines 107-116: NativeV2BackendNotSupported error definition
- `src/backend/mod.rs` - Lines 118-123: NativeV2 detection returns error when feature not enabled
- `src/main.rs` - Lines 1361-1389: emit_error() function with remediation support
- `tests/backend_detection_test.rs` - Lines 98-122: Integration test for error scenario

## Decisions Made

None - followed plan as specified. All error handling infrastructure was already complete from Phase 17.

## Deviations from Plan

None - plan executed exactly as written.

## Verification Results

### 1. Error.rs Verification (lines 107-116)

- [x] NativeV2BackendNotSupported error has error code LLM-E109 (line 145)
- [x] Error message includes database path (line 110: `Database: {path}`)
- [x] Remediation includes cargo install command (line 112: `cargo install llmgrep --features native-v2`)
- [x] Remediation includes cargo build command (line 113: `cargo build --release --features native-v2`)
- [x] Documentation link included (line 114: `https://docs.rs/llmgrep/latest/llmgrep/`)

### 2. Backend/mod.rs Verification (lines 118-123)

- [x] NativeV2 format detection returns NativeV2BackendNotSupported when feature not enabled
- [x] cfg(not(feature = "native-v2")) blocks NativeV2 variant
- [x] Error path is passed to error (line 120-121)

### 3. Main.rs emit_error() Verification (lines 1361-1389)

- [x] err.remediation() is called for human output (line 1365)
- [x] Remediation is included in JSON output (line 1375)
- [x] Hint is prefixed with "Hint: " for human output (line 1366)

### 4. Integration Test Verification

- [x] test_native_v2_not_supported_error passes
- [x] Test creates native-v2 header file
- [x] Test verifies error message contains "native-v2 support" or "LLM-E109"
- [x] Test verifies error message suggests rebuilding with --features native-v2

### 5. Manual Test Verification

Test command: `./target/release/llmgrep --db /tmp/test_native_v2.db search --query "test"`

Human output:
```
ERROR [LLM-E109]: LLM-E109: Native-V2 backend detected but llmgrep was built without native-v2 support.

Database: /tmp/llmgrep_native_v2_test_XXXXXX.db

To enable Native-V2 support, rebuild llmgrep with:
   cargo install llmgrep --features native-v2
   or: cargo build --release --features native-v2

For more information, see: https://docs.rs/llmgrep/latest/llmgrep/
Hint: Rebuild llmgrep with: cargo install llmgrep --features native-v2
```

JSON output includes remediation field:
```json
{
  "code": "LLM-E109",
  "error": "error",
  "message": "...",
  "remediation": "Rebuild llmgrep with: cargo install llmgrep --features native-v2"
}
```

All must-haves verified:
- [x] Error includes clear remediation steps
- [x] Error suggests rebuilding with --features native-v2
- [x] Error includes full cargo install command for easy copy-paste
- [x] Error displays database path for clarity

## Issues Encountered

None - verification completed successfully.

## User Setup Required

None - this was a verification plan only.

## Next Phase Readiness

- Phase 20 (CLI Integration) is progressing
- Plan 20-02 complete, ready for 20-03
- All error handling infrastructure verified and working correctly
- Both SQLite and native-v2 backends properly integrated into CLI

---
*Phase: 20-cli-integration*
*Plan: 02*
*Completed: 2026-02-10*
