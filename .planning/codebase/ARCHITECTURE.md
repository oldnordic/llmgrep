# Architecture

**Analysis Date:** 2026-02-10

## Pattern Overview

**Overall:** Backend Abstraction with Dual Storage Strategy

**Key Characteristics:**
- Trait-based backend abstraction for runtime storage detection
- Dual backend support: SQLite (traditional) and Native-V2 (high-performance)
- Shell-out integration with Magellan CLI for graph algorithms
- Streaming query pipeline with candidate filtering
- AST-first structural search capabilities

## Layers

**CLI Layer:**
- Purpose: Command-line interface parsing, validation, and user interaction
- Location: `src/main.rs`
- Contains: Clap-derived argument parsing, output formatting, platform checks
- Depends on: Backend layer, query layer, output layer
- Used by: End users via terminal

**Backend Abstraction Layer:**
- Purpose: Runtime backend detection and unified query interface
- Location: `src/backend/mod.rs`
- Contains: `Backend` enum, `BackendTrait`, backend-specific implementations
- Depends on: `magellan` crate, `sqlitegraph` crate, query layer
- Used by: CLI layer, query layer

**Backend Implementations:**
- Purpose: Concrete storage backends for different database formats
- Location: `src/backend/sqlite.rs`, `src/backend/native_v2.rs`
- Contains: `SqliteBackend` (rusqlite-based), `NativeV2Backend` (CodeGraph-based)
- Depends on: `rusqlite`, `magellan::CodeGraph`, `sqlitegraph`
- Used by: Backend abstraction layer

**Query Layer:**
- Purpose: Core search logic with filtering, scoring, and result assembly
- Location: `src/query.rs`
- Contains: `SearchOptions`, SQL queries, regex matching, context extraction
- Depends on: `rusqlite`, `regex`, algorithm layer, AST layer
- Used by: Backend implementations

**Algorithm Layer:**
- Purpose: Integration with Magellan's graph algorithms via shell-out
- Location: `src/algorithm.rs`
- Contains: `SymbolSet`, algorithm execution wrappers, version checking
- Depends on: `magellan` CLI, `serde_json`
- Used by: Query layer for graph-based filtering

**AST Layer:**
- Purpose: Abstract Syntax Tree queries and structural filtering
- Location: `src/ast.rs`
- Contains: `AstContext`, shorthand expansion, depth calculation
- Depends on: `rusqlite`, `anyhow`
- Used by: Query layer for structural filters

**Output Layer:**
- Purpose: Response type definitions and JSON serialization
- Location: `src/output.rs`, `src/output_common.rs`
- Contains: `SymbolMatch`, `SearchResponse`, `PerformanceMetrics`
- Depends on: `serde`, `chrono`
- Used by: CLI layer, query layer

**Error Handling Layer:**
- Purpose: Centralized error types with error codes and remediation hints
- Location: `src/error.rs`
- Contains: `LlmError` enum with categorized error codes
- Depends on: `thiserror`
- Used by: All layers

**Utility Layer:**
- Purpose: Safe UTF-8 extraction for multi-byte character handling
- Location: `src/safe_extraction.rs`
- Contains: Re-exports from `magellan::common`
- Depends on: `magellan`
- Used by: Query layer

## Data Flow

**Symbol Search Flow:**

1. User invokes `llmgrep --db code.db search --query "parse"`
2. `main.rs` parses CLI arguments via Clap
3. `Backend::detect_and_open()` detects SQLite vs Native-V2 format
4. `SearchOptions` constructed from CLI arguments
5. Backend trait method `search_symbols()` invoked
6. Query layer executes SQL with candidate filtering
7. Algorithm filters applied (reachable, dead-code, cycles, condense, paths)
8. AST filters applied if `--ast-kind` specified
9. Results sorted by selected mode (relevance, position, fan-in, complexity)
10. Response formatted (human, json, or pretty) and emitted

**Reference Search Flow:**

1. User invokes with `--mode references`
2. Query layer joins `references` table with `symbols` table
3. Scoring applied based on name match quality
4. Context/snippet extraction if requested
5. Results returned with `referenced_symbol` field

**Call Search Flow:**

1. User invokes with `--mode calls`
2. Query layer queries `references` table for outgoing edges
3. Joins with `symbols` table for caller/callee names
4. Results formatted with `caller -> callee` display

**AST Query Flow:**

1. User invokes `llmgrep --db code.db ast --file src/main.rs`
2. Backend delegates to shell-out `magellan ast` (SQLite) or direct query (Native-V2)
3. JSON response parsed and optionally truncated by `--limit`
4. Returned as JSON or pretty-printed

**State Management:**
- Stateless CLI application - each invocation is independent
- Database connections opened per invocation
- No in-memory caching between invocations
- Backend detection occurs on every `--db` open

## Key Abstractions

**Backend Trait:**
- Purpose: Unified interface for SQLite and Native-V2 storage
- Examples: `src/backend/mod.rs` (BackendTrait, Backend enum)
- Pattern: Strategy pattern with runtime dispatch

**SearchOptions:**
- Purpose: Configuration bundle for all query parameters
- Examples: `src/query.rs` (SearchOptions, ContextOptions, SnippetOptions, FqnOptions, MetricsOptions, AstOptions, DepthOptions, AlgorithmOptions)
- Pattern: Builder pattern via nested option structs

**SymbolMatch:**
- Purpose: Unified symbol representation across backends
- Examples: `src/output.rs` (SymbolMatch, ReferenceMatch, CallMatch)
- Pattern: Data transfer object with optional fields for conditional serialization

**SymbolSet:**
- Purpose: Collection of BLAKE3 SymbolId hashes for algorithm filtering
- Examples: `src/algorithm.rs` (SymbolSet, SymbolSetStrategy)
- Pattern: Value object with JSON serialization

**AstContext:**
- Purpose: Structural metadata from AST nodes table
- Examples: `src/ast.rs` (AstContext, AST_SHORTHANDS)
- Pattern: Enriched context with lazy computation

## Entry Points

**Binary Entry Point:**
- Location: `src/main.rs` (function `main`)
- Triggers: CLI invocation
- Responsibilities:
  - Platform support checks
  - Argument parsing via Clap
  - Backend detection and opening
  - Command dispatch to search/ast/complete/lookup
  - Output formatting and error emission

**Library Entry Points:**
- Location: `src/lib.rs`
- Triggers: Use as dependency by other Rust crates
- Responsibilities:
  - Module declarations and re-exports
  - Public API surface (algorithm, AST, backend, error, output, query, safe_extraction)
  - `SortMode` enum for result ordering

## Error Handling

**Strategy:** Enum-based error categorization with error codes and remediation hints

**Patterns:**
- `thiserror` derive for automatic error display/conversion
- Error codes in format `LLM-EXXX` for programmatic handling
- Remediation hints via `remediation()` method for user guidance
- Automatic conversions from `rusqlite::Error`, `regex::Error`, `serde_json::Error`, `std::io::Error`
- JSON error responses for machine-readable output
- Structured errors with context (path, query, reason fields)

## Cross-Cutting Concerns

**Logging:** Metrics tracking via `PerformanceMetrics` struct with millisecond timing for backend_detection, query_execution, output_formatting, total

**Validation:** Path validation blocking sensitive directories (`/etc`, `/root`, `/boot`, `sys`, `/proc`, `/dev`, `/run`, `/var/run`, `/var/tmp`, `~/.ssh`, `~/.config`)

**Authentication:** Not applicable (local file-based databases only)

**Security:** ReDoS prevention via regex size limits (10KB), sensitive directory blocking, SymbolId format validation (32 hex characters)

**Platform:** Feature-gated platform support (default unix, optional windows) via `src/platform.rs`

**Feature Flags:**
- `native-v2` - Optional Native-V2 backend support (disabled by default)
- `unix` / `windows` - Platform-specific code

---

*Architecture analysis: 2026-02-10*
