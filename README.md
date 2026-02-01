# llmgrep

**v1.2.0** (in development)

A command-line search tool for code indexed by [Magellan](https://github.com/oldnordic/magellan). Queries SQLite graph databases to find symbols, references, and call relationships. Outputs human-readable or structured JSON for programmatic use.

## Works with Magellan

llmgrep requires a Magellan database. It does not build or modify databases — Magellan handles indexing and freshness.

**Install Magellan:**
```bash
cargo install magellan
# or: git clone https://github.com/oldnordic/magellan && cd magellan && cargo build --release
```

**Build a database:**
```bash
magellan watch --root /path/to/repo --db /path/to/repo.db
```

**Crates:**
- [magellan](https://crates.io/crates/magellan) v1.8.0 - Code indexing and database creation
- [sqlitegraph](https://crates.io/crates/sqlitegraph) v1.2.7 - Graph database persistence
- [llmgrep](https://crates.io/crates/llmgrep) - This tool (query only)

## What it does

- Searches symbols, references, and calls from Magellan's code database
- Emits deterministic, schema-aligned JSON output for LLM consumption
- Supports regex, intelligent relevance ranking, and optional context/snippets
- `--sort-by {relevance|position|fan-in|fan-out|complexity}` flag for LLM-optimized or performance modes

## Features

- **Search modes**: symbols (definitions), references, calls, auto (all modes)
- **Regex support**: Pattern matching with `--regex`
- **Intelligent ranking**: Relevance mode (exact > prefix > contains scoring)
- **Fast position mode**: SQL-only sorting, skips in-memory scoring
- **Metrics-based**: Sort by fan-in, fan-out, or cyclomatic complexity
- **JSON output**: Schema-aligned for LLM consumption
- **Context/snippets**: Optional code context and snippets in results
- **FQN filtering**: Filter by fully-qualified name patterns
- **Language filtering**: Filter by programming language (rust, python, javascript, typescript, c, cpp, java, go)
- **Kind filtering**: Filter by symbol kind (Function, Struct, Method, Class, Enum, Module, etc.)
- **Security**: ReDoS prevention, resource bounds, path traversal blocking
- **Error codes**: LLM-E### format with remediation hints
- **Depth filtering**: Find symbols by AST nesting depth (--min-depth, --max-depth)
- **Structural search**: Find symbols by parent/child relationship (--inside, --contains)

## v1.2.0 - Depth and Structural Search (In Development)

**AST-aware search features:**
- **Decision depth filtering**: `--min-depth` and `--max-depth` to filter by AST nesting depth
  - Depth counts only decision points: if/match/loop/for/while expressions
  - Use `--with-ast-context` to see depth values in results
- **Structural search**: `--inside KIND` and `--contains KIND` for parent/child relationships
  - `--inside function_item`: find symbols inside any function
  - `--inside block`: find symbols inside blocks
  - `--contains if_expression`: find functions/structures containing if expressions
  - `--contains call_expression`: find functions with calls
- **AST context**: `--with-ast-context` flag adds `ast_context` to results (depth, kind, parent_id, byte_range)

## v1.1.1 - Bugfix Release

**Critical fix:** Metrics JOIN condition - metrics (fan_in, fan_out, cyclomatic_complexity) are now correctly returned in search results. Previous versions compared SHA hash strings to integer row IDs, causing never-matching JOINs.

## v1.1.0 - Magellan Integration

**Magellan 1.8.0 Integration:**
- Safe UTF-8 content extraction (no panics on emoji/CJK/accented characters)
- Chunk-based snippet retrieval (eliminates file I/O when chunks available)
- Metrics-based filtering: `--min-complexity`, `--max-complexity`, `--min-fan-in`, `--min-fan-out`
- Metrics-based sorting: `--sort-by fan-in|fan-out|complexity`
- SymbolId lookups via `--symbol-id` for unambiguous reference
- FQN filtering: `--fqn` (pattern) and `--exact-fqn` (exact match)
- Language filtering via `--language` flag
- Enhanced JSON fields: `symbol_id`, `canonical_fqn`, `display_fqn`, metrics, `content_hash`

## Install

```
cargo build --release
cp target/release/llmgrep ~/.local/bin/llmgrep
```

Or install from crates.io:

```
cargo install llmgrep
```

## Quick start

Query your Magellan database with llmgrep:

```bash
# Basic symbol search (relevance-ranked, LLM-friendly)
llmgrep --db /tmp/repo.db search --query "parse"

# Position-only sorting (faster, no scoring)
llmgrep --db /tmp/repo.db search --query "parse" --sort-by position

# Regex search with JSON output
llmgrep --db /tmp/repo.db search --query "^Token" --regex --output json

# Reference search
llmgrep --db /tmp/repo.db search --query "MyType" --mode references
```

## v1.1 Examples

### SymbolId lookup (unambiguous)
```bash
llmgrep --db code.db search --symbol-id abc123def456789abc123def456789ab
```

### Filter by complexity
```bash
llmgrep --db code.db search --query "parse" --min-complexity 10
```

### Filter by language and kind
```bash
llmgrep --db code.db search --query "Test" --language rust --kind Function
```

### Sort by fan-in (find hotspots)
```bash
llmgrep --db code.db search --query ".*" --sort-by fan-in --limit 20
```

### FQN pattern matching
```bash
llmgrep --db code.db search --query "helper" --fqn "%module::%"
```

## v1.2.0 Examples

### Find deeply nested code (code smell detection)
```bash
# Find closures nested 3+ levels deep (potential complexity)
llmgrep --db code.db search --query "closure" --min-depth 3 --with-ast-context

# Find functions at decision depth 0-1 (simple, testable)
llmgrep --db code.db search --query ".*" --kind Function --max-depth 1
```

### Find symbols inside specific structures
```bash
# Find all closures inside functions
llmgrep --db code.db search --query "closure" --inside function_item

# Find let declarations inside blocks
llmgrep --db code.db search --query "let" --ast-kind let_declaration --inside block

# Find call expressions inside if expressions (nested logic)
llmgrep --db code.db search --query "call" --ast-kind call_expression --inside if_expression
```

### Find parents containing specific children
```bash
# Find functions containing if expressions (conditional logic)
llmgrep --db code.db search --query ".*" --contains if_expression --kind Function

# Find functions with multiple calls (potential refactoring candidates)
llmgrep --db code.db search --query ".*" --contains call_expression --sort-by fan-out

# Find functions containing match expressions (pattern matching)
llmgrep --db code.db search --query ".*" --contains match_expression
```

### Combined depth and structural filters
```bash
# Find closures at depth >= 1 inside functions
llmgrep --db code.db search --query "closure" --min-depth 1 --inside function_item

# Find deeply nested if expressions inside loops
llmgrep --db code.db search --query "if" --min-depth 2 --inside loop_expression
```

### View AST context in results
```bash
# Search with AST context to see depth and kind
llmgrep --db code.db search --query "process" --with-ast-context
```

## Compatibility

- **Magellan**: v1.8.0 or later (for metrics/chunks support)
- **sqlitegraph**: v1.2.7 (via crates.io)
- **Database schema**: Magellan 1.x (graph_entities, graph_edges, symbol_metrics, code_chunks)
- **Languages**: Rust, C, C++, Java, JavaScript, TypeScript, Python (via Magellan parsers)

## Documentation

- `llmgrep search --help` — Built-in usage examples
- `MANUAL.md` — Full manual with all options
- `CHANGELOG.md` — Version history
- [Magellan repo](https://github.com/oldnordic/magellan) — Code indexing tool

## License

GPL-3.0-only. See `LICENSE.md`.
