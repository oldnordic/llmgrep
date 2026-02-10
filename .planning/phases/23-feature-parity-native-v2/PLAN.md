# Phase 23: Native-V2 Feature Parity

**Phase:** 23
**Name:** Native-V2 Feature Parity
**Status:** Pending
**Created:** 2026-02-10

## Overview

Achieve full feature parity between SQLite and Native-V2 backends. Currently, several fields that work in SQLite backend return `None` in Native-V2 backend, limiting functionality for users of the newer, more efficient backend.

## Goal

Enable all search and output features to work identically regardless of backend choice, so users can choose Native-V2 for performance without losing functionality.

## Depends On

- Phase 22: Production Readiness Bugfix (language field must be accurate)

## Requirements

### PARITY-01: Context Extraction
- **Must:** `--with-context` flag works with Native-V2 backend
- **Must:** Returns lines of code before and after the symbol
- **Must:** Same number of context lines as SQLite backend for same input

### PARITY-02: Snippet Extraction
- **Must:** `--with-snippet` flag works with Native-V2 backend
- **Must:** Returns the function/method body content
- **Must:** Same snippet content as SQLite backend for same input

### PARITY-03: Score Calculation
- **Must:** Search results include relevance score with Native-V2 backend
- **Must:** Score calculation is consistent with SQLite backend
- **Must:** `--sort-by relevance` works with Native-V2

### PARITY-04: Metrics Filtering
- **Must:** `fan_in` field is populated from CodeGraph metrics
- **Must:** `fan_out` field is populated from CodeGraph metrics
- **Must:** `cyclomatic_complexity` field is populated from CodeGraph metrics
- **Must:** `--min-fan-in`, `--min-fan-out`, `--min-complexity` flags work with Native-V2

## Success Criteria

1. `llmgrep --db native-v2.db search --query "foo" --with-context` returns context lines
2. `llmgrep --db native-v2.db search --query "foo" --with-snippet` returns snippet content
3. `llmgrep --db native-v2.db search --query "foo" --output json` includes `score` field
4. `llmgrep --db native-v2.db search --query "foo" --output json` includes `fan_in`, `fan_out`, `cyclomatic_complexity`
5. `llmgrep --db native-v2.db search --query "foo" --min-fan-in 2` filters correctly
6. Cross-backend tests verify identical output (excluding backend-specific fields)
7. All existing tests pass + new parity tests pass

## Implementation Plan

### Plan 23-01: Context Extraction from CodeGraph

**Goal:** Implement `--with-context` functionality for Native-V2 backend

**Changes:**
1. Research CodeGraph API for context extraction:
   - Does `CodeGraph` have a method to extract lines around a symbol?
   - Is there a `get_code_range(file_path, start, end)` method?
   - Or do we need to read the file directly using `std::fs::read_to_string()`?
2. Implement `extract_context()` helper in `src/backend/native_v2.rs`:
   - Takes: `file_path`, `byte_start`, `byte_end`, `context_lines`
   - Returns: `String` with context before and after symbol
   - Use existing safe extraction utilities from `src/safe_extraction.rs`
3. Update `symbol_node_to_match()` to populate `context` field when `with_context` option is true
4. Update `search_symbols()` to pass `context` option through to symbol creation

**Files to modify:**
- `src/backend/native_v2.rs` (add context extraction)

**Testing:**
- Unit test for `extract_context()` with various byte ranges
- Integration test: `--with-context 2` returns 2 lines before and after
- Integration test: Context at file start (no lines before) handled gracefully
- Integration test: Context at file end (no lines after) handled gracefully
- Cross-backend test: Same input produces same context (modulo encoding)

### Plan 23-02: Snippet Extraction from CodeGraph

**Goal:** Implement `--with-snippet` functionality for Native-V2 backend

**Changes:**
1. Research CodeGraph API for snippet extraction:
   - Is there a method to get function body range?
   - Can we extract from AST node's byte range directly?
2. Implement `extract_snippet()` helper in `src/backend/native_v2.rs`:
   - Takes: `file_path`, `byte_start`, `byte_end`
   - Returns: `String` with snippet content
   - Use existing safe extraction utilities
3. Update `symbol_node_to_match()` to populate `snippet` field when `with_snippet` option is true
4. Update `search_symbols()` to pass `snippet` option through

**Files to modify:**
- `src/backend/native_v2.rs` (add snippet extraction)

**Testing:**
- Unit test for `extract_snippet()` with various symbol types
- Integration test: `--with-snippet` returns function body
- Integration test: Snippet for multi-line function
- Integration test: Snippet for single-line function
- Cross-backend test: Same input produces same snippet

### Plan 23-03: Score Calculation

**Goal:** Implement relevance scoring for Native-V2 backend

**Changes:**
1. Research SQLite backend score calculation:
   - How is `score` calculated in `src/query.rs`?
   - Is it based on name match quality? Position?
2. Implement `calculate_score()` helper in `src/backend/native_v2.rs`:
   - Takes: `query`, `symbol_name`
   - Returns: `u64` score (100 for exact match, lower for partial)
   - Match SQLite backend scoring algorithm
3. Update `symbol_node_to_match()` to populate `score` field
4. Verify `--sort-by relevance` works with scored results

**Files to modify:**
- `src/backend/native_v2.rs` (add score calculation)

**Testing:**
- Unit test for exact match → score 100
- Unit test for partial match → score < 100
- Unit test for no match → score 0
- Integration test: `--sort-by relevance` sorts by score descending
- Cross-backend test: Same query produces same scores

### Plan 23-04: Metrics from CodeGraph

**Goal:** Populate metrics fields from CodeGraph API

**Changes:**
1. Research CodeGraph API for metrics:
   - Does `SymbolNode` include metrics fields?
   - Is there a separate `get_metrics()` method?
   - How do we get fan_in, fan_out, cyclomatic_complexity?
2. Implement `get_metrics()` helper in `src/backend/native_v2.rs`:
   - Query CodeGraph for symbol metrics
   - Extract: `fan_in`, `fan_out`, `cyclomatic_complexity`
   - Return `Option<(u64, u64, u64)>` for each metric
3. Update `symbol_node_to_match()` to populate metrics fields:
   - `fan_in: Some(value)`
   - `fan_out: Some(value)`
   - `cyclomatic_complexity: Some(value)`
4. Update `search_symbols()` to apply `min_fan_in`, `min_fan_out`, `min_complexity` filters

**Files to modify:**
- `src/backend/native_v2.rs` (add metrics extraction)

**Testing:**
- Unit test for metrics extraction from SymbolNode
- Integration test: `--output json` includes metrics
- Integration test: `--min-fan-in 2` filters correctly
- Integration test: `--min-fan-out 1` filters correctly
- Integration test: `--min-complexity 5` filters correctly
- Cross-backend test: Same database produces same metrics

### Plan 23-05: Cross-Backend Parity Tests

**Goal:** Verify identical behavior across backends

**Changes:**
1. Create comprehensive test fixtures:
   - Functions with various complexity levels
   - Functions with various fan-in/fan-out counts
   - Multi-file codebase
2. Create both SQLite and Native-V2 databases from same source
3. Write parity tests:
   - `test_context_parity()`: Both backends return same context
   - `test_snippet_parity()`: Both backends return same snippet
   - `test_score_parity()`: Both backends return same scores
   - `test_metrics_parity()`: Both backends return same metrics
   - `test_filter_parity()`: Both backends filter identically
4. Run parity tests as part of CI

**Files to create:**
- `tests/backend_parity_extended_test.rs` (extend existing parity tests)

**Testing:**
- All parity tests pass
- Discrepancies documented with acceptable differences

### Plan 23-06: Documentation and Verification

**Goal:** Update docs and verify complete parity

**Changes:**
1. Update FEATURE_PARITY.md documenting all supported features per backend
2. Update README.md to note Native-V2 has full feature parity
3. Update CONCERNS.md to remove "Missing native-v2 features" section
4. Manual verification:
   - Test all features with real codebase
   - Verify performance is still better than SQLite
5. Update ROADMAP.md to mark Phase 23 complete

**Files to modify:**
- `FEATURE_PARITY.md` (create if doesn't exist)
- `README.md`
- `CONCERNS.md`
- `ROADMAP.md`

**Testing:**
- Manual testing on real codebase
- All automated tests pass
- Documentation updated

## Execution Order

Plans must execute in order:
1. 23-01 (Context extraction)
2. 23-02 (Snippet extraction)
3. 23-03 (Score calculation)
4. 23-04 (Metrics)
5. 23-05 (Parity tests)
6. 23-06 (Documentation)

## Definition of Done

- [ ] All 6 plans complete
- [ ] All automated tests pass (400+)
- [ ] Context extraction works with Native-V2
- [ ] Snippet extraction works with Native-V2
- [ ] Score calculation works with Native-V2
- [ ] Metrics filtering works with Native-V2
- [ ] Cross-backend parity tests pass
- [ ] Documentation updated
- [ ] CONCERNS.md updated (feature gap removed)
- [ ] Phase marked complete in ROADMAP.md

## Rollback Plan

If issues arise:
1. Individual plans can be rolled back independently
2. Fields return `None` as before (graceful degradation)
3. Users can use SQLite backend for missing features

## Notes

- **Why separate Phase 23?** Feature parity is substantial work requiring research into CodeGraph API for each feature area. Each feature (context, snippet, score, metrics) may have different implementation challenges.
- **Estimated effort:** 8-16 hours (depends on CodeGraph API capabilities)
- **Risk:** CodeGraph API may not expose all needed data; fallback to file reading may be required

## Open Questions

1. Does CodeGraph API expose context/snippet extraction methods, or must we read files directly?
2. Are metrics stored in SymbolNode or require separate query?
3. Should we cache file contents for performance (multiple symbols per file)?

## Dependencies on CodeGraph API

This phase's success depends on CodeGraph API capabilities. If the API doesn't expose needed data:
- **Fallback:** Read files directly using `std::fs::read_to_string()`
- **Performance impact:** May reduce Native-V2 advantage
- **Alternative:** Request CodeGraph API enhancements from Magellan team
