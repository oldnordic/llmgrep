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

| Command | Description |
|---------|-------------|
| `search` | Search symbols, references, calls |
| `ast` | Query raw AST tree for a file |
| `find-ast` | Find AST nodes by kind |

### Search Options

**Search mode:** `--mode {symbols|references|calls|auto}`

**Filters:** `--path`, `--kind`, `--language`, `--regex`, `--fqn`, `--symbol-id`

**Metrics:** `--min-complexity`, `--max-complexity`, `--min-fan-in`, `--min-fan-out`

**AST filtering:** `--ast-kind`, `--with-ast-context`, `--min-depth`, `--max-depth`, `--inside`, `--contains`

**Algorithm filters:** `--condense`, `--paths-from`, `--paths-to`, `--reachable-from`, `--dead-code-in`, `--in-cycle`, `--slice-backward-from`, `--slice-forward-from`

**Sorting:** `--sort-by {relevance|position|fan-in|fan-out|complexity|nesting-depth}`

**Output:** `--output {human|json|pretty}`

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

## Requirements

- **[Magellan](https://github.com/oldnordic/magellan)** 2.1.0+ — Required for code indexing
  ```bash
  cargo install magellan
  magellan watch --root ./src --db .codemcp/codegraph.db
  ```
- **[sqlitegraph](https://crates.io/crates/sqlitegraph)** 1.3.0+ — Included automatically

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
