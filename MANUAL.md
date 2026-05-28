# llmgrep Manual

**v3.6.0** (shipped 2026-05-28)

llmgrep is a read-only query tool for Magellan's code map. Part of the sqlitegraph toolset alongside Magellan (indexing), Mirage (CFG analysis), and Splice (precision editing).

llmgrep only works in conjunction with Magellan — it does not build or modify databases. Magellan owns indexing and freshness.

**Toolset:**
- [Magellan](https://crates.io/crates/magellan) v3.3.3 — Code indexing and algorithm execution
- [llmgrep](https://crates.io/crates/llmgrep) v3.6.0 — This tool (query only)
- [Mirage](https://crates.io/crates/mirage-analyzer) v1.5.0 — CFG analysis (Rust)
- [Splice](https://crates.io/crates/splice) — Precision code editing
- [sqlitegraph](https://crates.io/crates/sqlitegraph) v3.0 — Graph database with 35+ algorithms

## Commands

```bash
llmgrep search --db <FILE> --query <STRING> [OPTIONS]
llmgrep ast --db <FILE> --file <PATH> [OPTIONS]
llmgrep find-ast --db <FILE> --kind <KIND> [OPTIONS]
llmgrep explore --db <FILE> --intent <STRING> [OPTIONS]
llmgrep navigate --db <FILE> --symbol <NAME> [OPTIONS]
llmgrep stats --db <FILE> [OPTIONS]
llmgrep evolve --db <FILE> [OPTIONS]
```

## search command

### Search modes

| Mode | Description |
|------|-------------|
| `symbols` | Search symbol definitions (default) |
| `references` | Search references to symbols |
| `calls` | Search function calls |
| `implements` | Search type-trait implementations |
| `docs` | Search source documents (wiki, specs, messages) |
| `facts` | Search candidate knowledge triples |
| `auto` | Run symbols, references, and calls modes combined |

### Options

**Required:**
- `--db <FILE>` — Path to Magellan SQLite database
- `--query <STRING>` — Search query string

**Search mode:**
- `--mode <MODE>` — Search mode: `symbols` (default), `references`, `calls`, `implements`, `docs`, `facts`, `auto`

**Filters:**
- `--path <PATH>` — Filter by file path prefix
- `--kind <KIND>` — Filter by symbol kind (Function, Struct, Method, Class, Interface, Enum, Module, Union, Namespace, TypeAlias)
- `--language <LANGUAGE>` — Filter by programming language (rust, python, javascript, typescript, c, cpp, java, go)
- `--regex` — Treat query as regex pattern
- `--fqn <PATTERN>` — Filter by FQN pattern (LIKE match, use % for wildcards)
- `--exact-fqn <FQN>` — Exact FQN match
- `--symbol-id <SYMBOL_ID>` — Search by 32-char BLAKE3 hash (unambiguous)

**Metrics filtering:**
- `--min-complexity <N>` — Minimum cyclomatic complexity
- `--max-complexity <N>` — Maximum cyclomatic complexity
- `--min-fan-in <N>` — Minimum incoming references
- `--min-fan-out <N>` — Minimum outgoing calls

**AST filtering:**
- `--ast-kind <KIND>` — Filter by AST node kind (supports shorthands and specific kinds)
- `--with-ast-context` — Include enriched AST context (depth, parent_kind, children, decision_points)

**Depth filtering:**
- `--min-depth <N>` — Minimum nesting depth (decision points only)
- `--max-depth <N>` — Maximum nesting depth (decision points only)

**Structural search:**
- `--inside <KIND>` — Find symbols within a parent of specific kind
- `--contains <KIND>` — Find symbols containing specific children

**Algorithm filtering:**
- `--from-symbol-set <FILE>` — Load pre-computed SymbolSet from JSON file
- `--condense` — Filter to symbols in strongly connected components (SCCs)
- `--paths-from <SYMBOL>` — Filter by execution paths from start symbol
- `--paths-to <SYMBOL>` — Optional end symbol for path filtering
- `--reachable-from <SYMBOL>` — Find symbols reachable from specified symbol
- `--dead-code-in <SYMBOL>` — Find dead code (unreachable symbols)
- `--in-cycle <SYMBOL>` — Find symbols in dependency cycles
- `--slice-backward-from <SYMBOL>` — Backward slice (code affecting target)
- `--slice-forward-from <SYMBOL>` — Forward slice (code affected by target)

**Sorting:**
- `--sort-by <MODE>` — Sort mode (default: `relevance`)
  - `relevance` — Text relevance scoring with configurable weight factors
  - `position` — Fast SQL-only sorting
  - `fan-in` — Most referenced symbols first
  - `fan-out` — Symbols with most calls first
  - `complexity` — Highest complexity first
  - `nesting-depth` — Deepest nested first

**Limits:**
- `--limit <N>` — Max results (default: 50)
- `--candidates <N>` — Candidate limit for filtering (default: 500)
- `--auto-limit <MODE>` — Auto mode behavior: `per-mode` (default) or `global`

**Output:**
- `--output <FORMAT>` — Output format: `human` (default), `json`, `pretty`

**Content inclusion:**
- `--with-context` — Include context lines in output
- `--with-snippet` — Include code snippets in output
- `--with-fqn` — Include fully-qualified names in output
- `--fields <FIELDS>` — JSON-only field selector (overrides `--with-*` flags)

**Context options:**
- `--context-lines <N>` — Context lines before/after (default: 3)
- `--max-context-lines <N>` — Maximum context lines (default: 20)

**Snippet options:**
- `--max-snippet-bytes <N>` — Max snippet size in bytes (default: 200)

**Docs mode filters** (`--mode docs`):
- `--tags <TAGS>` — Filter by tags (comma-separated, OR match)
- `--wikilinks <LINKS>` — Filter by wikilinks
- `--source-kind <KIND>` — Filter by source kind (wiki, code, message, etc.)
- `--since <TIMESTAMP>` — Filter by timestamp (Unix epoch)
- `--path <PATH>` — Filter by document path prefix

**Facts mode filters** (`--mode facts`):
- `--subject <SUBJECT>` — Filter by subject key (LIKE match)
- `--predicate <PRED>` — Filter by predicate (exact match)
- `--object <OBJECT>` — Filter by object key (LIKE match)
- `--status <STATUS>` — Filter by status (pending, accepted, rejected, ambiguous)
- `--subject-type <TYPE>` — Filter by subject type (Task, Agent, Event, etc.)

### Docs and Facts examples

```bash
# Find wiki documents tagged "rust"
llmgrep search --db code.db --mode docs --tags "rust"

# Find documents about a specific topic via wikilinks
llmgrep search --db code.db --mode docs --wikilinks "memory-system"

# List all candidate facts assigned to an agent
llmgrep search --db code.db --mode facts --predicate assigned_to --subject-type Task

# Find rejected facts for review
llmgrep search --db code.db --mode facts --status rejected --output json
```

## ast command (v2.1)

Query raw AST tree for a file.

```bash
llmgrep ast --db <FILE> --file <PATH> [OPTIONS]
```

**Options:**
- `--db <FILE>` — Path to Magellan SQLite database (required)
- `--file <PATH>` — Path to source file (required)
- `--position <OFFSET>` — Query AST node at specific byte offset
- `--limit <N>` — Limit number of nodes returned (default: 1000)

**Output:** Hierarchical JSON structure preserving parent_id relationships.

## find-ast command (v2.1)

Find AST nodes by kind.

```bash
llmgrep find-ast --db <FILE> --kind <KIND>
```

**Options:**
- `--db <FILE>` — Path to Magellan SQLite database (required)
- `--kind <KIND>` — AST node kind to search (required)

**Output:** JSON list of matching AST nodes.

## explore command (v3.5)

Intent-based code search using graph metadata — no embeddings needed.

```bash
llmgrep explore --db code.db --intent "database connection pooling"
llmgrep explore --db code.db --intent "error handling" --output json
llmgrep explore --db code.db --intent "cfg" --limit 5
```

### How it works

1. Tokenizes the intent string (strips stop words, expands abbreviations like `db`→`database`)
2. Searches symbols via FTS5 prefix match + LIKE fallback
3. Ranks candidates by name match score + fan-in (heavily referenced symbols rank higher)
4. Clusters results by file/module

### Options

| Flag | Default | Description |
|------|---------|-------------|
| `--intent` | (required) | Natural language description of what you're looking for |
| `--limit` | 10 | Max symbols to return |
| `--output` | human | Output format: `human`, `json`, `pretty` |

### Abbreviation expansion

The tokenizer expands common abbreviations: `db`→`database`, `cfg`→`config`, `impl`→`implement`, `auth`→`authentication`, `fn`→`function`, `util`→`utility`, `conn`→`connection`, `err`→`error`, `svc`→`service`, `mgr`→`manager`, and more.

### Example output

```
Exploring: "database connection pooling"

Module: graph (score: 36)
  File: src/graph/db_compat.rs
  open_database (Function, fan-in: 12)
  database_path (Function, fan-in: 5)

Module: storage (score: 15)
  File: src/storage/sqlite.rs
  SqliteBackend::new (Function, fan-in: 3)

2 modules, 3 symbols found
```

### When to use

- **Instead of 3-5 separate `search` commands** when you know *what* you want but not the exact symbol name
- **For codebase orientation** in an unfamiliar project
- **For agent integration** — single command replaces multi-query round trips

## navigate command (v3.6)

Stepable graph navigation using magellan's `SymbolNavigator`. Resolve a symbol, then traverse its call graph with configurable depth.

```bash
llmgrep navigate --db code.db --symbol "function_name" --callees --depth 2
llmgrep navigate --db code.db --symbol "function_name" --callers --depth 1 --output json
llmgrep navigate --db code.db --id 42 --edges
```

### How it works

1. Resolves the starting symbol by name (`--symbol`) or entity ID (`--id`)
2. Traverses the call graph in the requested direction(s) up to `--depth` hops
3. Returns depth-tagged node lists, edges, and optional resolve information

### Options

| Flag | Default | Description |
|------|---------|-------------|
| `--symbol` | — | Symbol name to resolve (mutually exclusive with `--id`) |
| `--id` | — | Entity ID to use directly (mutually exclusive with `--symbol`) |
| `--callers` | false | Traverse callers (incoming calls) |
| `--callees` | false | Traverse callees (outgoing calls) |
| `--edges` | false | Include connected entity edges in output |
| `--depth` | 1 | Maximum traversal depth |
| `--output` | human | Output format: `human`, `json`, `pretty` |

### Examples

```bash
# Show all functions called by parse_config, 2 levels deep
llmgrep navigate --db code.db --symbol "parse_config" --callees --depth 2

# Show who calls parse_config (1 level)
llmgrep navigate --db code.db --symbol "parse_config" --callers --depth 1 --output json

# Show all edges connected to entity 42
llmgrep navigate --db code.db --id 42 --edges
```

## stats command (v3.5)

Code health summary from the database. No arguments required.

```bash
llmgrep stats
llmgrep stats --output json
```

### What it shows

- Symbol counts by kind (function, struct, trait, etc.)
- Dead code detection (symbols with zero fan-in and zero fan-out)
- Top hotspots ranked by composite score (fan-in × complexity)
- Coverage gaps (files in graph but not indexed)

## evolve command (v3.5)

Score symbols by `fan_in × cyclomatic_complexity` and optionally write high-impact candidates to `candidate_facts` table.

```bash
# Dry run — show scores without writing
llmgrep evolve --dry-run --min-score 50

# Write candidates to database
llmgrep evolve --min-score 8 --limit 20

# JSON output for scripting
llmgrep evolve --output json
```

### Options

| Flag | Default | Description |
|------|---------|-------------|
| `--min-score` | 8 | Minimum composite score to include |
| `--limit` | 50 | Max candidates to return |
| `--dry-run` | false | Show scores without writing to database |
| `--output` | human | Output format: `human`, `json`, `pretty` |

## AST filtering

### `--ast-kind` flag

Filter search results by AST node kind for structural code queries.

**Syntax:**
```bash
--ast-kind <KIND|SHORTHAND>[,...]
```

**Shorthands expand to multiple node kinds:**

| Shorthand | Expands To | Description |
|-----------|------------|-------------|
| `loops` | `for_expression,while_expression,loop_expression` | Loop constructs |
| `conditionals` | `if_expression,match_expression,match_arm` | Conditionals |
| `functions` | `function_item,closure_expression,async_function_item` | Functions |
| `declarations` | `struct_item,enum_item,let_declaration,const_item,static_item,type_alias_item` | Declarations |
| `unsafe` | `unsafe_block` | Unsafe blocks |
| `types` | `struct_item,enum_item,type_alias_item,union_item` | Type definitions |
| `macros` | `macro_invocation,macro_definition,macro_rule` | Macros |
| `mods` | `mod_item` | Module declarations |
| `traits` | `trait_item,trait_impl_item` | Trait items and impls |
| `impls` | `impl_item` | Impl blocks |

**Language-aware expansion:**

When used with `--language`, shorthands expand to language-specific node kinds.

| Language | Shorthand | Expands To |
|----------|-----------|------------|
| `rust` | `loops` | `for_expression,while_expression,loop_expression` |
| `python` | `loops` | `for_statement,while_statement` |
| `javascript` | `loops` | `for_statement,for_in_statement,for_of_statement,while_statement,do_statement` |
| `typescript` | `loops` | `for_statement,for_in_statement,for_of_statement,while_statement,do_statement` |
| `rust` | `functions` | `function_item,closure_expression,async_function_item` |
| `python` | `functions` | `function_definition,lambda,async_function_definition` |
| `javascript` | `functions` | `function_declaration,function_expression,arrow_function,generator_function_declaration,generator_function_expression` |
| `typescript` | `functions` | `function_declaration,function_expression,arrow_function,generator_function_declaration,generator_function_expression` |

## Depth filtering

### Decision depth

Depth is measured as **decision points only** (branching control flow structures):
- `if_expression`, `match_expression`, `for_expression`, `while_expression`, `loop_expression`

Root-level code has depth 0. Each decision point ancestor adds 1.

**Examples:**
```bash
# Find deeply nested code (complexity hotspots)
llmgrep --db code.db search --query ".*" --min-depth 5

# Find only top-level code
llmgrep --db code.db search --query "process" --max-depth 1

# Find code at specific depth range
llmgrep --db code.db search --query ".*" --min-depth 2 --max-depth 3
```

## Structural search

### `--inside` flag

Find symbols that are children of a parent with a specific AST node kind.

```bash
# Find closures within functions
llmgrep --db code.db search --query ".*" --inside function_item --ast-kind closure_expression

# Find local variables inside functions
llmgrep --db code.db search --query ".*" --inside function_item --ast-kind let_declaration
```

### `--contains` flag

Find symbols that contain children with a specific AST node kind.

```bash
# Find functions containing async calls
llmgrep --db code.db search --query ".*" --contains await_expression --ast-kind function_item

# Find functions with loops
llmgrep --db code.db search --query ".*" --contains for_expression --ast-kind function_item
```

## Magellan Algorithm Integration

### `--condense` flag (v2.1)

Filter search results to symbols in strongly connected components (SCCs). Detects dependency cycles in the call graph.

```bash
# Find all symbols participating in dependency cycles
llmgrep --db code.db search --condense --query ".*"
```

### `--paths-from` / `--paths-to` flags (v2.1)

Filter by execution paths using bounded DFS to prevent exponential explosion.

```bash
# Find symbols on paths from main
llmgrep --db code.db search --paths-from main --query ".*"

# Find symbols on paths between two symbols
llmgrep --db code.db search --paths-from parse --paths-to execute --output json
```

**Default bounds:** max-depth=100, max-paths=1000

### `--from-symbol-set` FILE

Load a pre-computed SymbolSet from a JSON file and filter search results.

**SymbolSet file format:**
```json
{
  "symbol_ids": [
    "abc123def456789012345678901234ab",
    "0123456789abcdef0123456789abcdef"
  ]
}
```

### `--reachable-from` SYMBOL

Find all symbols reachable from the specified symbol.

### `--dead-code-in` SYMBOL

Find all symbols NOT reachable from the specified symbol (dead code).

### `--in-cycle` SYMBOL

Find all symbols that participate in a dependency cycle with the specified symbol.

### `--slice-backward-from` / `--slice-forward-from` SYMBOL

Backward slice: code affecting target. Forward slice: code affected by target.

## Error Codes

- **SPL-E105**: Magellan CLI not found. Install magellan to use algorithm features.
- **SPL-E106**: Ambiguous symbol name. Multiple symbols match.
- **SPL-E107**: Magellan version mismatch. Required version not available.
- **SPL-E108**: Magellan execution failed. The algorithm command exited with an error.

## Database compatibility

AST features require Magellan databases with `ast_nodes` table. If the table doesn't exist, AST filters are silently ignored (graceful degradation).

Algorithm features require Magellan 2.1.0+ CLI to be installed.

## Output formats

### Human (default)
Human-readable text with color-coded results when output is a terminal.

### JSON
Schema-aligned JSON for programmatic use.

### Pretty
Formatted JSON with indentation for readability.

## Error Codes

| Code | Description | Solution |
|------|-------------|----------|
| **SPL-E105** | Magellan CLI not found | Install Magellan: `cargo install magellan` |
| **SPL-E106** | Ambiguous symbol name | Add `--path` or `--kind` filter to disambiguate |
| **SPL-E107** | Magellan version mismatch | Update Magellan: `cargo install magellan --force` |
| **SPL-E108** | Magellan execution failed | Check Magellan logs, verify database integrity |
| **SPL-E112** | Database file not found | Verify database path, run `magellan watch` |
| **SPL-E113** | Database table missing | Reindex database with `--scan-initial` |
| **SPL-E114** | Invalid regex pattern | Check regex syntax, escape special characters |
| **SPL-E115** | Invalid FQN format | Use valid FQN format: `crate::module::symbol` |
| **SPL-E116** | Symbol not found | Verify symbol exists, check spelling, use `--ambiguous` flag |
| **SPL-E117** | Timeout executing algorithm | Reduce search scope, use `--limit` |

## Best Practices

### For Programmatic Use

1. **Always use `--output json`** for structured output
2. **Use specific `--mode`** instead of `auto` (3x faster)
3. **Request only needed fields** with `--fields`
4. **Combine filters** to reduce result set early

### For Interactive Use

1. **Use `--output human`** for terminal display
2. **Add `--show-metrics`** when debugging performance
3. **Use `--limit`** to cap large result sets
4. **Use `--sort-by`** for discovery (complexity, fan-in)

### For Scripting

1. **Use `--output json`** with `jq` for processing
2. **Prefer exact match** over regex when possible
3. **Cache algorithm results** (expensive Magelliand subprocess calls)

## Performance Tips

| Tip | Impact |
|-----|--------|
| Use `--mode symbols` instead of `auto` | 3x faster |
| Remove `--with-ast-context` unless needed | 2-3x faster |
| Use `--limit` on wildcard queries | Prevents large outputs |
| Cache algorithm filter results | Avoid subprocess overhead |

## Version history

See `CHANGELOG.md` for detailed version history.

## Further Documentation

- **[README.md](README.md)** — Quick start and overview
- **[CHANGELOG.md](CHANGELOG.md)** — Version history
- **[ARCHITECTURE.md](ARCHITECTURE.md)** — Component design
- **[API_INTEGRATION.md](API_INTEGRATION.md)** — Magellan contract details
