# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.2.0] - 2026-02-01

### Added

**AST filtering features:**
- `--ast-kind <KIND>` flag to filter search results by AST node kind
  - Shorthand expansion: `loops`, `conditionals`, `functions`, `declarations`, `unsafe`, `types`, `macros`, `mods`, `traits`, `impls`
  - Language-aware expansion for Rust, Python, JavaScript, TypeScript
  - Overlap matching for robust results with real Magellan databases (symbol spans may not exactly match AST node spans)
- `--with-ast-context` flag to include enriched AST context in results
  - `depth`: Nesting depth from AST root
  - `parent_kind`: Kind of parent AST node
  - `children_count_by_kind`: Count of direct children grouped by kind
  - `decision_points`: Number of decision point children
- `--min-depth <N>` flag to filter by minimum nesting depth
- `--max-depth <N>` flag to filter by maximum nesting depth
  - Depth counts decision points only: if/match/loop/for/while expressions
  - Root-level code has depth 0

**New function:**
- `get_ast_context_for_symbol_with_preference()` in `src/ast.rs`
  - Finds AST nodes by symbol overlap, preferring specified kinds
  - Falls back to smallest containing node when no preferred match

### Changed

**AST filtering fixes:**
- Fixed overlap matching formula: `byte_start <= symbol_end AND byte_end >= symbol_start`
- Changed from exact span matching to overlap matching for robustness with real Magellan databases
- Fixed SQL parameter binding by using direct SQL construction instead of `params_from_iter`
- Fixed ORDER BY to prefer smallest containing node when no preferred kinds match
- Test fixtures updated to use correct byte spans matching actual AST nodes

**Documentation:**
- Updated MANUAL.md with complete AST filtering documentation
- Updated README.md with AST filtering examples
- Updated scripts/llmgrep-workflow.sh to v1.2.0 with AST flag support

### Fixed

- AST kind filter now uses overlap matching instead of exact span matching
- `--with-ast-context` flag now works correctly with real Magellan databases
- Depth filtering (`--min-depth`, `--max-depth`) now correctly filters by decision point depth

### Test Coverage

- 277 tests passing
- AST filtering tests: `test_ast_kind_filter`, `test_with_ast_context_flag`, `test_min_depth_filter`, `test_min_max_depth_range`

## [1.1.1] - 2026-01-31

### Bug Fixes

**Critical: Metrics not returned in search results**

- Fixed JOIN condition in `build_symbol_query()` (line 1237)
- Changed from `json_extract(s.data, '$.symbol_id') = sm.symbol_id` to `s.id = sm.symbol_id`
- Root cause: Compared SHA hash string to INTEGER row ID, causing never-matching JOIN
- Impact: Metrics filtering, sorting, and JSON output now work correctly

**Test infrastructure updates**

- Updated all test fixtures to match production Magellan schema
- `symbol_metrics.symbol_id` now INTEGER PRIMARY KEY (was TEXT)
- Added new fields: `estimated_loc`, `last_updated`
- Added FOREIGN KEY constraint to `graph_entities(id)`
- Updated all `insert_metrics()` helper functions

**Test coverage**

- Added regression test `test_metrics_present_in_search_results()`
- Total: 195 tests passing (was 194)

## [1.1.0] - 2026-01-31

### Magellan 1.8.0 Integration

**Added**
- Magellan v1.8.0 dependency for safe UTF-8 content extraction
- Chunk-based snippet retrieval from `code_chunks` table (eliminates file I/O when available)
- Metrics-based filtering: `--min-complexity`, `--max-complexity`, `--min-fan-in`, `--min-fan-out`
- Metrics-based sorting: `--sort-by fan-in|fan-out|complexity`
- SymbolId-based lookups via `--symbol-id` flag (32-char BLAKE3 hash, unambiguous reference)
- FQN filtering: `--fqn` (pattern match with LIKE) and `--exact-fqn` (exact match)
- Ambiguity detection for symbols with multiple matches
- Language filtering via `--language` flag (rust, python, javascript, typescript, c, cpp, java, go, etc.)
- Enhanced `--kind` flag with comma-separated multiple values support
- New JSON fields: `symbol_id`, `canonical_fqn`, `display_fqn`
- New JSON fields: `complexity_score`, `fan_in`, `fan_out`, `cyclomatic_complexity`
- New JSON fields: `content_hash` (SHA-256), `language`, `kind_normalized`
- Integration tests for v1.1 features (10 tests covering UTF-8, metrics, SymbolId, FQN, language filtering)
- Unit tests for v1.1 features (11 tests covering safe extraction and public API)

**Changed**
- Updated `sqlitegraph` dependency to crates.io version 1.2.7
- UTF-8 extraction now uses Magellan's safe functions (no panics on multi-byte boundaries)
- Enhanced help text with v1.1 examples
- MANUAL.md updated with complete v1.1 documentation
- README.md updated with v1.1 feature highlights

**Performance**
- Chunk-based retrieval reduces file I/O for snippet extraction
- Label-based filtering uses indexed queries for faster results
- Metrics sorting uses COALESCE to handle NULL values gracefully

**Fixed**
- SymbolId format validation (32 hex characters)
- FQN pattern matching with LIKE wildcards

**Breaking Changes**
- None (backward compatible with v1.0 databases)

## [1.0.0] - 2026-01-25

### Production-Ready CLI

**Initial release with comprehensive error handling, security hardening, code quality improvements, test coverage, developer experience enhancements, and LLM-optimized performance.**

### Added

- **Error handling framework**
  - LLM-E### error codes with severity and remediation hints
  - Structured JSON responses with error chains
  - Dual-mode output (human-readable or JSON)

- **Security hardening**
  - ReDoS prevention via 10KB regex size limit
  - Resource bounds validation on all parameters
  - Path traversal blocking with canonicalize()

- **Code quality refactoring**
  - SearchOptions struct consolidates 11-16 parameters
  - Shared output formatting module eliminates duplication
  - 90% parameter reduction across search functions

- **Comprehensive test coverage**
  - 118 unit tests with 87.72% coverage
  - Tests for scoring algorithm, SQL query builders, CLI parsing
  - Public API tests for all search functions

- **Developer experience**
  - Clear error messages with actionable remediation
  - Database existence validation before connection
  - File read error logging with context
  - Distinct "not found" vs "corrupted" error messages

- **Performance optimization**
  - `--sort-by {relevance|position}` flag for LLM-optimized or performance modes
  - Position mode skips in-memory scoring for pure SQL ORDER BY
  - Relevance mode uses intelligent ranking (exact > prefix > contains)

- **CLI documentation**
  - 8 usage examples in help text
  - Covers basic search, regex, JSON output, path filtering, all modes
  - Documents `--db`, `--output`, filtering flags, and `--sort-by`

### Changed

- Consolidated 11-16 parameters into single SearchOptions struct
- Replaced three near-identical output functions with shared formatting
- Fixed ReDoS vulnerability in regex compilation
- Added path validation blocking for sensitive directories

### Security

- All regex input validated for size limits (prevents catastrophic backtracking)
- Resource parameters enforce upper bounds at parse time
- Path validation blocks traversal attempts and symlinks to sensitive dirs

### Tested

- 118 unit tests passing
- 87.72% code coverage on src/query.rs
- Zero clippy warnings
- Release build verified

[Unreleased]: Initial llmgrep CLI with Magellan DB search
