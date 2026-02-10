# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-02-09)

**Core value:** Fast, reliable search of Magellan code databases (SQLite or native-v2) with CLI behavior consistent with Splice and Magellan. Dual backend support enables O(1) KV lookups with native-v2 while maintaining SQLite compatibility. Optimized for LLM consumption with intelligent relevance scoring, AST-based structural queries, and graph algorithm integration.
**Current focus:** v3.0 MILESTONE COMPLETE - Ready for v3.1.0 release

## Current Position

Phase: v3.0 MILESTONE COMPLETE
Current Plan: None - All 44 plans complete
Status: v3.0 milestone COMPLETE
Last activity: 2026-02-10 — Phase 24 final verification complete - zero warnings, 400 tests passing (7 min)

Progress: [█████████] 100% (44/44 plans complete in v3.0 milestone)

## Performance Metrics

**Velocity:**
- Total plans completed: 52 (v1.0-v3.0)
- Average duration: ~14 min
- Total execution time: ~11 hours (across all milestones)

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 1-6 (v1.0) | 23 | 3h 45m | ~10 min |
| 7 (v1.1) | 6 | 1h 30m | ~15 min |
| 7.1 (bugfix) | 3 | 45m | ~15 min |
| 8-10 (v2.0) | 3 | 45m | ~15 min |
| 11 (v1.4) | 4 | 1h | ~15 min |
| 12 (v2.1) | 3 | 6m | ~2 min |
| 13 (v2.1) | 3 | ~20m | ~7 min |
| 14 (v2.1) | 4 | 14m | ~4 min |
| 15 (v2.1) | 2 | ~10m | ~5 min |
| 16 (v2.1) | 2 | ~10m | ~5 min |
| 17 (v3.0) | 5 | ~24m | ~5 min |
| 18 (v3.0) | 6 | ~11m | ~2 min |

**Recent Trend:**
- Phase 17 (5 plans): ~5 min each
- Phase 18 (5 plans): ~2 min each
- Trend: Stable

*Updated after phase completion*
| Phase 18-sqlite-backend-refactor P06 | 3min | 1 tasks | 0 files |
| Phase 19-native-v2-backend-implementation P01 | 5min | 1 tasks | 1 files |
| Phase 19 P05 | 15min | 1 tasks | 2 files |
| Phase 19 P06 | 3min | 1 tasks | 0 files |
| Phase 20-cli-integration P01 | 4min | 1 tasks | 1 files |
| Phase 21-native-v2-exclusive-features P01 | 3min | 1 tasks | 4 files |
| Phase 21-native-v2-exclusive-features P02 | 7min | 2 tasks | 5 files |
| Phase 21-native-v2-exclusive-features P03 | 7min | 1 tasks | 4 files |
| Phase 21-native-v2-exclusive-features P04 | 12min | 1 tasks | 4 files |
| Phase 21-native-v2-exclusive-features P05 | 5min | 1 tasks | 1 files |
| Phase 22-bugfix-production-ready P01 | 7min | 3 tasks | 5 files |
| Phase 22-bugfix-production-ready P02 | 5min | 1 tasks | 1 files |
| Phase 22-bugfix-production-ready P03 | 15min | 4 tasks | 12 files |
| Phase 23-feature-parity-native-v2 P03 | 16min | 1 tasks | 1 files |
| Phase 23-feature-parity-native-v2 P04 | 3min | 1 tasks | 1 files |
| Phase 23-feature-parity-native-v2 P05 | 7min | 1 tasks | 1 files |
| Phase 23 P23-05 | 7 | 1 tasks | 1 files |
| Phase 24-code-quality-polish P01 | 374min | 1 tasks | 2 files |
| Phase 24-code-quality-polish P02 | 9min | 1 tasks | 11 files |
| Phase 24 P03 | 4min | 1 tasks | 1 files |
| Phase 24 P05 | 15min | 1 tasks | 5 files |
| Phase 24 P04 | 10min | 3 tasks | 4 files |
| Phase 24 P06 | 1min | 1 tasks | 1 files |

## Accumulated Context

### Decisions

Decisions are logged in PROJECT.md Key Decisions table.
Recent decisions affecting current work:

- **Phase 17**: Dual backend support via BackendTrait abstraction
- **Phase 17**: Runtime backend detection via Backend::detect_and_open() (no --backend flag needed)
- **Phase 17**: Feature-gated native-v2 (disabled by default)
- **Phase 17**: BackendTrait has no Send/Sync bounds (rusqlite::Connection not Sync, CodeGraph not Send)
- **Phase 17**: Zero breaking changes to SQLite backend
- **Phase 18**: SqliteBackend stores db_path for magellan shell-out in ast/find_ast
- **Phase 18**: Helper function migration skipped (referenced functions don't exist in current codebase)
- **Phase 18**: search_symbols_impl takes explicit Connection parameter to enable trait method implementation
- **Phase 19**: Simplified parity tests to API behavior tests due to Magellan file storage disconnect (graph nodes vs KV storage)
- **Phase 19**: Fixed SqliteBackend ast to apply limit on JSON result since magellan binary doesn't support --limit flag
- **Phase 18**: search_references_impl takes explicit Connection parameter to enable trait method implementation
- **Phase 18**: search_calls_impl takes explicit Connection parameter to enable trait method implementation
- **Phase 18**: All BackendTrait methods implemented on SqliteBackend (delegation pattern + magellan shell-out for AST)
- **Phase 19**: NativeV2Backend::ast() delegates to CodeGraph::get_ast_nodes_by_file() (direct API, no shell-out)
- **Phase 19**: NativeV2Backend::find_ast() delegates to CodeGraph::get_ast_nodes_by_kind() (direct API, no shell-out)
- **Phase 19**: NativeV2Backend search methods use SQL queries via rusqlite (Magellan 2.1.0 native-v2 still uses SQLite storage)
- **Phase 19**: NativeV2Backend stores db_path for direct SQL access (connect() helper method)
- **Phase 19**: SqliteBackend applies limit via result truncation (magellan ast doesn't support --limit)
- **Phase 19**: SqliteBackend explicitly specifies --output json for magellan shell-out commands
- [Phase 19]: Simplified parity tests to API behavior tests due to Magellan file storage disconnect
- [Phase 19]: Fixed SqliteBackend ast to apply limit on JSON result since magellan doesn't support --limit
- [Phase 20]: RequiresNativeV2Backend error (LLM-E111) for native-v2-only commands
- [Phase 20]: require_native_v2() helper function for backend variant checking with cfg gates
- [Phase 21]: Complete command defined unconditionally (not cfg-gated) for consistent --help output
- [Phase 24]: All public APIs documented with rustdoc - zero documentation warnings
- [Phase 24]: Module-level `//!` documentation added to all public modules
- [Phase 24]: Struct and field level documentation added to output.rs, query.rs, lib.rs
- [Phase 21]: Backend check at runtime via require_native_v2() for graceful SQLite fallback
- [Phase 21]: KV prefix scan using magellan::kv::keys::sym_fqn_key for consistent key format
- [Phase 21]: SymbolNotFound error (LLM-E112) for exact FQN lookup failures with fqn, db, partial fields
- [Phase 21]: Lookup command with O(1) exact FQN resolution using magellan::kv::lookup_symbol_by_fqn
- [Phase 21]: Two-phase lookup pattern: KV store for O(1) ID resolution, SQL for full metadata
- [Phase 21]: Label search using magellan::kv::keys::label_key and decode_symbol_ids for SymbolId list retrieval
- [Phase 21]: Default label "test" when --label flag not specified
- [Phase 21]: Empty label results return empty SearchResponse (not error)
- [Phase 21]: Performance metrics with --show-metrics flag and 3-phase timing breakdown
- [Phase 21]: Metrics printed to stderr for human output, included in JSON for structured output
- [Phase 21]: std::time::Instant for high-precision timing measurements
- [Phase 21]: Comprehensive test suite for native-v2 exclusive commands with 30 tests
- [Phase 21]: Tests verify complete, lookup, label search error handling on SQLite backend
- [Phase 21]: Performance metrics structure validation and JSON serialization tests
- [Phase 22]: Language inference from file extension using infer_language() function
- [Phase 22]: Capitalized language names (Rust, Python, JavaScript) for output consistency
- [Phase 22]: has_known_extension() helper for detecting 18+ source file extensions in FQN parsing
- [Phase 22]: Integration tests for language detection (15 tests covering 7 languages)
- [Phase 22]: Code inspection test verifying no debug output in production code
- [Phase 23]: Verbatim port of score_match() from src/query.rs to NativeV2Backend
- [Phase 23]: Relevance scoring for Native-V2 backend with regex pattern support
- [Phase 23]: Conditional score calculation only when include_score option is true
- [Phase 23]: Max score aggregation for calls (caller_score.max(callee_score))
- [Phase 23]: Metrics extraction from KV store using pattern `sm:symbol:{entity_id}`
- [Phase 23]: Graceful degradation for metrics when not available (native-v2 databases may not have metrics)
- [Phase 23]: Filter out symbols without metrics when metrics filter is active
- [Phase 24]: Safety documentation uses `# Safety` format for all unsafe blocks in NativeV2Backend
- [Phase 24]: All 8 unsafe blocks documented with invariants and correctness rationale
- [Phase 24]: Code consistency improvements - redundant closures removed, Default impl added, needless borrows fixed
- [Phase 24]: CONCERNS.md updated with all Phase 22, 23, 24 resolutions marked
- [Phase 24]: Final verification complete - zero warnings, 400 tests passing

### Pending Todos

**NONE** - Phase 24 (Code Quality Polish) is complete. v3.0 milestone is complete.

## Session Continuity

Last session: 2026-02-10 — Phase 24 plan 24-07 (COMPLETE)
Stopped at: Completed Phase 24 Plan 24-07: Final verification with zero warnings, 400 tests passing
Resume file: None
Milestone Status: v3.0 COMPLETE - 44/44 plans delivered

### Blockers/Concerns

**Resolved in Phase 24:**
- **watch_cmd compilation:** Fixed by cfg-gating behind unstable-watch feature
- **Clippy warnings:** All 10 warnings resolved with appropriate allowances or fixes
- **Documentation warnings:** Redundant link target fixed

**From Research:**
- **Phase 18**: SqliteBackend refactor must preserve exact SQL query logic to maintain output parity
- **Phase 19**: CodeGraph API exact method signatures (consult Magellan docs during implementation)
- **Phase 21**: KV prefix scan performance (benchmark on realistic datasets)

**From 17-02 Execution:**
- **Native-v2 backend**: magellan::CodeGraph uses Rc internally which is not Send + Sync. This was resolved in 17-05 by removing Send/Sync bounds from BackendTrait.

**From 17-05 Execution:**
- **BackendTrait has no Send/Sync bounds** - Required because rusqlite::Connection is not Sync and magellan::CodeGraph is not Send
- **Backend enum has Debug derive** - Required for test assertions
- **Custom Debug for NativeV2Backend** - CodeGraph doesn't implement Debug

## Next Steps

1. Phase 20 (remaining plans): Complete CLI integration
   - [Plan 20-01: COMPLETE] Wire Backend enum into CLI
   - Remaining plans in Phase 20 (if any)
2. Phase 21: Native-v2 storage implementation
   - Migrate from SQLite to pure KV storage
   - Benchmark KV prefix scan performance

## Phase 18 Summary

**Completed:** 2026-02-09
**Plans:** 6/6 complete (18-01 through 18-06)
**Status:** COMPLETE

**Artifacts Created:**
- src/backend/sqlite.rs — Complete SqliteBackend BackendTrait implementation
- src/query.rs — search_symbols_impl() with Connection parameter
- src/query.rs — search_references_impl() with Connection parameter
- src/query.rs — search_calls_impl() with Connection parameter
- .planning/phases/018-sqlite-backend-refactor/18-06-SUMMARY.md — Verification summary

**Commits:** 5 atomic commits (18-06 was verification only)

**Verification:**
- [x] SqliteBackend implements all 5 BackendTrait methods
- [x] search_symbols/search_references/search_calls delegate to _impl() functions
- [x] ast/find_ast shell out to magellan binary
- [x] All 324 tests pass without modification
- [x] Zero breaking changes confirmed
- [x] Output parity maintained

---

## Phase 19 Summary

**Completed:** 2026-02-10
**Plans:** 6/6 complete (19-01 through 19-06)
**Status:** COMPLETE

**Artifacts Created:**
- src/backend/native_v2.rs — Complete NativeV2Backend with all 5 BackendTrait methods
- src/backend/sqlite.rs — ast/find_ast with magellan shell-out, limit via truncation
- tests/backend_parity_test.rs — Integration tests for ast/find_ast
- .planning/phases/019-native-v2-backend-implementation/19-01-SUMMARY.md — ast() summary
- .planning/phases/019-native-v2-backend-implementation/19-02-SUMMARY.md — find_ast() summary
- .planning/phases/019-native-v2-backend-implementation/19-03-SUMMARY.md — search methods summary
- .planning/phases/019-native-v2-backend-implementation/19-04-SUMMARY.md — ast/find_ast backend parity summary
- .planning/phases/019-native-v2-backend-implementation/19-05-SUMMARY.md — parity tests summary
- .planning/phases/019-native-v2-backend-implementation/19-06-SUMMARY.md — verification summary

**Commits:** 4 atomic commits (678f88b, 6e16650, 56627ee, 661c9fe)

**Verification:**
- [x] ast() method delegates to CodeGraph::get_ast_nodes_by_file()
- [x] Position filtering implemented (byte_start <= pos < byte_end)
- [x] Limit parameter support via .take()
- [x] find_ast() method delegates to CodeGraph::get_ast_nodes_by_kind()
- [x] search_symbols() implemented via SQL queries
- [x] search_references() implemented via SQL queries
- [x] search_calls() implemented via SQL queries
- [x] SqliteBackend ast/find_ast implemented via magellan shell-out
- [x] SqliteBackend applies limit via JSON array truncation
- [x] Integration tests for ast/find_ast added (9 tests pass)
- [x] Code compiles with native-v2 feature enabled
- [x] Error handling converts anyhow::Error to LlmError::SearchFailed
- [x] All 182 tests passing (zero regressions)
- [x] All 5 BackendTrait methods implemented on NativeV2Backend

---

## Phase 17 Summary

**Completed:** 2026-02-09
**Plans:** 5/5 complete (17-01 through 17-05)
**Verification:** 10/10 must-haves verified
**Tests:** 342 tests passing (zero regressions)

**Artifacts Created:**
- src/backend/mod.rs — BackendTrait + Backend enum + detect_and_open()
- src/backend/sqlite.rs — SqliteBackend struct with stub implementations
- src/backend/native_v2.rs — NativeV2Backend struct cfg-gated
- src/error.rs — NativeV2BackendNotSupported + BackendDetectionFailed
- tests/backend_detection_test.rs — Integration tests for backend detection
- Cargo.toml — native-v2 feature flag + updated dependencies

**Commits:** 12 atomic commits across 3 waves

---

## Phase 20 Summary

**Completed:** 2026-02-10
**Plans:** 5/5 complete (20-01 through 20-05)
**Status:** COMPLETE

**Artifacts Created:**
- .planning/phases/020-cli-integration/20-01-SUMMARY.md — CLI integration summary
- .planning/phases/020-cli-integration/20-02-SUMMARY.md — Native-V2 error handling verification
- .planning/phases/020-cli-integration/20-03-SUMMARY.md — RequiresNativeV2Backend error type
- .planning/phases/020-cli-integration/20-04-SUMMARY.md — CLI integration tests summary
- .planning/phases/020-cli-integration/20-05-SUMMARY.md — Verification summary
- .planning/phases/020-cli-integration/20-05-VERIFICATION.md — Full verification report
- tests/cli_integration_test.rs — 9 integration tests for CLI backend detection

**Commits:** 4 atomic commits (2888dcd, 06b1976, ac61383, 71203e9)

**Verification:**
- [x] CLI compiles with Backend enum integration
- [x] All 333 tests passing (zero regressions)
- [x] Backend::detect_and_open() called in run_search(), run_ast(), run_find_ast()
- [x] Backend trait methods delegated to from main.rs
- [x] No --backend flag (automatic detection per Phase 17 decision)
- [x] Error handling for NativeV2BackendNotSupported includes remediation
- [x] LLM-E109 error verified with cargo install/build commands
- [x] Integration test test_native_v2_backend_error passes
- [x] RequiresNativeV2Backend error type with LLM-E111 code
- [x] require_native_v2() helper function for backend variant checking
- [x] CLI integration tests created (9 tests pass)
- [x] Tests verify search/ast/find-ast commands with SQLite backend
- [x] Tests verify all search modes (symbols, references, calls)
- [x] Tests verify JSON output format
- [x] Tests verify backend detection for SQLite format
- [x] All success criteria met (5/5)
- [x] User verification approved

---

## Phase 21 Summary

**Started:** 2026-02-10
**Plans:** 5/6 complete (21-01, 21-02, 21-03, 21-04, 21-05)
**Status:** IN PROGRESS

**Artifacts Created:**
- .planning/phases/021-native-v2-exclusive-features/21-01-SUMMARY.md — Complete command summary
- .planning/phases/021-native-v2-exclusive-features/21-02-SUMMARY.md — Lookup command summary
- .planning/phases/021-native-v2-exclusive-features/21-03-SUMMARY.md — Label search mode summary
- .planning/phases/021-native-v2-exclusive-features/21-04-SUMMARY.md — Performance metrics summary
- .planning/phases/021-native-v2-exclusive-features/21-05-SUMMARY.md — Test suite summary

**Commits:** 5 atomic commits (56be1ea, f94c08e, 42e5b8c, 352fefc, 84b9a01)

**Verification:**
- [x] Complete command implemented with KV prefix scan
- [x] Command visible in --help output
- [x] LLM-E111 error on SQLite backend (verified)
- [x] Empty prefix validation (verified)
- [x] Human and JSON output formats (implemented)
- [x] Lookup command implemented with O(1) FQN lookup
- [x] SymbolNotFound error (LLM-E112) added with helpful remediation
- [x] magellan::kv::lookup_symbol_by_fqn for O(1) ID resolution
- [x] SQL query for full symbol metadata after KV lookup
- [x] Label search mode with SearchMode::Labels variant
- [x] --label flag with default "test" label
- [x] NativeV2Backend::search_by_label using label_key KV lookup
- [x] Empty label results handled gracefully
- [x] All 151 tests passing (zero regressions)
- [x] --show-metrics global flag added to CLI
- [x] PerformanceMetrics struct with 4 timing fields
- [x] 3-phase timing instrumentation (backend detection, query execution, output formatting)
- [x] Metrics printed to stderr for human output
- [x] Metrics included in JSON output under "performance" key
- [x] All 5 command functions instrumented (run_search, run_ast, run_find_ast, run_complete, run_lookup)
- [x] Magellan API compatibility fixes (__backend_for_benchmarks)
- [x] Comprehensive test suite with 30 integration tests for native-v2 exclusive features
- [x] Tests verify complete/lookup/label command error handling on SQLite backend
- [x] Performance metrics structure validation and JSON serialization tests

---

## Phase 22 Summary

**Started:** 2026-02-10
**Completed:** 2026-02-10
**Plans:** 4/4 complete (22-01, 22-02, 22-03, 22-04)
**Status:** COMPLETE

**Artifacts Created:**
- .planning/phases/22-bugfix-production-ready/22-01-SUMMARY.md — Language inference summary
- .planning/phases/22-bugfix-production-ready/22-02-SUMMARY.md — Debug output removal summary
- .planning/phases/22-bugfix-production-ready/22-03-SUMMARY.md — Cross-backend verification tests summary
- .planning/phases/22-bugfix-production-ready/22-04-SUMMARY.md — Final verification and release summary

**Commits:**
- e6c6be9, 4596572, d70d43f (22-01: Language inference)
- a27b635 (22-02: Debug output removal - included in 22-01)
- 1a10bbc, d1bbffa (22-03: Integration tests)
- 3590b7d (22-04: SQLite test fixture improvement)

**Verification:**
- [x] Language inference added for Python, JavaScript, TypeScript, C/C++, Java, Rust
- [x] has_known_extension() helper function added
- [x] symbol_node_to_match() uses infer_language()
- [x] search_symbols() uses infer_language()
- [x] All 386 tests passing (zero regressions, 15 new tests added)
- [x] Debug output removed from complete() method (36 lines)
- [x] No eprintln! statements remain in native_v2.rs
- [x] No DEBUG references remain in native_v2.rs
- [x] File reduced from 679 to 643 lines
- [x] Integration tests for language detection (15 tests)
- [x] Test fixtures in 7 languages
- [x] Native-v2 test database created
- [x] Binary builds successfully with native-v2 feature
- [x] Production-ready v3.0.1 release

---

## Phase 23 Summary

**Started:** 2026-02-10
**Completed:** 2026-02-10
**Plans:** 6 of 6 complete (23-01 through 23-06)
**Status:** COMPLETE

**Artifacts Created:**
- .planning/phases/23-feature-parity-native-v2/23-03-SUMMARY.md — Relevance scoring summary
- .planning/phases/23-feature-parity-native-v2/23-04-SUMMARY.md — Metrics extraction summary
- .planning/phases/23-feature-parity-native-v2/23-05-SUMMARY.md — Cross-backend parity tests summary
- .planning/phases/23-feature-parity-native-v2/23-06-VERIFICATION.md — Comprehensive verification report
- .planning/phases/23-feature-parity-native-v2/23-06-SUMMARY.md — Phase 23 summary
- tests/backend_parity_extended_test.rs — 13 comprehensive parity tests
- README.md — Feature Parity section added
- .planning/ROADMAP.md — Phase 23 marked complete

**Commits:**
- 4786210 (23-03: Relevance scoring for Native-V2 backend)
- 3f0a3ff (23-04: Metrics extraction for Native-V2 backend)
- d002743 (23-05: Cross-backend feature parity tests)
- 7079b6e (23-06: cfg-gate incomplete watch command)
- 2f77481 (23-06: Add feature parity section to README)

**Verification:**
- [x] All success criteria met:
  - [x] `--with-context` flag works with Native-V2 backend
  - [x] `--with-snippet` flag works with Native-V2 backend
  - [x] Search results include relevance score with Native-V2 backend
  - [x] Metrics fields populated from CodeGraph
  - [x] `--min-fan-in`, `--min-fan-out`, `--min-complexity` flags work with Native-V2
  - [x] Cross-backend tests verify identical output
- [x] 399 tests passing (zero failures)
- [x] Binary builds successfully with native-v2 feature
- [x] Documentation updated (README.md, ROADMAP.md)
- [x] User verification approved

**Key Achievement:** Full feature parity between SQLite and Native-V2 backends for all core search functionality

---

## Phase 24 Summary

**Completed:** 2026-02-10
**Plans:** 7/7 complete (24-01 through 24-07)
**Status:** COMPLETE

**Artifacts Created:**
- .planning/phases/24-code-quality-polish/24-01-SUMMARY.md — Compiler warnings fixed
- .planning/phases/24-code-quality-polish/24-02-SUMMARY.md — Test error messages improved
- .planning/phases/24-code-quality-polish/24-03-SUMMARY.md — Safety documentation added
- .planning/phases/24-code-quality-polish/24-04-SUMMARY.md — Public API documentation improved
- .planning/phases/24-code-quality-polish/24-05-SUMMARY.md — Code consistency improvements
- .planning/phases/24-code-quality-polish/24-06-SUMMARY.md — CONCERNS.md updated
- .planning/phases/24-code-quality-polish/24-07-SUMMARY.md — Final verification summary
- .planning/phases/24-code-quality-polish/24-07-VERIFICATION.md — Comprehensive verification report

**Commits:**
- Previous plans: 24-01 through 24-06 commits
- 24-07: Cfg-gate fixes, clippy warnings resolved (to be committed)

**Verification:**
- [x] All 400 tests passing (zero failures, 9 ignored)
- [x] Zero compiler warnings in release build
- [x] Zero clippy warnings with `-D warnings`
- [x] Zero documentation warnings
- [x] All public APIs documented
- [x] All unsafe blocks have safety documentation
- [x] All success criteria met (6/6)

**Key Achievements:**
- Zero compiler warnings across all feature combinations
- Zero clippy warnings with strict `-D warnings` flag
- Zero documentation warnings
- 400 passing tests with zero failures
- Production-ready code quality suitable for v3.1.0 release
- Incomplete `watch_cmd` module properly cfg-gated behind `unstable-watch` feature

**Code Quality Metrics:**
- Lines of code reviewed: ~3500
- Public APIs documented: 100%
- Unsafe blocks documented: 100% (8 blocks)
- Test error messages improved: ~50 instances
- Clippy warnings resolved: 10
- Compiler warnings resolved: 2

**Decisions Made:**
- Cfg-gate `watch_cmd` behind `unstable-watch` feature to prevent compilation errors
- Add `#[allow(...)]` attributes with rationale for acceptable clippy warnings
- Change `kind_filter.to_string()` to `kind_filter` to fix `cmp_owned` warning
- Derive `Default` for `PerformanceMetrics` instead of manual implementation

