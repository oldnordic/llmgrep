---
phase: 23-feature-parity-native-v2
plan: 01
type: execute
wave: 1

title: "Context Extraction for Native-V2 Backend"

one_liner: "Implement --with-context functionality using FileCache pattern and safe UTF-8 extraction"

author: "Claude (Sonnet + Opus)"
completed: "2026-02-10T15:30:00Z"
duration_minutes: 40
tasks_completed: 1
tasks_total: 1

files_modified:
  - path: "src/backend/native_v2.rs"
    provides: "Context extraction from source files for native-v2 backend"
    exports: ["span_context_from_file", "FileCache", "load_file"]
    covered_by: "Task 1"

key_decisions:
  - "Context extraction requires direct file reading via std::fs - CodeGraph API does not expose file contents"
  - "FileCache pattern prevents redundant file reads when extracting context for multiple symbols in same file"
  - "FileCache includes both bytes (for snippet) and lines (for context) for unified caching"
  - "Implementation follows SQLite backend pattern from src/query.rs for output parity"

dependencies_resolved:
  - "Magellan 2.2.1 provides extract_symbol_content_safe() for UTF-8 safe byte extraction"
  - "SpanContext type already exists in src/output.rs for structured context representation"

tech_stack:
  added:
    - "std::collections::HashMap for file caching"
    - "std::fs for file reading"
    - "crate::output::SpanContext for structured context representation"
  patterns:
    - "FileCache: bytes + lines caching for O(1) subsequent access"
    - "load_file: read-or-cache pattern using HashMap::or_insert_with"
    - "span_context_from_file: line-based context with boundary handling"

deviations_from_plan: |
  The initial agent reported a linter conflict that blocked completion. The orchestrator
  manually completed the implementation by:
  1. Adding SpanContext to imports
  2. Updating FileCache to include lines vector
  3. Adding span_context_from_file() function
  4. Updating symbol_node_to_match() signature to include context parameter
  5. Updating search_symbols() to extract context when options.context.include is true
  6. Fixing all symbol_node_to_match() calls to include None for context parameter

verification:
  - "All 151 tests pass with native-v2 feature - PASSED"
  - "Code compiles without errors - PASSED"
  - "FileCache struct includes both bytes and lines - PASSED"
  - "span_context_from_file() handles file boundaries - PASSED (via code inspection)"

success_criteria:
  - "Context extraction implemented in search_symbols() - COMPLETE"
  - "FileCache updated with lines vector - COMPLETE"
  - "span_context_from_file() function added - COMPLETE"
  - "All existing tests still pass (no regressions) - PASSED"
  - "Code compiles with native-v2 feature - PASSED"

next_steps: None (plan complete)
---
