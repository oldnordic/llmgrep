# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [2.1.0] - 2026-02-04

### Added

**New commands:**
- `llmgrep ast --db <DB> --file <PATH>` — Query raw AST tree for a file
  - `--position <OFFSET>` flag for node-at-position query
  - `--limit <N>` flag for output limiting
- `llmgrep find-ast --db <DB> --kind <KIND>` — Find AST nodes by kind

**New search flags:**
- `--condense` — Filter to symbols in strongly connected components (SCCs)
- `--paths-from <SYMBOL>` — Filter by execution paths from start symbol
- `--paths-to <SYMBOL>` — Optional end symbol for path filtering

**Algorithm integration:**
- `parse_condense_output()` — Parse magellan condense JSON output
- `parse_paths_output()` — Parse magellan paths JSON output with bounded detection
- `run_ast()` — Shell-out helper for magellan ast command
- `run_find_ast()` — Shell-out helper for magellan find-ast command

**Dependencies:**
- Updated magellan to 2.1.0
- Updated sqlitegraph to 1.3.0

**Error handling:**
- Magellan availability check before algorithm commands (version 2.1.0+ required)
- Clear error messages with installation hints when Magellan not found

**New tests:**
- 18 new tests for ast command (file mode, position mode, limit, validation)
- 2 new tests for find-ast command (basic, various kinds)
- Total: 320 tests passing

**Documentation:**
- Updated README.md with toolset context (Magellan, Mirage, Splice, sqlitegraph)
- Updated MANUAL.md with ast and find-ast commands
- Added v2.1.0 entry to CHANGELOG.md

### Changed

- Documentation now emphasizes llmgrep is part of the sqlitegraph toolset
- All documentation updated to clarify llmgrep only works in conjunction with Magellan

### Compatibility

- Magellan 2.1.0 or later required for algorithm features
- sqlitegraph 1.3.0 or later required

## [1.4.0] - 2026-02-03

### Added

**Magellan 2.0 Algorithm Integration:**
- `--from-symbol-set <FILE>` flag — Load pre-computed SymbolSet from JSON file for filtering
- `--reachable-from <SYMBOL>` flag — One-shot filter: find symbols reachable from specified symbol
- `--dead-code-in <SYMBOL>` flag — One-shot filter: find dead code (unreachable symbols)
- `--in-cycle <SYMBOL>` flag — One-shot filter: find symbols in dependency cycles
- `--slice-backward-from <SYMBOL>` flag — One-shot filter: backward slice (code affecting target)
- `--slice-forward-from <SYMBOL>` flag — One-shot filter: forward slice (code affected by target)

**SymbolSet module (src/algorithm.rs):**
- SymbolSet type with JSON serialization/deserialization
- `run_magellan_algorithm()` — Shell-out helper for Mag CLI algorithms
- `parse_symbol_set_file()` — Load and validate SymbolSet JSON files
- `resolve_fqn_to_symbol_id()` — FQN to SymbolId resolution via `magellan find`
- Temporary table optimization for large SymbolSets (>1000 items)

**SQL filtering extensions:**
- `WHERE symbol_id IN (...)` clause for small SymbolSets
- Temporary table JOIN strategy for large SymbolSets
- `apply_algorithm_filters()` — Orchestrates pre-computed and one-shot filters

**Error codes (LLM-E1xx series):**
- `LLM-E105`: Magellan CLI not found
- `LLM-E106`: Ambiguous symbol name
- `LLM-E107`: Magellan version mismatch
- `LLM-E108`: Magellan algorithm execution failed

**New tests:**
- 33 new algorithm integration tests (561 lines) covering all algorithm types
- Tests for SymbolSet parsing, validation, FQN resolution, and error handling
- Total: 310 tests passing (up from 277)

**Documentation:**
- README.md updated with Magellan Algorithm Integration examples
- MANUAL.md updated with complete algorithm flag reference and composed workflow
- SymbolSet file format documented

**Changed**
- Updated Magellan dependency to support algorithm commands
- Updated Magellan CLI integration for `--output json` format parsing

**Compatibility**
- Magellan 2.0.0 or later recommended for algorithm features
- Backward compatible with v1.3 databases (graceful degradation for missing ast_nodes)
- SymbolSet file format: `{"symbol_ids": ["abc123...", ...]}`

## [1.3.0] - 2026-02-03

### Added

**Structural search features:**
- `--inside <KIND>` flag to find symbols within a parent of specific kind
- `--contains <KIND>` flag to find symbols containing specific children

### Changed

**Documentation updates:**
- Added structural search examples to MANUAL.md
- Added structural search examples to README.md
- Fixed compiler warnings (unused variables prefixed with underscore)

## [1.2.0] - 2026-02-01

### Added

**AST filtering features:**
- `--ast-kind <KIND>` flag to filter search results by AST node kind
- `--with-ast-context` flag to include enriched AST context in results
- `--min-depth <N>` flag to filter by minimum nesting depth
- `--max-depth <N>` flag to filter by maximum nesting depth

## [1.1.1] - 2026-01-31

### Bug Fixes

**Critical: Metrics not returned in search results**
- Fixed JOIN condition in `build_symbol_query()` (line 1237)
- Root cause: Compared SHA hash string to INTEGER row ID

## [1.1.0] - 2026-01-31

### Magellan 1.8.0 Integration

**Added**
- Magellan v1.8.0 dependency for safe UTF-8 content extraction
- Chunk-based snippet retrieval from `code_chunks` table
- Metrics-based filtering and sorting
- SymbolId-based lookups via `--symbol-id` flag
- FQN filtering and ambiguity detection
- Language filtering via `--language` flag

## [1.0.0] - 2026-01-25

### Production-Ready CLI

**Initial release with comprehensive error handling, security hardening, code quality improvements, test coverage, developer experience enhancements, and LLM-optimized performance.**

[Unreleased]: Initial llmgrep CLI with Magellan DB search
