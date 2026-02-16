# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [3.0.8] - 2026-02-16

### Changed
- **Dependencies:**
  - magellan: 2.4.3 → 2.4.5 (V3 backend persistence fix)
  - sqlitegraph: 2.0.3 → 2.0.5 (V3 backend persistence fix)

### Fixed
- **V3 Backend Persistence:**
  - V3 databases now properly persist and reload across process restarts
  - Previous: databases created by magellan/llmgrep would lose nodes on reopen
  - Now: full V3 persistence working - create, close, reopen, query all work correctly
  - Requires sqlitegraph 2.0.5+ which fixes NodeStore root_page_id sync issue
- **Updated for magellan 2.4.5 API:**
  - Changed `Rc<dyn GraphBackend>` to `Arc<dyn GraphBackend>` in native_v3.rs
  - Matches magellan's thread-safety improvement from 2.4.4

## [3.0.7] - 2026-02-14

### Changed
- **Dependencies:**
  - magellan: 2.4.2 → 2.4.3 (`get_symbol_by_entity_id()` for full symbol details)

### Fixed
- **V3 Backend:** Completed V3 backend implementations
  - `lookup()` - Properly extracts file path from `canonical_fqn`, returns complete `SymbolMatch`
  - `search_by_label()` - Properly extracts file path from `canonical_fqn`, returns complete `SymbolMatch`
  - Both methods now use `get_symbol_by_entity_id()` to fetch full symbol details from entity IDs

### Improved
- **Documentation:** Updated README with accurate Native-V3 feature descriptions
- **Repository:** Removed `.planning/` folder from git tracking (development docs only)

## [3.0.6] - 2026-02-14

### Changed
- **Dependencies:**
  - magellan: 2.4.1 → 2.4.2 (V3 KV operations for llmgrep integration)

### Fixed
- **V3 Backend:** Implemented V3-exclusive methods using new magellan KV APIs
  - `complete()` - now uses `CodeGraph::complete_fqn()`
  - `lookup()` - now uses `CodeGraph::lookup_symbol_by_fqn()`
  - `search_by_label()` - now uses `CodeGraph::get_symbols_by_label_kv()`

## [3.0.5] - 2026-02-14

### Changed
- **Dependencies:**
  - magellan: 2.3.1 → 2.4.1 (full V3 backend feature parity)
  - sqlitegraph: 2.0.1 → 2.0.3 (kv_prefix_scan_v3 support)

### Fixed
- **Test Suite:** Updated test references from native-v2 to native-v3 feature
  - Fixed backend detection tests with proper 16-byte headers
  - Renamed test files to reflect native-v3 naming

## [3.0.2] - 2026-02-14

### Changed

- **Native-V3 Backend Support:**
  - Migrated from native-v2 to native-v3 backend (magellan 2.3.0)
  - V3 uses high-performance binary format with KV store side tables
  - Single `.v3` file contains all data (graph + side tables)
  - Zero SQLite dependency when using V3 backend
  - Feature flag changed from `native-v2` to `native-v3`

- **Dependencies:**
  - magellan: 2.2.1 → 2.3.0
  - sqlitegraph: 1.5.7 → 2.0.1

## [3.0.1] - 2026-02-10

### Fixed

**KV Data Persistence (Critical Fix):**
- Fixed KV index data not being persisted across process restarts
  - Updated magellan to 2.2.1 (includes WAL buffer flush fix)
  - Updated sqlitegraph to 1.5.7 (includes `flush()` method)
  - `complete` and `lookup` commands now work reliably after magellan exits
  - WAL file properly contains KV data (1.4MB with 1000+ entries)

### Changed

- **Dependencies:**
  - magellan: 2.2.0 → 2.2.1
  - sqlitegraph: 1.5.6 → 1.5.7

### Verified

- Cross-process KV communication works (magellan → llmgrep)
- `complete` command returns 1000+ FQN entries
- `lookup` command finds symbols by exact FQN
- WAL persists at 1.4MB after magellan exits

---

## [3.0.0] - 2026-02-10

### Native-V2 Backend Support

**Major release adding dual backend support with Magellan's native-v2 storage.**

**New commands (native-v2 exclusive):**
- `llmgrep complete --db <DB> --prefix <PREFIX>` — FQN autocomplete via KV prefix scan
  - O(1) prefix-based autocomplete using `magellan::kv::kv_prefix_scan()`
  - Supports `--limit <N>` for result limiting
  - Human output (one FQN per line) and JSON formats
- `llmgrep lookup --db <DB> --fqn <FQN>` — O(1) exact symbol lookup
  - Two-phase lookup: KV store for SymbolId, SQL for full metadata
  - Returns `SymbolNotFound` error (LLM-E112) with helpful hints

**New search features:**
- `--mode label` flag — Purpose-based semantic search
  - Search by semantic role: `test`, `entry_point`, `public_api`
  - Uses `magellan::kv::label_key()` for O(1) label lookups
  - Default label: `test` when `--label` not specified

**Performance instrumentation:**
- `--show-metrics` global flag — Display timing breakdown
  - Three-phase timing: backend detection, query execution, output formatting
  - Metrics printed to stderr (human) or included in JSON output
  - Helps identify bottlenecks in query pipelines

**Backend architecture:**
- **Dual backend support:** SQLite (rusqlite) + Native-V2 (Magellan CodeGraph API)
- **Runtime detection:** Automatic backend format detection (no `--backend` flag needed)
- **Graceful fallback:** Native-v2 commands return `RequiresNativeV2Backend` error (LLM-E111) on SQLite databases
- **Feature flag:** Native-v2 support requires `--features native-v2` at compile time

**New error codes:**
- `LLM-E109`: NativeV2BackendNotSupported — native-v2 database detected but llmgrep built without native-v2 feature
- `LLM-E110`: BackendDetectionFailed — unable to determine database format
- `LLM-E111`: RequiresNativeV2Backend — native-v2-only command run on SQLite database
- `LLM-E112`: SymbolNotFound — exact FQN lookup failed with suggestions

**Test coverage:**
- 30 new integration tests for native-v2 exclusive features
- Total: 371 tests passing (zero regressions)

**Dependencies:**
- Updated magellan to 2.2.0+
- Updated sqlitegraph to 1.5.5+
- Added native-v2 feature flag (`--features native-v2`)

**Documentation:**
- Updated README.md with v3.0 features
- Updated CHANGELOG.md with v3.0.0 entry
- All Phase 17-21 planning artifacts in `.planning/phases/`

**Migration from v2.1.x:**
- SQLite backend continues to work without changes
- Native-v2 features are opt-in via feature flag
- No breaking changes to existing commands
- Rebuild with `--features native-v2` to enable native-v2 support

**Compatibility:**
- Magellan 2.2.0+ recommended for native-v2 features
- SQLite databases continue to work with rusqlite backend
- Native-v2 storage provides O(1) KV lookups and smaller file sizes

---

## [2.1.1] - 2026-02-04

### Added
- **Windows Support:** Full cross-platform compatibility via explicit feature flag
  - Use `--features windows` to enable Windows builds
  - Default: `--features unix` (Linux/macOS)
  - Platform detection centralized in `platform.rs` module
  - llmgrep is fully functional on Windows (read-only tool, no background processes)

### Changed
- Feature model: `default = ["unix"]`, `windows` opt-in
- Updated magellan dependency to 2.1.1

**One sentence for the docs:**
> Windows support is opt-in via `--features windows`. Fully supported — llmgrep is a read-only tool.

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
