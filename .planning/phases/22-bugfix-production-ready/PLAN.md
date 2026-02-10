# Phase 22: Production Readiness Bugfix

**Phase:** 22
**Name:** Production Readiness Bugfix
**Status:** Pending
**Created:** 2026-02-10

## Overview

Fix critical bugs that prevent llmgrep from being production-ready with the native-v2 backend. These bugs were discovered during verification testing after Phase 21 completion.

## Goal

Deliver a production-ready llmgrep binary where:
1. Language field accurately reflects the source file language (not hardcoded "Rust")
2. No debug output pollutes stderr during normal operation
3. Binary behaves as a polished CLI tool

## Depends On

- Phase 21: Native-V2 Exclusive Features (must be complete)

## Requirements

### BUG-01: Accurate Language Detection
- **Must:** Native-V2 backend must return correct language for all supported file types
- **Must:** Python files (`.py`) return `"Python"` not `"Rust"`
- **Must:** JavaScript files (`.js`, `.mjs`) return `"JavaScript"` not `"Rust"`
- **Must:** TypeScript files (`.ts`, `.tsx`) return `"TypeScript"` not `"Rust"`
- **Must:** C/C++ files (`.c`, `.cpp`, `.h`, `.hpp`) return `"C"` or `"C++"` not `"Rust"`
- **Must:** Java files (`.java`) return `"Java"` not `"Rust"`
- **Must:** Rust files (`.rs`) continue to return `"Rust"`
- **Must:** Unknown file extensions return `"Unknown"` or sensible default

### BUG-02: Remove Debug Output from Production
- **Must:** `llmgrep complete` command produces NO debug output to stderr
- **Must:** No `eprintln!("DEBUG: ...")` statements in production code
- **Must:** stderr is only used for legitimate warnings/errors
- **Must:** Debug statements are either removed or conditionally compiled behind `#[cfg(debug_assertions)]`

### BUG-03: Verify All Search Modes
- **Must:** `search_symbols` returns accurate language field
- **Must:** `search_references` returns accurate language field (when available)
- **Must:** `search_calls` returns accurate language field (when available)
- **Must:** `lookup` command returns accurate language field
- **Must:** `complete` command produces no debug output
- **Must:** `search_by_label` returns accurate language field

## Success Criteria

1. `llmgrep --db test.py.db search --query "foo" --output json` shows `"language": "Python"`
2. `llmgrep --db test.js.db search --query "bar" --output json` shows `"language": "JavaScript"`
3. `llmgrep complete --prefix "func" 2>&1` produces NO lines containing "DEBUG"
4. All 333+ existing tests continue to pass
5. New integration tests verify language detection for all supported file types
6. New integration tests verify no debug output in complete command
7. `cargo test` passes with zero warnings (or only pre-existing unrelated warnings)

## Implementation Plan

### Plan 22-01: Add Language Inference to NativeV2Backend

**Goal:** Implement accurate language detection from file extensions

**Changes:**
1. Add `infer_language_from_extension()` helper to `src/backend/native_v2.rs`
   - Reuse or adapt existing `infer_language()` from `src/query.rs` if applicable
   - Map file extensions to language strings:
     - `.rs` → `"Rust"`
     - `.py` → `"Python"`
     - `.js` / `.mjs` / `.cjs` → `"JavaScript"`
     - `.ts` → `"TypeScript"`
     - `.tsx` / `.jsx` → `"TypeScript"` (or `"JSX"` distinct)
     - `.c` → `"C"`
     - `.cpp` / `.cc` / `.cxx` → `"C++"`
     - `.h` / `.hpp` → `"C"` or `"C++"` (based on extension)
     - `.java` → `"Java"`
     - `.go` → `"Go"`
     - `.rb` → `"Ruby"`
     - `.php` → `"PHP"`
     - Unknown → `"Unknown"` or `None`
2. Update `symbol_node_to_match()` to use inferred language instead of hardcoded `"Rust"`
3. Update `search_symbols()` to use inferred language
4. Update `search_references()` to use inferred language (when file_path available)
5. Update `search_calls()` to use inferred language (when file_path available)
6. Update `lookup()` to use inferred language
7. Update `search_by_label()` to use inferred language

**Files to modify:**
- `src/backend/native_v2.rs` (add helper, update all 7 locations)

**Testing:**
- Unit test for `infer_language_from_extension()` covering all mapped extensions
- Unit test for unknown extensions returning `"Unknown"`
- Integration test: Python file returns `"Python"`
- Integration test: JavaScript file returns `"JavaScript"`
- Integration test: TypeScript file returns `"TypeScript"`
- Integration test: C/C++ file returns `"C"` or `"C++"`
- Integration test: Java file returns `"Java"`
- Integration test: Rust file still returns `"Rust"`

### Plan 22-02: Remove Debug Output from complete() Method

**Goal:** Remove all `eprintln!("DEBUG: ...")` statements from production code

**Changes:**
1. Remove lines 522-559 from `src/backend/native_v2.rs` (all debug output in `complete()`)
2. Verify no other debug output exists in `src/backend/native_v2.rs`
3. Verify no debug output exists in other backend files
4. Consider: Should any debug output be preserved behind `#[cfg(debug_assertions)]`?
   - Decision: NO - remove entirely for cleaner production code

**Files to modify:**
- `src/backend/native_v2.rs` (remove ~40 lines of debug code)

**Testing:**
- Integration test: `llmgrep complete --prefix "test" 2>&1 | grep -i "debug"` returns exit code 1 (no matches)
- Integration test: `llmgrep complete --prefix "test" > /dev/null` produces only stdout, no stderr pollution
- Manual test: Run `complete` command and verify clean output

### Plan 22-03: Cross-Backend Verification Tests

**Goal:** Verify all search modes work correctly with language detection

**Changes:**
1. Add test fixture files in multiple languages (Python, JavaScript, TypeScript, C, Java)
2. Create native-v2 test databases for each language
3. Write integration tests verifying language field accuracy:
   - `test_python_language_detection()`
   - `test_javascript_language_detection()`
   - `test_typescript_language_detection()`
   - `test_c_language_detection()`
   - `test_java_language_detection()`
   - `test_rust_language_detection()`
4. Write integration tests verifying no debug output:
   - `test_complete_no_debug_output()`
   - `test_lookup_no_debug_output()`
   - `test_search_no_debug_output()`

**Files to create:**
- `tests/native_v2_language_test.rs` (new test file)

**Testing:**
- All new tests pass
- All existing tests continue to pass
- Run `cargo test` and verify 350+ tests passing

### Plan 22-04: Final Verification and Release

**Goal:** Verify all fixes work together and prepare for release

**Changes:**
1. Rebuild binary: `cargo install --path . --features native-v2 --force`
2. Manual testing with real codebases:
   - Test Python codebase: verify language field shows "Python"
   - Test JavaScript codebase: verify language field shows "JavaScript"
   - Test TypeScript codebase: verify language field shows "TypeScript"
   - Test Rust codebase: verify language field shows "Rust"
   - Verify `complete` command produces no debug output
3. Update documentation if needed (no changes expected for bugfix)
4. Tag and prepare release notes

**Testing:**
- Manual testing on 3+ real codebases in different languages
- All automated tests pass
- No regressions from Phase 21 baseline

## Execution Order

Plans must execute in order:
1. 22-01 (Add language inference)
2. 22-02 (Remove debug output)
3. 22-03 (Verification tests)
4. 22-04 (Final verification)

## Definition of Done

- [ ] All 4 plans complete
- [ ] All automated tests pass (350+)
- [ ] Language field accurate for Python, JavaScript, TypeScript, C/C++, Java, Rust
- [ ] No debug output in any command (stderr only for errors/warnings)
- [ ] Manual testing confirms fixes on real codebases
- [ ] Phase marked complete in ROADMAP.md
- [ ] No new warnings introduced
- [ ] Git commit with clean, descriptive message

## Rollback Plan

If issues arise:
1. Revert to Phase 21 baseline (commit before 22-01)
2. Binary continues to work with known issues (hardcoded "Rust", debug output)
3. Users can be warned about limitations in release notes

## Notes

- **Why separate Phase 22?** These are critical bugfixes that should be shipped as soon as possible. They are NOT new features, but fixes to incomplete work from Phase 21.
- **Why not part of Phase 21?** Phase 21 was marked complete before these issues were discovered during verification testing. Separating bugfixes makes the roadmap clearer about what was delivered vs. what needed fixing.
- **Estimated effort:** 2-4 hours total (simple fixes, extensive testing)
