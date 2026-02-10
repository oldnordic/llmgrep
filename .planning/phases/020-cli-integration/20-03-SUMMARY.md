---
phase: 20-cli-integration
plan: 03
subsystem: error-handling
tags: [error-types, backend-variant, native-v2-only, LLM-E111]

# Dependency graph
requires:
  - phase: 17-backend-infrastructure
    provides: Backend enum with Sqlite and NativeV2 variants
provides:
  - LlmError::RequiresNativeV2Backend for native-v2-only features
  - require_native_v2() helper function for backend variant checking
affects: [21-native-v2-commands]

# Tech tracking
tech-stack:
  added: []
  patterns: [backend-variant-guard, cfg-gated-error-handling]

key-files:
  created: []
  modified: [src/error.rs, src/main.rs]

key-decisions:
  - "LLM-E111 error code assigned for native-v2-only features"
  - "require_native_v2() helper uses cfg gates to handle feature-gated NativeV2 variant"
  - "Error message includes full remediation: magellan watch --storage native-v2"

patterns-established:
  - "Pattern: Backend variant checking with match on Backend enum"
  - "Pattern: cfg-gated branches for feature-conditional error handling"

# Metrics
duration: 2min
completed: 2026-02-10
---

# Phase 20 Plan 03: Prepare Error Handling for Native-V2-Only Commands Summary

**RequiresNativeV2Backend error type with LLM-E111 code and require_native_v2() helper for backend variant checking**

## Performance

- **Duration:** 2 min
- **Started:** 2026-02-10T00:01:37Z
- **Completed:** 2026-02-10T00:04:19Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- Added `LlmError::RequiresNativeV2Backend` variant with comprehensive error message including command name, database path, and remediation steps
- Assigned error code LLM-E111 (unique, within LLM-E100-199 range for Query and parsing errors)
- Added `require_native_v2()` helper function in main.rs for checking backend variant
- Pattern established for Phase 21 to use for native-v2-only commands (complete, lookup)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add RequiresNativeV2Backend error type** - `06b1976` (feat)
2. **Task 2: Add require_native_v2 helper function** - `ac61383` (feat)

**Plan metadata:** (final summary commit to follow)

## Files Created/Modified

- `src/error.rs` - Added RequiresNativeV2Backend variant, error code LLM-E111, severity, and remediation mappings
- `src/main.rs` - Added require_native_v2() helper function with cfg-gated branches for NativeV2 variant

## Error Type Details

### LlmError::RequiresNativeV2Backend

```rust
#[error(
    "LLM-E111: The '{command}' command requires native-v2 backend.\n\n\
     Current database: {path}\n\
     Detected backend: SQLite\n\n\
     This command is only available with native-v2 storage which provides:\n\
     \x20  - O(1) KV store lookups\n\
     \x20  - Prefix-based autocomplete\n\
     \x20  - Smaller database sizes\n\n\
     To use this command:\n\
     \x20  1. Reindex your codebase with native-v2: magellan watch --root . --db code.db --storage native-v2\n\
     \x20  2. Ensure llmgrep is built with native-v2 feature: cargo install llmgrep --features native-v2\n\n\
     For more information, see: https://docs.rs/llmgrep/latest/llmgrep/"
)]
RequiresNativeV2Backend { command: String, path: String }
```

**Error code:** LLM-E111 (Query and parsing errors range)
**Severity:** error
**Remediation:** "Reindex with native-v2 storage: magellan watch --root . --db code.db --storage native-v2"

## Helper Function Pattern

### require_native_v2()

```rust
fn require_native_v2(backend: &Backend, command: &str, db_path: &Path) -> Result<(), LlmError> {
    #[cfg(feature = "native-v2")]
    {
        match backend {
            Backend::NativeV2(_) => Ok(()),
            Backend::Sqlite(_) => Err(LlmError::RequiresNativeV2Backend {
                command: command.to_string(),
                path: db_path.display().to_string(),
            }),
        }
    }
    #[cfg(not(feature = "native-v2"))]
    {
        // When native-v2 feature is disabled, all backends are SQLite
        let _ = (backend, command);
        Err(LlmError::RequiresNativeV2Backend {
            command: command.to_string(),
            path: db_path.display().to_string(),
        })
    }
}
```

**Usage in Phase 21:** Call this function at the start of `run_complete()` and `run_lookup()` commands:

```rust
fn run_complete(cli: &Cli, ...) -> Result<(), LlmError> {
    let backend = Backend::detect_and_open(&db_path)?;
    require_native_v2(&backend, "complete", &db_path)?;
    // ... rest of implementation
}
```

## Decisions Made

- **LLM-E111 error code:** Assigned in LLM-E100-199 range (Query and parsing errors) alongside other Magellan-related errors
- **cfg-gated implementation:** Helper function uses #[cfg(feature = "native-v2")] to handle the feature-gated Backend::NativeV2 variant correctly
- **Error message design:** Includes full context (command name, path, detected backend) and remediation steps to guide users

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None - all tasks completed as specified.

## Authentication Gates

None - no authentication required for this plan.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Phase 21 can now use the `require_native_v2()` helper to guard native-v2-only commands (complete, lookup). The pattern is:

1. Detect and open backend with `Backend::detect_and_open()`
2. Call `require_native_v2(&backend, "command_name", &db_path)?`
3. Continue with native-v2-specific implementation

All error handling infrastructure is in place for graceful failure on SQLite backend.

---
*Phase: 20-cli-integration*
*Plan: 03*
*Completed: 2026-02-10*
