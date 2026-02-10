# Technology Stack

**Analysis Date:** 2026-02-10

## Languages

**Primary:**
- Rust 2021 Edition - All source code in `src/` and `tests/`

**Secondary:**
- SQL - Embedded queries for SQLite backend
- Shell - Bash scripts in `scripts/` directory
- Markdown - Documentation

## Runtime

**Environment:**
- Rust 2021 Edition
- Binary: `llmgrep` (installed via `cargo install llmgrep`)

**Package Manager:**
- Cargo
- Lockfile: Present (`Cargo.lock`)

**Build Target:**
- Binary crate with library API (`lib.rs`)

## Frameworks

**Core:**
- clap 4.5 - CLI argument parsing with derive macros
- magellan 2.2.1 - Code graph indexing and symbol database
- sqlitegraph 1.5.7 - Graph database foundation (shared with Magellan)

**Storage Backends:**
- rusqlite 0.31 - SQLite backend for legacy databases
- magellan::CodeGraph - Native-V2 backend (requires `--features native-v2`)

**Testing:**
- Built-in Rust test framework
- Integration tests in `tests/` directory

**CLI/Data:**
- regex 1.10 - Pattern matching for symbol search
- serde 1.0 - Serialization/deserialization
- serde_json 1.0 - JSON output format
- chrono 0.4 - Timestamp generation

**Build/Dev:**
- tempfile 3.10 - Temporary file creation in tests

## Key Dependencies

**Critical:**
- magellan 2.2.1 - Core dependency providing CodeGraph, KV store, and graph algorithms. llmgrep is a query client for Magellan databases.
- sqlitegraph 1.5.7 - Shared graph database types (SnapshotId, KvValue, backend)
- rusqlite 0.31 - SQLite access for legacy database backend

**Infrastructure:**
- clap 4.5 - CLI with derive macros for argument parsing
- regex 1.10 - Query pattern matching with ReDoS protection
- serde/serde_json 1.0 - JSON schema-aligned output for LLM consumption
- thiserror 1.0 - Error type definitions with error codes
- anyhow 1.0 - Error handling in AST module
- sha2 0.10 - SHA-256 hashing for code chunk deduplication
- hex 0.4 - Hex encoding for SymbolId display

## Configuration

**Environment:**
- Feature flags for platform selection: `unix` (default), `windows`
- Feature flag for backend: `native-v2` (optional, requires Magellan 2.2+)
- No environment variables required (CLI-driven)

**Build:**
- `.cargo/config.toml` - Sets target-dir
- Feature model: Platform-specific and backend-specific compilation

**Binary Configuration:**
- `--db <PATH>` - Database path (auto-detected: .codemcp/codegraph.db default)
- `--output <FORMAT>` - Output format (human, json, pretty)
- `--show-metrics` - Performance timing display

## Platform Requirements

**Development:**
- Rust 2021 Edition toolchain
- Cargo for dependency management
- magellan installed externally (for database creation)

**Production:**
- Linux/Unix (default), Windows (opt-in via `--features windows`)
- Pre-built Magellan database (created by `magellan watch`)
- No runtime dependencies beyond the database file

**Database Formats Supported:**
- SQLite format (Magellan 1.x, legacy)
- Native-V2 format (Magellan 2.x+, GraphFile format)

---

*Stack analysis: 2026-02-10*
