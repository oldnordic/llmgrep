# Codebase Structure

**Analysis Date:** 2026-02-10

## Directory Layout

```
llmgrep/
├── src/                    # Core library and binary source code
│   ├── backend/           # Backend abstraction and implementations
│   └── bin/               # Additional binary utilities
├── tests/                 # Integration and unit tests
├── docs/                  # Documentation files
├── scripts/               # Shell scripts for workflows
├── .planning/             # Planning and phase tracking
│   ├── phases/           # Individual phase documentation
│   ├── milestones/       # Milestone tracking
│   ├── research/         # Research notes
│   └── codebase/         # Codebase analysis documents (this file)
├── .codemcp/             # Magellan database storage
├── test_db/              # Test databases
└── target/               # Cargo build output (generated)
```

## Directory Purposes

**src/:**
- Purpose: Core library and binary source code
- Contains: Library modules, CLI binary, backend implementations
- Key files: `src/lib.rs`, `src/main.rs`, `src/query.rs`, `src/backend/mod.rs`

**src/backend/:**
- Purpose: Storage backend abstraction and implementations
- Contains: Backend trait, SQLite backend, Native-V2 backend
- Key files: `src/backend/mod.rs`, `src/backend/sqlite.rs`, `src/backend/native_v2.rs`

**src/bin/:**
- Purpose: Additional binary utilities
- Contains: Standalone binary programs
- Key files: `src/bin/detect_debug.rs`

**tests/:**
- Purpose: Integration and unit tests
- Contains: Test modules for specific functionality
- Key files: `tests/integration_tests.rs`, `tests/cli_integration_test.rs`, `tests/native_v2_commands_test.rs`

**docs/:**
- Purpose: Project documentation
- Contains: Architecture docs, API references, guides
- Key files: `docs/ARCHITECTURE.md`, `docs/BEST_PRACTICES.md`, `docs/TROUBLESHOOTING.md`

**scripts/:**
- Purpose: Shell scripts for development workflows
- Contains: Utility scripts, workflow automation
- Key files: `scripts/llmgrep-workflow.sh`, `scripts/blast-zone.sh`, `scripts/call-chain.sh`

**.planning/:**
- Purpose: Development planning and tracking
- Contains: Phase documentation, research notes
- Key files: `.planning/phases/` (individual phase directories), `.planning/codebase/` (analysis documents)

**.codemcp/:**
- Purpose: Magellan code graph database storage
- Generated: Yes
- Committed: No (in .gitignore)

**test_db/:**
- Purpose: Test database files for testing
- Generated: Yes
- Committed: No

## Key File Locations

**Entry Points:**
- `src/main.rs`: CLI binary entry point with Clap parsing and command dispatch
- `src/lib.rs`: Library entry point with module declarations and re-exports

**Configuration:**
- `Cargo.toml`: Package manifest with dependencies, features, and metadata
- `.cargo/config.toml`: Cargo configuration (if present)

**Core Logic:**
- `src/query.rs`: Core search implementation with SQL queries and filtering (223KB, largest module)
- `src/backend/mod.rs`: Backend trait and enum for storage abstraction
- `src/algorithm.rs`: Mag CLI integration for graph algorithms
- `src/ast.rs`: AST query utilities and shorthand expansion

**Testing:**
- `tests/integration_tests.rs`: Main integration test suite
- `tests/cli_integration_test.rs`: CLI-specific integration tests
- `tests/native_v2_commands_test.rs`: Native-V2 backend tests
- `tests/backend_parity_test.rs`: Backend parity tests
- `tests/algorithm_tests.rs`: Algorithm integration tests
- `tests/ast_tests.rs`: AST functionality tests

**Output and Errors:**
- `src/output.rs`: Response types (`SymbolMatch`, `SearchResponse`, etc.)
- `src/output_common.rs`: Shared output formatting utilities
- `src/error.rs`: Error types with codes and remediation hints

**Utilities:**
- `src/safe_extraction.rs`: Safe UTF-8 extraction re-exports from magellan
- `src/platform.rs`: Platform support checking

## Naming Conventions

**Files:**
- Modules: `lowercase_snake_case.rs` (e.g., `query.rs`, `algorithm.rs`)
- Tests: `<topic>_tests.rs` or `<topic>_test.rs` (e.g., `search_tests.rs`, `backend_parity_test.rs`)
- Directories: `lowercase_snake_case/` (e.g., `backend/`, `bin/`)

**Directories:**
- Module directories match module names (e.g., `backend/` for `backend` module)
- Test directory is `tests/` (Rust convention)

**Types:**
- Structs: `PascalCase` (e.g., `SearchOptions`, `SymbolMatch`, `Backend`)
- Enums: `PascalCase` (e.g., `SortMode`, `OutputFormat`, `LlmError`)
- Traits: `PascalCase` ending in `Trait` (e.g., `BackendTrait`)

**Functions:**
- Public: `lowercase_snake_case` (e.g., `search_symbols`, `check_ast_table_exists`)
- Internal: `lowercase_snake_case` with leading underscore if needed (e.g., `_format_result`)

**Constants:**
- `SCREAMING_SNAKE_CASE` (e.g., `MAX_REGEX_SIZE`, `SCHEMA_VERSION`)

## Where to Add New Code

**New Feature:**
- Primary code: `src/` new module or existing relevant module
- Tests: `tests/<feature>_tests.rs` or relevant test file
- Example: New search mode - add to `src/query.rs`, test in `tests/search_tests.rs`

**New Component/Module:**
- Implementation: `src/<module_name>.rs`
- Export: Add `pub mod <module_name>;` to `src/lib.rs`
- Tests: Create `tests/<module_name>_tests.rs`

**New Backend:**
- Implementation: `src/backend/<backend_name>.rs`
- Registration: Add variant to `Backend` enum in `src/backend/mod.rs`
- Trait implementation: Implement `BackendTrait` for new backend

**Utilities:**
- Shared helpers: `src/<utility_name>.rs`
- Re-export from dependencies: Add to appropriate module (e.g., safe extraction in `src/safe_extraction.rs`)

**CLI Commands:**
- Command definition: Add variant to `Command` enum in `src/main.rs`
- Handler: Add `run_<command>()` function in `src/main.rs`
- Backend method: Add to `BackendTrait` in `src/backend/mod.rs` if needed

## Special Directories

**target/:**
- Purpose: Cargo build output directory
- Generated: Yes
- Committed: No (in .gitignore)

**.codemcp/:**
- Purpose: Magellan code graph database storage for development
- Generated: Yes
- Committed: No (in .gitignore)

**test_db/:**
- Purpose: Test database files
- Generated: Yes
- Committed: No

**.git/:**
- Purpose: Git repository metadata
- Generated: Yes
- Committed: N/A

**.planning/phases/:**
- Purpose: Individual development phase documentation
- Generated: No
- Committed: Yes
- Naming: `<number>-<phase-name>` or `<number>.<sub-phase>-<phase-name>`

**scripts/:**
- Purpose: Shell scripts for development workflows
- Generated: No
- Committed: Yes
- Executable: Yes (scripts have execute permissions)

---

*Structure analysis: 2026-02-10*
