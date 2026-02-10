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

**Current State Analysis:**
- `src/query.rs` has `infer_language(file_path: &str) -> Option<&'static str>` (lines 234-259)
- This function is NOT exported from the module (private)
- `SymbolNode` from magellan does NOT have a `file_path` field
- File path must be extracted from `canonical_fqn` format: `"crate_name::file_path::kind symbol_name"`
- Current extraction at line 79-92 only looks for `/` or `.rs` - misses `.py`, `.js`, `.ts`, etc.

**Implementation Steps:**

1. **Export `infer_language` from query module:**
   - Add `pub use query::infer_language;` to `src/lib.rs`
   - Or move `infer_language()` to a more appropriate module

2. **Enhance file_path extraction in `symbol_node_to_match()`:**
   - Current code (line 86): `parts.iter().find(|p| p.contains('/') || p.ends_with(".rs"))`
   - Change to: `parts.iter().find(|p| p.contains('/') || has_known_extension(p))`
   - Add helper to detect known extensions: `.rs`, `.py`, `.js`, `.ts`, `.tsx`, `.jsx`, `.c`, `.cpp`, `.h`, `.java`, `.go`, `.rb`, `.php`, etc.

3. **Update hardcoded language at line 121:**
   - Current: `language: Some("Rust".to_string())`
   - Change to: `language: infer_language(&file_path).map(|s| s.to_string())`

4. **Update hardcoded language in `search_symbols()` (line 228):**
   - Current: `language: Some("Rust".to_string())`
   - Change to: `language: infer_language(&file_path_str).map(|s| s.to_string())`

5. **Verify no other hardcoded "Rust" strings exist:**
   - Check `search_references()` - uses `SymbolMatch` from `symbol_node_to_match()` (covered)
   - Check `search_calls()` - constructs `CallMatch` (no language field, OK)
   - Check `lookup()` - uses `symbol_node_to_match()` (covered)
   - Check `search_by_label()` - uses `symbol_node_to_match()` (covered)

**Code Changes - Specific:**

```rust
// In src/lib.rs - add to re-exports:
pub use query::infer_language;

// In src/backend/native_v2.rs - add helper:
fn has_known_extension(path: &str) -> bool {
    path.ends_with(".rs") || path.ends_with(".py") || path.ends_with(".js")
        || path.ends_with(".ts") || path.ends_with(".tsx") || path.ends_with(".jsx")
        || path.ends_with(".c") || path.ends_with(".cpp") || path.ends_with(".cc")
        || path.ends_with(".cxx") || path.ends_with(".h") || path.ends_with(".hpp")
        || path.ends_with(".java") || path.ends_with(".go") || path.ends_with(".rb")
        || path.ends_with(".php") || path.ends_with(".swift") || path.ends_with(".kt")
        || path.ends_with(".kts") || path.ends_with(".scala") || path.ends_with(".lua")
        || path.ends_with(".r") || path.ends_with(".m") || path.ends_with(".cs")
}

// Update line 86 in symbol_node_to_match():
parts.iter().find(|p| p.contains('/') || has_known_extension(p))

// Update line 121 in symbol_node_to_match():
language: infer_language(&file_path).map(|s| s.to_string()),

// Update line 228 in search_symbols():
language: infer_language(&file_path_str).map(|s| s.to_string()),
```

**Files to modify:**
- `src/lib.rs` (add re-export)
- `src/backend/native_v2.rs` (add helper, update lines 86, 121, 228)

**Testing:**
- Unit test for `has_known_extension()` covering all extensions
- Unit test for file_path extraction with `.py`, `.js`, `.ts` files
- Integration test: Python file returns `"Python"`
- Integration test: JavaScript file returns `"JavaScript"`
- Integration test: TypeScript file returns `"TypeScript"`
- Integration test: C/C++ file returns `"C"` or `"C++"`
- Integration test: Java file returns `"Java"`
- Integration test: Rust file still returns `"Rust"`

### Plan 22-02: Remove Debug Output from complete() Method

**Goal:** Remove all `eprintln!("DEBUG: ...")` statements from production code

**Current State Analysis:**
- `src/backend/native_v2.rs` lines 521-559 contain debug output in `complete()` method
- These lines were added for KV store debugging during development
- Total of ~40 lines to remove

**Specific Lines to Remove:**

```rust
// Line 521-536: First debug block - scan 'sym:' prefix
match backend.kv_prefix_scan(snapshot, b"sym:") {
    Ok(all_entries) => {
        eprintln!("DEBUG: Found {} total KV entries with 'sym:' prefix", all_entries.len());
        if all_entries.is_empty() {
            eprintln!("DEBUG: KV store is completely empty for 'sym:' prefix!");
        }
        for (k, v) in all_entries.iter().take(20) {
            let key_str = String::from_utf8(k.clone()).unwrap_or_default();
            eprintln!("DEBUG:   {:?} -> {:?}", key_str, v);
        }
    }
    Err(e) => {
        eprintln!("DEBUG: Error scanning 'sym:' prefix: {}", e);
    }
}

// Line 538-552: Second debug block - scan all keys
match backend.kv_prefix_scan(snapshot, b"") {
    Ok(all_entries) => {
        eprintln!("DEBUG: Found {} total KV entries (all keys)", all_entries.len());
        for (k, v) in all_entries.iter().take(20) {
            let key_str = String::from_utf8(k.clone()).unwrap_or_default();
            if key_str.len() < 100 {
                eprintln!("DEBUG:   {:?} -> {:?}", key_str, v);
            }
        }
    }
    Err(e) => {
        eprintln!("DEBUG: Error scanning all keys: {}", e);
    }
}

// Line 554-559: Third debug block - actual query
eprintln!("DEBUG: Scanning KV for prefix: {:?}", String::from_utf8_lossy(&prefix_key));
let entries = backend.kv_prefix_scan(snapshot, &prefix_key)
    .map_err(|e| LlmError::SearchFailed {
        reason: format!("KV prefix scan failed: {}", e),
    })?;
eprintln!("DEBUG: Found {} KV entries for prefix", entries.len());
```

**Implementation Steps:**

1. **Remove lines 521-559 entirely:**
   - Delete the entire debug section
   - Keep only the actual KV query that returns results

2. **Simplified `complete()` method after removal:**
   ```rust
   fn complete(&self, prefix: &str, limit: usize) -> Result<Vec<String>, LlmError> {
       use magellan::kv::keys::sym_fqn_key;

       let prefix_key = sym_fqn_key(prefix);
       let snapshot = SnapshotId::current();
       let backend = self.backend();

       let entries = backend.kv_prefix_scan(snapshot, &prefix_key)
           .map_err(|e| LlmError::SearchFailed {
               reason: format!("KV prefix scan failed: {}", e),
           })?;

       let completions: Vec<String> = entries
           .iter()
           .filter_map(|(key, _value)| {
               let key_str = String::from_utf8(key.clone()).ok()?;
               key_str.strip_prefix("sym:fqn:").map(|s| s.to_string())
           })
           .take(limit)
           .collect();

       Ok(completions)
   }
   ```

3. **Verify no other debug output exists:**
   ```bash
   grep -n "eprintln!" src/backend/native_v2.rs
   grep -n "println!" src/backend/native_v2.rs
   grep -n "DEBUG" src/backend/native_v2.rs
   ```
   - Should return nothing after removal

4. **Verify no debug output in other backend files:**
   ```bash
   grep -n "DEBUG" src/backend/sqlite.rs
   grep -n "eprintln!" src/backend/mod.rs
   ```

**Files to modify:**
- `src/backend/native_v2.rs` (remove lines 521-559, ~40 lines)

**Expected file size change:**
- Before: 679 lines
- After: ~639 lines

**Testing:**
- Integration test: `llmgrep complete --prefix "test" 2>&1 | grep -i "debug"` returns exit code 1 (no matches)
- Integration test: `llmgrep complete --prefix "test" > /dev/null` produces only stdout, no stderr pollution
- Manual test: Run `complete` command and verify clean output
- Verify total line count decreased by ~40 lines

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
