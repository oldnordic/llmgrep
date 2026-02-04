# llmgrep Manual

**v2.1.0** (shipped 2026-02-04)

llmgrep is a read-only query tool for Magellan's code map. Part of the sqlitegraph toolset alongside Magellan (indexing), Mirage (CFG analysis), and Splice (precision editing).

llmgrep only works in conjunction with Magellan — it does not build or modify databases. Magellan owns indexing and freshness.

**Toolset:**
- [Magellan](https://crates.io/crates/magellan) v2.1.0 — Code indexing and algorithm execution
- [llmgrep](https://crates.io/crates/llmgrep) v2.1.0 — This tool (query only)
- [Mirage](https://crates.io/crates/mirage-analyzer) — CFG analysis (Rust)
- [Splice](https://crates.io/crates/splice) — Precision code editing
- [sqlitegraph](https://crates.io/crates/sqlitegraph) v1.3.0 — Graph database with 35+ algorithms

## Commands

```bash
llmgrep search --db <FILE> --query <STRING> [OPTIONS]
llmgrep ast --db <FILE> --file <PATH> [OPTIONS]
llmgrep find-ast --db <FILE> --kind <KIND> [OPTIONS]
```

## search command

### Search modes

| Mode | Description |
|------|-------------|
| `symbols` | Search symbol definitions (default) |
| `references` | Search references to symbols |
| `calls` | Search function calls |
| `auto` | Run all three modes and combine results |

### Options

**Required:**
- `--db <FILE>` — Path to Magellan SQLite database
- `--query <STRING>` — Search query string

**Search mode:**
- `--mode <MODE>` — Search mode: `symbols` (default), `references`, `calls`, `auto`

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
  - `relevance` — Intelligent scoring (LLM-friendly)
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

- **LLM-E105**: Magellan CLI not found. Install magellan to use algorithm features.
- **LLM-E106**: Ambiguous symbol name. Multiple symbols match.
- **LLM-E107**: Magellan version mismatch. Required version not available.
- **LLM-E108**: Magellan execution failed. The algorithm command exited with an error.

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

## Version history

See `CHANGELOG.md` for detailed version history.
