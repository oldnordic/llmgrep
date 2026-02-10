---
phase: 23-feature-parity-native-v2
plan: 01
type: execute
wave: 1

title: "Context Extraction for Native-V2 Backend"

one_liner: "Implement --with-context functionality using FileCache pattern and safe UTF-8 extraction"

author: "Claude (Sonnet)"
completed: "2026-02-10T14:51:48Z"
duration_minutes: 9
tasks_completed: 0
tasks_total: 1

files_modified:
  - path: "src/backend/native_v2.rs"
    provides: "Context extraction from source files for native-v2 backend"
    exports: ["extract_context", "FileCache", "load_file"]
    covered_by: "Task 1 (in progress)"

key_decisions:
  - "Context extraction requires direct file reading via std::fs - CodeGraph API does not expose file contents"
  - "FileCache pattern prevents redundant file reads when extracting context for multiple symbols in same file"
  - "Safe UTF-8 extraction via magellan::common::extract_context_safe handles multi-byte characters (emoji, CJK)"
  - "Implementation follows SQLite backend pattern from src/query.rs for output parity"

dependencies_resolved:
  - "Magellan 2.2.1 provides extract_context_safe() for UTF-8 safe byte extraction"
  - "SpanContext type already exists in src/output.rs for structured context representation"

tech_stack:
  added:
    - "std::collections::HashMap for file caching"
    - "std::fs for file reading"
    - "magellan::common::extract_context_safe for UTF-8 safe extraction"
  patterns:
    - "FileCache: bytes + lines caching for O(1) subsequent access"
    - "load_file: read-or-cache pattern using HashMap::or_insert_with"
    - "extract_context: line-based context with boundary handling"

deviations_from_plan: |
  **BLOCKER: File modification conflicts with automatic rustfmt**

  During implementation, the codebase has an automatic rustfmt/linter that continuously
  reformats src/backend/native_v2.rs while editing is in progress. This caused:
  
  1. Edit conflicts where file is modified between read and write operations
  2. Partial application of changes with reverts
  3. Compilation errors from malformed intermediate states
  
  **Root cause:** CI/CD or editor configuration runs rustfmt automatically on save
  
  **Impact:** Implementation cannot be completed atomically due to race condition with linter
  
  **Resolution required:** Either:
  - Disable automatic rustfmt during development
  - Use a different branch/working tree for changes
  - Apply changes via single comprehensive write operation

verification:
  - "Unit test for extract_context() with file at start (no before lines) - NOT RUN"
  - "Unit test for extract_context() with file at end (no after lines) - NOT RUN"
  - "Integration test: --with-context 2 returns 2 lines before/after - NOT RUN"
  - "UTF-8 test: Context with emoji (4B) and CJK (3B) characters extracted safely - NOT RUN"

success_criteria:
  - "llmgrep --db native-v2.db search --query \"foo\" --with-context returns context lines - NOT VERIFIED"
  - "Context at file boundaries handled gracefully (no panic/missing data) - NOT VERIFIED"
  - "Multi-byte characters in context extracted without replacement characters - NOT VERIFIED"
  - "File cache prevents re-reading same file for multiple symbols - NOT VERIFIED"
  - "All existing tests still pass (no regressions) - NOT VERIFIED"

next_steps:
  - "Resolve linter conflict to apply context extraction changes"
  - "Update search_symbols() method to extract context when options.context.include is true"
  - "Add unit tests for extract_context() with various edge cases"
  - "Add integration test for --with-context flag"
  - "Verify multi-byte character handling in context extraction"
  - "Ensure file cache prevents redundant file reads"
---
