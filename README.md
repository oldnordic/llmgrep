# llmgrep

**v1.3.0** (shipped 2026-02-03)

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
- **AST filtering**: Filter by AST node kind with overlap matching (--ast-kind)
- **AST context**: Include enriched AST context with depth and parent info (--with-ast-context)
- **Depth filtering**: Find symbols by AST nesting depth (--min-depth, --max-depth)
- **Structural search**: Find symbols by parent/child relationships (--inside, --contains)
- **Magellan algorithm integration**: Filter by Magellan graph algorithm results (--from-symbol-set, --reachable-from, --dead-code-in, --in-cycle, --slice-backward-from, --slice-forward-from)

## v1.3.0 - AST Structural Search (2026-02-03)

**AST-aware search features:**
- **AST node kind filtering**: `--ast-kind` flag to filter by AST node type
  - Supports shorthands: `loops`, `conditionals`, `functions`, `declarations`, `unsafe`, `types`, `macros`, `mods`, `traits`, `impls`
  - Language-aware expansion: shorthands expand to language-specific node kinds
  - Uses overlap matching for robust results with real Magellan databases
- **AST context**: `--with-ast-context` flag adds `ast_context` to results
  - `depth`: Nesting depth from AST root
  - `parent_kind`: Kind of parent AST node
  - `children_count_by_kind`: Count of direct children grouped by kind
  - `decision_points`: Number of decision point children
- **Decision depth filtering**: `--min-depth` and `--max-depth` flags
  - Depth counts only decision points: if/match/loop/for/while expressions
  - Root-level code has depth 0

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

## v1.3.0 Examples

### AST kind filtering
```bash
# Find all loops using shorthand
llmgrep --db code.db search --query ".*" --ast-kind loops

# Find conditionals in Python
llmgrep --db code.db search --query ".*" --ast-kind conditionals --language python

# Find specific node kind
llmgrep --db code.db search --query "parse" --ast-kind call_expression
```

### Find deeply nested code (code smell detection)
```bash
# Find code nested 3+ levels deep
llmgrep --db code.db search --query ".*" --min-depth 3 --with-ast-context

# Find functions at decision depth 0-1 (simple, testable)
llmgrep --db code.db search --query ".*" --kind Function --max-depth 1
```

### View AST context in results
```bash
# Search with AST context to see depth and kind
llmgrep --db code.db search --query "process" --with-ast-context
```

### Structural search (find by parent/child relationships)
```bash
# Find closures within functions
llmgrep --db code.db search --query ".*" --inside function_item --ast-kind closure_expression

# Find functions containing async calls
llmgrep --db code.db search --query ".*" --contains await_expression --ast-kind function_item

# Find functions with loops
llmgrep --db code.db search --query ".*" --contains for_expression --ast-kind function_item

# Combine: find closures inside functions that contain loops
llmgrep --db code.db search --query ".*" --inside function_item --contains for_expression --ast-kind closure_expression
```

## AST Filtering (v1.3)

Filter search results by AST node kind for structural code queries.

### Shorthand Groups

Common patterns have memorable shorthands:

```bash
# Find all loops (for, while, loop)
llmgrep --db code.db search --query ".*" --ast-kind loops

# Find conditionals (if, match)
llmgrep --db code.db search --query ".*" --ast-kind conditionals

# Find functions and closures
llmgrep --db code.db search --query "process" --ast-kind functions

# Find all declarations (struct, enum, let, const, static)
llmgrep --db code.db search --query "Config" --ast-kind declarations

# Find unsafe blocks
llmgrep --db code.db search --query ".*" --ast-kind unsafe
```

### Multi-Language Support

Node kinds work across indexed languages:

```bash
# Python functions
llmgrep --db code.db search --query "test" --ast-kind functions --language python

# TypeScript classes
llmgrep --db code.db search --query "Service" --ast-kind declarations --language typescript

# JavaScript loops
llmgrep --db code.db search --query ".*" --ast-kind loops --language javascript
```

### Combine Shorthands and Specific Kinds

```bash
# Combine multiple shorthands
llmgrep --db code.db search --query ".*" --ast-kind loops,conditionals

# Mix shorthands with specific kinds
llmgrep --db code.db search --query "process" --ast-kind loops,function_item

# Find both loops and closures
llmgrep --db code.db search --query ".*" --ast-kind loops,closure_expression
```

### Available Shorthands

| Shorthand   | Expands To |
|------------|------------|
| `loops` | Loop constructs (for, while, loop) |
| `conditionals` | Conditionals (if, match) |
| `functions` | Functions and closures |
| `declarations` | Declarations (struct, enum, let, const, static) |
| `unsafe` | Unsafe blocks |
| `types` | Type definitions (struct, enum, type_alias, union) |
| `macros` | Macro invocations and definitions |
| `mods` | Module declarations |
| `traits` | Trait items and impls |
| `impls` | Impl blocks |

See `MANUAL.md` for complete node kind reference per language.


## Magellan Algorithm Integration (v1.4)

Filter search results using Magellan's graph analysis algorithms.

### Filter by Pre-computed SymbolSet

```bash
# Run magellan reachable to find all symbols reachable from main
magellan reachable --from main --db code.db --output reachable.json

# Filter llmgrep search to only show reachable symbols
llmgrep --db code.db search --query "handler" --from-symbol-set reachable.json
```

### One-shot: Find Dead Code

```bash
# Find symbols not reachable from main (dead code)
llmgrep --db code.db search --query ".*" --dead-code-in main

# Find unused utility functions
llmgrep --db code.db search --query "util_" --kind Function --dead-code-in main
```

### One-shot: Find Symbols in Dependency Cycles

```bash
# Find all functions participating in dependency cycles
llmgrep --db code.db search --query ".*" --kind Function --in-cycle process
```

### One-shot: Backward/Forward Slicing

```bash
# Backward slice: find all code that affects error handling
llmgrep --db code.db search --query "parse" --slice-backward-from handle_error

# Forward slice: find all code affected by configuration
llmgrep --db code.db search --query "validate" --slice-forward-from load_config
```

### Composed Workflow

```bash
# Complex analysis: condense -> slice -> reachable -> llmgrep -> LLM
magellan condense --db code.db --output condense.json
magellan slice --db code.db --target main --direction backward --output slice.json
llmgrep --db code.db search --from-symbol-set slice.json --query "error" --output json | llm ...
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
