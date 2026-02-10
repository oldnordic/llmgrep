# llmgrep

**Semantic Code Search for Magellan Databases**

[![Crates.io](https://img.shields.io/crates/v/llmgrep)](https://crates.io/crates/llmgrep)
[![Documentation](https://docs.rs/llmgrep/badge.svg)](https://docs.rs/llmgrep)

> Fast, reliable search of code indexed by Magellan with LLM-optimized output.

## ⚠️ Requires Magellan

**llmgrep requires [Magellan](https://github.com/oldnordic/magellan) to function.**

Magellan provides the code graph database that llmgrep queries. You must run `magellan watch` on your codebase before using llmgrep.

```bash
# Install Magellan first
cargo install magellan

# Watch your project (builds database)
magellan watch --root ./src --db .codemcp/codegraph.db

# Now llmgrep can search
llmgrep --db .codemcp/codegraph.db search --query "parse"
```

## The Code Intelligence Toolset

llmgrep is part of a coordinated toolset built on [sqlitegraph](https://github.com/oldnordic/sqlitegraph). All tools share a common SQLite graph database and are designed to work together for AI-assisted code understanding.

```
┌─────────────┐      ┌─────────────┐      ┌─────────────┐
│  Magellan   │ ───► │  llmgrep    │ ───► │   Mirage    │
│(Symbols &   │      │ (Semantic   │      │(CFG & Paths)│
│  Call Graph)│      │  Search)    │      │             │
└─────────────┘      └─────────────┘      └─────────────┘
       │                    │                     │
       └────────────────────┴─────────────────────┘
                     │
              ┌──────▼──────┐
              │ sqlitegraph │
              │  (Database) │
              └─────────────┘
                     │
              ┌──────▼──────┐
              │   splice    │
              │(Edit using  │
              │   spans)    │
              └─────────────┘
```

| Tool | Purpose | Repository | Install |
|------|---------|------------|---------|
| **sqlitegraph** | Graph database foundation | [github.com/oldnordic/sqlitegraph](https://github.com/oldnordic/sqlitegraph) | `cargo add sqlitegraph` |
| **Magellan** | Call graph indexing, symbol navigation | [github.com/oldnordic/magellan](https://github.com/oldnordic/magellan) | `cargo install magellan` |
| **llmgrep** | Semantic code search | [github.com/oldnordic/llmgrep](https://github.com/oldnordic/llmgrep) | `cargo install llmgrep` |
| **Mirage** | CFG analysis, path enumeration | [github.com/oldnordic/mirage](https://github.com/oldnordic/mirage) | `cargo install mirage-analyzer` |
| **splice** | Precision code editing | [github.com/oldnordic/splice](https://github.com/oldnordic/splice) | `cargo install splice` |

## What is llmgrep?

llmgrep queries Magellan's code graph database to find symbols, references, and call relationships. Emits deterministic, schema-aligned JSON for LLM consumption.

### What llmgrep is NOT

- ❌ A code indexer (use [Magellan](https://github.com/oldnordic/magellan))
- ❌ An embedding or semantic search tool
- ❌ A CFG analysis tool (use [Mirage](https://github.com/oldnordic/mirage))
- ❌ A code editing tool (use [splice](https://github.com/oldnordic/splice))

### What llmgrep IS

- ✅ Read-only query tool for Magellan databases
- ✅ Symbol search with regex and intelligent ranking
- ✅ Reference and call graph traversal
- ✅ AST-aware structural search
- ✅ Magellan algorithm integration (condense, paths, cycles, reachable, dead-code, slice)
- ✅ Raw AST tree queries
- ✅ LLM-optimized JSON output

## Installation

```bash
cargo install llmgrep
```

Or build from source:

```bash
git clone https://github.com/oldnordic/llmgrep
cd llmgrep
cargo install --path .
```

## Platform Support

llmgrep uses a shared platform feature model across the SQLiteGraph toolset:

| Platform | Feature | Status |
|----------|---------|--------|
| **Linux** | `unix` (default) | ✅ Full support |
| **macOS** | `unix` (default) | ✅ Full support |
| **Windows** | `windows` (opt-in) | ✅ Full support |

### Building for Windows

Windows support is opt-in via feature flag:

```bash
# Build for Windows
cargo build --release --features windows

# Install from source on Windows
cargo install llmgrep --features windows
```

**llmgrep is fully functional on Windows.**

llmgrep is a read-only query tool with no background processes, file watching, or signal handling. All features work identically across platforms.

### Feature Model

```toml
[features]
default = ["unix"]
unix = []
windows = []
```

## Quick Start

### 1. Install the Toolset

```bash
# Install all tools for complete workflow
cargo install magellan        # Call graph & CFG extraction (REQUIRED)
cargo install llmgrep         # Semantic search
cargo install mirage-analyzer # Path-aware analysis
cargo install splice          # Precision editing
```

### 2. Index Your Project

```bash
# Magellan watches your source and builds database
magellan watch --root ./src --db .codemcp/codegraph.db
```

### 3. Search with llmgrep

```bash
# Basic symbol search
llmgrep --db .codemcp/codegraph.db search --query "parse"

# Reference search
llmgrep --db .codemcp/codegraph.db search --query "MyType" --mode references

# Regex with JSON output
llmgrep --db .codemcp/codegraph.db search --query "^Token" --regex --output json
```

## Commands

| Command | Description | Backend |
|---------|-------------|---------|
| `search` | Search symbols, references, calls | SQLite + Native-V2 |
| `ast` | Query raw AST tree for a file | SQLite + Native-V2 |
| `find-ast` | Find AST nodes by kind | SQLite + Native-V2 |
| `complete` | FQN autocomplete via KV prefix scan (v3.0) | Native-V2 only |
| `lookup` | O(1) exact symbol lookup by FQN (v3.0) | Native-V2 only |

### Search Options

**Search mode:** `--mode {symbols|references|calls|label}` (v3.0 adds `label`)

**Filters:** `--path`, `--kind`, `--language`, `--regex`, `--fqn`, `--symbol-id`, `--label` (v3.0)

**Metrics:** `--min-complexity`, `--max-complexity`, `--min-fan-in`, `--min-fan-out`

**AST filtering:** `--ast-kind`, `--with-ast-context`, `--min-depth`, `--max-depth`, `--inside`, `--contains`

**Algorithm filters:** `--condense`, `--paths-from`, `--paths-to`, `--reachable-from`, `--dead-code-in`, `--in-cycle`, `--slice-backward-from`, `--slice-forward-from`

**Sorting:** `--sort-by {relevance|position|fan-in|fan-out|complexity|nesting-depth}`

**Output:** `--output {human|json|pretty}`

**Performance:** `--show-metrics` (v3.0) — Display timing breakdown for queries

## Examples

### Basic Search

```bash
# Relevance-ranked (LLM-friendly)
llmgrep --db code.db search --query "parse"

# Position-sorted (faster)
llmgrep --db code.db search --query "parse" --sort-by position

# Sort by complexity
llmgrep --db code.db search --query ".*" --sort-by complexity --limit 20
```

### AST Filtering

```bash
# Find all loops
llmgrep --db code.db search --query ".*" --ast-kind loops

# Find deeply nested code
llmgrep --db code.db search --query ".*" --min-depth 3 --with-ast-context

# Find closures within functions
llmgrep --db code.db search --query ".*" --inside function_item --ast-kind closure_expression
```

### Magellan Algorithm Integration

```bash
# Find symbols in strongly connected components
llmgrep --db code.db search --condense --query ".*"

# Find symbols on execution paths from main
llmgrep --db code.db search --paths-from main --query ".*"

# Find symbols on paths between two symbols
llmgrep --db code.db search --paths-from parse --paths-to execute

# Find dead code (unreachable from main)
llmgrep --db code.db search --dead-code-in main --query ".*"

# Find symbols in dependency cycles
llmgrep --db code.db search --in-cycle process --query ".*"

# Backward slice: code affecting target
llmgrep --db code.db search --slice-backward-from handle_error --query ".*"

# Forward slice: code affected by source
llmgrep --db code.db search --slice-forward-from load_config --query ".*"
```

### AST Commands (v2.1)

```bash
# Get full AST tree for a file
llmgrep --db code.db ast --file src/main.rs

# Get AST node at specific position
llmgrep --db code.db ast --file src/main.rs --position 100

# Limit output for large files
llmgrep --db code.db ast --file src/main.rs --limit 50

# Find all function definitions
llmgrep --db code.db find-ast --kind function_item

# Find all loops
llmgrep --db code.db find-ast --kind for_expression
```

### Native-V2 Features (v3.0)

> **Requires:** `--features native-v2` at compile time and native-v2 database format

```bash
# Build with native-v2 support
cargo install llmgrep --features native-v2

# Index with native-v2 storage (smaller, faster)
magellan watch --root ./src --db code.db --storage native-v2

# FQN autocomplete
llmgrep --db code.db complete --prefix "my_crate::"
# Returns: my_crate::module::function_name
#         my_crate::module::AnotherStruct

# O(1) exact symbol lookup
llmgrep --db code.db lookup --fqn "my_crate::module::function_name"
# Returns full symbol details in JSON

# Purpose-based semantic search
llmgrep --db code.db search --mode label --label test
# Returns all test functions

llmgrep --db code.db search --mode label --label entry_point
# Returns: main(), lib exports, public API

# Performance metrics
llmgrep --db code.db search --query "main" --show-metrics
# Output includes timing breakdown:
#   Backend detection: 5ms
#   Query execution: 23ms
#   Output formatting: 2ms
#   Total: 30ms
```

**Error handling on SQLite databases:**

Native-v2 exclusive commands gracefully fallback with helpful errors on SQLite databases:

```bash
# On SQLite database, returns LLM-E111
llmgrep --db sqlite.db complete --prefix "test"
# ERROR LLM-E111: The 'complete' command requires native-v2 backend.
# Reindex with: magellan watch --root . --db code.db --storage native-v2
```

## Requirements

- **[Magellan](https://github.com/oldnordic/magellan)** 2.2.0+ — Required for code indexing
  ```bash
  cargo install magellan
  magellan watch --root ./src --db .codemcp/codegraph.db
  ```
- **[sqlitegraph](https://crates.io/crates/sqlitegraph)** 1.5.5+ — Included automatically
- **Optional:** Native-v2 features require `--features native-v2` at compile time
  ```bash
  cargo install llmgrep --features native-v2
  magellan watch --root ./src --db code.db --storage native-v2
  ```

## Documentation

- `llmgrep search --help` — Built-in usage examples
- `MANUAL.md` — Full manual with all options
- `CHANGELOG.md` — Version history

## License

GPL-3.0-only. See `LICENSE.md`.

## Related Projects

- [sqlitegraph](https://github.com/oldnordic/sqlitegraph) — Graph database library
- [Magellan](https://github.com/oldnordic/magellan) — Call graph indexer
- [Mirage](https://github.com/oldnordic/mirage) — CFG analysis
- [splice](https://github.com/oldnordic/splice) — Precision editing
