---
phase: 021-native-v2-exclusive-features
plan: 03
subsystem: semantic-search
tags: [magellan, kv-store, labels, native-v2, sqlitegraph]

# Dependency graph
requires:
  - phase: 21-01
    provides: BackendTrait with complete() and lookup() methods
  - phase: 21-02
    provides: O(1) FQN lookup pattern and KV store usage
  - phase: 20
    provides: Backend enum and runtime backend detection
provides:
  - search_by_label() method on BackendTrait for label-based semantic search
  - SearchMode::Labels variant for --mode label CLI usage
  - --label flag for specifying label name (defaults to "test")
affects: []

# Tech tracking
tech-stack:
  added: [magellan::kv::keys::label_key, magellan::kv::encoding::decode_symbol_ids]
  patterns: [KV label lookup, two-phase query (KV + SQL), empty result handling]

key-files:
  created: []
  modified: [src/main.rs, src/backend/mod.rs, src/backend/native_v2.rs, src/backend/sqlite.rs]

key-decisions:
  - "Default label is 'test' when --label flag not specified"
  - "Empty label results return empty SearchResponse (not error)"
  - "SQLite backend returns RequiresNativeV2Backend error for label search"

patterns-established:
  - "Pattern: Native-V2 exclusive feature with graceful SQLite fallback via RequiresNativeV2Backend error"
  - "Pattern: Two-phase query - KV store for ID lookup, SQL for full metadata"
  - "Pattern: Default values for optional CLI flags using unwrap_or()"

# Metrics
duration: 7min
completed: 2026-02-10T01:32:01Z
---

# Phase 21 Plan 03: Label Search Mode Summary

**Label-based semantic search using Magellan KV store with test/entry_point/public_api categories**

## Performance

- **Duration:** 7 min
- **Started:** 2026-02-10T01:24:23Z
- **Completed:** 2026-02-10T01:32:01Z
- **Tasks:** 1
- **Files modified:** 4

## Accomplishments

- Added `SearchMode::Labels` variant to SearchMode enum for --mode label support
- Added `--label` CLI flag to Search command (defaults to "test" if not specified)
- Implemented `BackendTrait::search_by_label()` method signature
- Implemented `NativeV2Backend::search_by_label()` using `magellan::kv::label_key()` for KV lookup
- Implemented `SqliteBackend::search_by_label()` returning `RequiresNativeV2Backend` error
- Added Labels match arm in `run_search()` with default label handling
- Empty label results handled gracefully (returns empty SearchResponse, not error)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add label search mode for purpose-based queries** - `352fefc` (feat)

**Plan metadata:** (none - final docs commit after state update)

_Note: Single task plan_

## Files Created/Modified

### Modified

- `src/main.rs` - Added SearchMode::Labels variant, --label flag, Labels match arm in run_search()
- `src/backend/mod.rs` - Added search_by_label() to BackendTrait and Backend enum
- `src/backend/native_v2.rs` - Implemented NativeV2Backend::search_by_label() using KV store
- `src/backend/sqlite.rs` - Implemented SqliteBackend::search_by_label() returning error

## Implementation Details

### Label Search Architecture

Label search uses a two-phase query pattern:
1. **KV Lookup**: Use `magellan::kv::label_key()` to construct `b"label:{name}"` key and retrieve `Vec<SymbolId>`
2. **SQL Query**: Query symbol_nodes table for full metadata using retrieved IDs

### Common Labels

Labels are populated during Magellan indexing. Common labels include:
- `test` - Test functions
- `entry_point` - Main/entry functions
- `public_api` - Public API symbols

Note: Label availability depends on indexer configuration and label population during indexing.

### Error Handling

- **SQLite backend**: Returns `LlmError::RequiresNativeV2Backend` with helpful error message
- **Label not found**: Returns empty SearchResponse (not an error)
- **Invalid label value type**: Returns empty SearchResponse with notice

### CLI Usage

```bash
# Search with default label "test"
llmgrep --db code.db search --mode label

# Search with specific label
llmgrep --db code.db search --mode label --label entry_point

# JSON output
llmgrep --db code.db search --mode label --label public_api --output json
```

## Decisions Made

1. **Default label is "test"**: When `--label` flag is not specified, defaults to "test" for common test function search
2. **Empty results are not errors**: Labels may not exist or may have no symbols - return empty SearchResponse rather than error
3. **SQLite returns clear error**: SQLite backend cannot support label search, return RequiresNativeV2Backend with remediation

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

### Issue 1: File modification conflicts during editing
- **Problem**: File was being modified by linter/formatter during edits, causing "File modified since read" errors
- **Solution**: Re-read file before each edit, used larger context strings for unique matching

### Issue 2: Missing label field in pattern match
- **Problem**: Added `label` field to Search command struct but forgot to add it to the pattern match in dispatch()
- **Solution**: Added `label` to the Command::Search pattern match after `language` field

## Verification

### Build Success
- `cargo build` completed without errors
- All 151 library tests passing (zero regressions)

### Expected Behavior (Post-Verification)
- `llmgrep --db test.db search --mode label` - Should use default "test" label
- `llmgrep --db test.db search --mode label --label entry_point` - Should search for entry_point label
- `llmgrep --db sqlite.db search --mode label` - Should return LLM-E111 error

## Common Labels Documentation

Labels are semantic categories assigned to symbols during indexing:

| Label | Description | Example Symbols |
|-------|-------------|-----------------|
| `test` | Test functions | `test_*`, `#[test]` functions |
| `entry_point` | Entry points | `main()`, `lib.rs` exports |
| `public_api` | Public API | `pub fn`, `pub struct` |

**Note**: Label population depends on Mag indexer configuration. Labels must be explicitly stored during indexing using `magellan::kv::store_label()`.

## Next Phase Readiness

- Label search implementation complete
- Ready for next native-v2 exclusive feature (performance metrics)
- No blockers or concerns

---
*Phase: 021-native-v2-exclusive-features*
*Plan: 21-03*
*Completed: 2026-02-10*
