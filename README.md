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

---

## When NOT to Use llmgrep

llmgrep excels at querying indexed code graphs, but it's not the right tool for every task:

| Task | Use This Instead | Why |
|------|----------------|-----|
| Full-text code search | `ripgrep` (`rg`) | Faster, no indexing needed |
| Substring search in files | `grep`/`rg` | Direct file access |
| Live code analysis | Language Server (LSP) | Real-time semantic info |
| Indexing codebases | [Magellan](https://github.com/oldnordic/magellan) | llmgrep is read-only |
| Type information | Compiler/LSP | llmgrep has no type checker |
| CFG analysis | [Mirage](https://github.com/oldnordic/mirage) | Dedicated CFG tool |
| Edit code with spans | [splice](https://github.com/oldnordic/splice) | Precision editing |
| Pattern replacement | `sed`/`llmgrep` + `splice` | llmgrep finds, splice edits |

**Quick Decision Guide:**
- Need **semantic symbol search**? → Use llmgrep
- Need **text/substring search**? → Use ripgrep
- Need **real-time IDE features**? → Use LSP
- Need **to index code**? → Use Magellan

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

## Feature Parity

**As of v3.1**, llmgrep achieves full feature parity between SQLite and Native-V2 backends for all search operations. Both backends now support identical functionality:

### Shared Features (SQLite + Native-V2)

| Feature | Description | Status |
|---------|-------------|--------|
| **Context extraction** | `--with-context` flag | Full parity |
| **Snippet extraction** | `--with-snippet` flag | Full parity |
| **Relevance scoring** | `--sort-by relevance` | Full parity |
| **Metrics filtering** | `--min-fan-in`, `--min-fan-out`, `--min-complexity` | Full parity |
| **Symbol search** | `--mode symbols` | Full parity |
| **Reference search** | `--mode references` | Full parity |
| **Call search** | `--mode calls` | Full parity |
| **AST queries** | `ast`, `find-ast` commands | Full parity |
| **Regex patterns** | `--regex` flag | Full parity |
| **Path filtering** | `--path` flag | Full parity |
| **Kind filtering** | `--kind` flag | Full parity |
| **Language filtering** | `--language` flag | Full parity |

### Native-V2 Exclusive Features

The following features are only available with the Native-V3 backend (requires `--features native-v3` at compile time and `.v3` database file):

| Feature | Description |
|---------|-------------|
| **FQN autocomplete** | `complete` command with O(1) KV prefix scan |
| **Exact lookup** | `lookup` command with O(1) FQN resolution |
| **Label search** | `--mode label` for purpose-based queries (test functions, entry points) |
| **Performance metrics** | `--show-metrics` flag for timing breakdown |

### Backend Selection

llmgrep automatically detects the backend format from the database file header:

```bash
# SQLite backend (default)
magellan watch --root ./src --db code.db

# Native-V2 backend (opt-in)
magellan watch --root ./src --db code.v3

# Build with native-v3 support
cargo install llmgrep --features native-v3
```

Both backends provide identical search results for all shared features.

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

### Native-V3 Features (v3.0.2+)

> **Requires:** `--features native-v3` at compile time and `.v3` database file

```bash
# Build with native-v3 support
cargo install llmgrep --features native-v3

# Index with native-v3 storage (smaller, faster)
magellan watch --root ./src --db code.v3

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
# ERROR LLM-E111: The 'complete' command requires native-v3 backend.
# Reindex with: magellan watch --root . --db code.v3
```

## Requirements

- **[Magellan](https://github.com/oldnordic/magellan)** 2.2.1+ — Required for code indexing
  ```bash
  cargo install magellan
  magellan watch --root ./src --db .codemcp/codegraph.db
  ```
- **[sqlitegraph](https://crates.io/crates/sqlitegraph)** 2.0.1+ — Included automatically
- **Optional:** Native-v3 features require `--features native-v3` at compile time
  ```bash
  cargo install llmgrep --features native-v3
  magellan watch --root ./src --db code.v3
  ```

## Performance Characteristics

llmgrep is designed for fast, LLM-friendly code queries. Here's what to expect:

| Operation | Typical Time | Notes |
|-----------|--------------|-------|
| Simple symbol search | 10-50ms | SQLite: indexed lookup |
| Regex pattern search | 20-100ms | Depends on pattern complexity |
| Reference search | 20-80ms | O(1) graph traversal |
| AST filtering | 50-200ms | Tree structure queries |
| Algorithm filters | 100-500ms | Requires Magellan subprocess |
| Complete/lookup (Native-V2) | 5-20ms | O(1) KV store access |

**Token efficiency** — llmgrep outputs are typically 95-99% smaller than raw source code:

```
Task: "Find all functions in src/lib.rs"
- cat src/lib.rs:          ~15,000 tokens (full file)
- llmgrep search --json:   ~150 tokens (just the facts)
- Savings: 99%
```

## Realistic LLM Workflow

llmgrep is designed for AI assistants to use. Here's how an LLM would work with llmgrep:

```markdown
# User: "Find all functions related to authentication"

# LLM generates:
llmgrep --db .codemcp/codegraph.db search --query "auth" --output json

# Response: 50 tokens of structured data
[
  {"name":"authenticate","kind":"Function","file":"src/auth.rs","line":23},
  {"name":"login_handler","kind":"Function","file":"src/auth.rs","line":45},
  {"name":"is_authenticated","kind":"Function","file":"src/middleware.rs","line":12}
]

# LLM now has precise facts, not 5000 lines of code
```

**Complete refactor workflow:**

```bash
# 1. Discover symbols
llmgrep --db .codemcp/codegraph.db search --query "process" --output json

# 2. Find references
llmgrep --db .codemcp/codegraph.db search --query "process_request" --mode references --output json

# 3. Check impact (via Magellan)
magellan reachable --db .codemcp/codegraph.db --symbol <ID>

# 4. Edit with splice (using byte spans from llmgrep output)
splice rename --symbol <ID> --file src/lib.rs --to handle_request
```

## Documentation

- `llmgrep search --help` — Built-in usage examples
- `MANUAL.md` — Full manual with all options
- `CHANGELOG.md` — Version history
- **Further Documentation:**
  - [Performance Guide](docs/PERFORMANCE.md) — Benchmarks and optimization
  - [Best Practices](docs/BEST_PRACTICES.md) — Recommended workflows
  - [Troubleshooting](docs/TROUBLESHOOTING.md) — Common issues and solutions

## License

GPL-3.0-only. See `LICENSE.md`.

## Related Projects

- [sqlitegraph](https://github.com/oldnordic/sqlitegraph) — Graph database library
- [Magellan](https://github.com/oldnordic/magellan) — Call graph indexer
- [Mirage](https://github.com/oldnordic/mirage) — CFG analysis
- [splice](https://github.com/oldnordic/splice) — Precision editing
