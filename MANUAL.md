# llmgrep Manual

**v1.4.0** (shipped 2026-02-03)

llmgrep is a read-only query tool for Magellan's code map. It does not build or modify the database — Magellan owns indexing and freshness.

**Magellan repository:** https://github.com/oldnordic/magellan
**Magellan crate:** https://crates.io/crates/magellan (v1.8.0)
**sqlitegraph crate:** https://crates.io/crates/sqlitegraph (v1.2.7)

## Core command

```bash
llmgrep search --db <FILE> --query <STRING> [OPTIONS]
```

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

**Metrics filtering (v1.1):**
- `--min-complexity <N>` — Minimum cyclomatic complexity
- `--max-complexity <N>` — Maximum cyclomatic complexity
- `--min-fan-in <N>` — Minimum incoming references
- `--min-fan-out <N>` — Minimum outgoing calls

**AST filtering (v1.3):**
- `--ast-kind <KIND>` — Filter by AST node kind (supports shorthands and specific kinds)
- `--with-ast-context` — Include enriched AST context (depth, parent_kind, children, decision_points)

**Depth filtering (v1.3):**
- `--min-depth <N>` — Minimum nesting depth (decision points only)
- `--max-depth <N>` — Maximum nesting depth (decision points only)

**Structural search (v1.3):**
- `--inside <KIND>` — Find symbols within a parent of specific kind (e.g., `--inside function_item` finds closures)
- `--contains <KIND>` — Find symbols containing specific children (e.g., `--contains await_expression` finds async functions)

**Algorithm filtering (v1.4):**
- `--from-symbol-set <FILE>` — Load pre-computed SymbolSet from JSON file
- `--reachable-from <SYMBOL>` — One-shot: find symbols reachable from specified symbol
- `--dead-code-in <SYMBOL>` — One-shot: find dead code (unreachable symbols)
- `--in-cycle <SYMBOL>` — One-shot: find symbols in dependency cycles
- `--slice-backward-from <SYMBOL>` — One-shot: backward slice (code affecting target)
- `--slice-forward-from <SYMBOL>` — One-shot: forward slice (code affected by target)
- `--condense` — One-shot: find symbols in strongly connected components (SCCs)
- `--paths-from <SYMBOL>` — One-shot: find symbols on execution paths from start symbol
- `--paths-to <SYMBOL>` — Optional end symbol for path filtering (use with --paths-from)

**Sorting:**
- `--sort-by <MODE>` — Sort mode (default: `relevance`)
  - `relevance` — Intelligent scoring (LLM-friendly)
  - `position` — Fast SQL-only sorting
  - `fan-in` — Most referenced symbols first
  - `fan-out` — Symbols with most calls first
  - `complexity` — Highest complexity first

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

## AST filtering (v1.2)

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

When used with `--language`, shorthands expand to language-specific node kinds:

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
| `rust` | `conditionals` | `if_expression,match_expression,match_arm` |
| `python` | `conditionals` | `if_statement,match_statement` |
| `javascript` | `conditionals` | `if_statement,switch_statement,catch_clause` |
| `typescript` | `conditionals` | `if_statement,switch_statement,catch_clause` |
| `rust` | `declarations` | `struct_item,enum_item,let_declaration,const_item,static_item,type_alias_item` |
| `python` | `declarations` | `class_definition,type_alias_statement` |
| `javascript` | `declarations` | `class_declaration,class_expression,variable_declaration,type_alias_declaration` |
| `typescript` | `declarations` | `class_declaration,class_expression,variable_declaration,type_alias_declaration,interface_declaration,enum_declaration` |

**Examples:**

```bash
# Find all loops using shorthand
llmgrep --db code.db search --query ".*" --ast-kind loops

# Find conditionals in Python
llmgrep --db code.db search --query ".*" --ast-kind conditionals --language python

# Combine shorthands
llmgrep --db code.db search --query ".*" --ast-kind loops,conditionals

# Mix shorthands with specific kinds
llmgrep --db code.db search --query "process" --ast-kind loops,function_item

# Find specific node kind
llmgrep --db code.db search --query "parse" --ast-kind call_expression
```

### Language-specific node kinds

**Rust (tree-sitter-rust):**
- Control flow: `if_expression`, `match_expression`, `match_arm`, `while_expression`, `loop_expression`, `for_expression`, `if_let_expression`, `while_let_expression`, `let_else_expression`
- Functions: `function_item`, `closure_expression`, `async_function_item`
- Declarations: `struct_item`, `enum_item`, `let_declaration`, `const_item`, `static_item`, `type_alias_item`, `union_item`, `trait_item`, `impl_item`, `mod_item`
- Expressions: `call_expression`, `method_call_expression`, `block`, `expression_statement`
- unsafe: `unsafe_block`

**Python (tree-sitter-python):**
- Control flow: `if_statement`, `match_statement`, `for_statement`, `while_statement`, `with_statement`, `try_statement`, `except_clause`
- Functions: `function_definition`, `lambda`, `async_function_definition`
- Classes: `class_definition`, `decorated_definition`
- Comprehensions: `list_comprehension`, `dictionary_comprehension`, `set_comprehension`, `generator_expression`
- Declarations: `type_alias_statement`, `variable_declaration`
- Module: `import_statement`, `import_from_statement`, `module`

**JavaScript (tree-sitter-javascript):**
- Control flow: `if_statement`, `switch_statement`, `for_statement`, `for_in_statement`, `for_of_statement`, `while_statement`, `do_statement`, `try_statement`, `catch_clause`, `finally_clause`
- Functions: `function_declaration`, `function_expression`, `arrow_function`, `generator_function_declaration`, `generator_function_expression`, `method_definition`
- Classes: `class_declaration`, `class_expression`
- Declarations: `variable_declaration`, `type_alias_declaration`
- Modules: `import_statement`, `export_statement`

**TypeScript (tree-sitter-typescript):**
- Control flow: `if_statement`, `switch_statement`, `for_statement`, `for_in_statement`, `for_of_statement`, `while_statement`, `do_statement`, `try_statement`, `catch_clause`, `finally_clause`
- Functions: `function_declaration`, `function_expression`, `arrow_function`, `generator_function_declaration`, `generator_function_expression`, `method_definition`
- Classes: `class_declaration`, `class_expression`
- Type declarations: `type_alias_declaration`, `interface_declaration`, `enum_declaration`
- Declarations: `variable_declaration`, `type_alias_declaration`
- Modules: `import_statement`, `export_statement`

## Depth filtering (v1.3)

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

## Structural search (v1.3)

### `--inside` flag

Find symbols that are children of a parent with a specific AST node kind.

```bash
# Find closures within functions
llmgrep --db code.db search --query ".*" --inside function_item --ast-kind closure_expression

# Find local variables inside functions
llmgrep --db code.db search --query ".*" --inside function_item --ast-kind let_declaration

# Find methods within impl blocks
llmgrep --db code.db search --query ".*" --inside impl_item --ast-kind function_item
```

### `--contains` flag

Find symbols that contain children with a specific AST node kind.

```bash
# Find functions containing async calls
llmgrep --db code.db search --query ".*" --contains await_expression --ast-kind function_item

# Find functions with loops
llmgrep --db code.db search --query ".*" --contains for_expression --ast-kind function_item

# Find functions with unsafe blocks
llmgrep --db code.db search --query ".*" --contains unsafe_block --ast-kind function_item
```

### Combining structural filters

```bash
# Find functions inside impls that contain async calls
llmgrep --db code.db search --query ".*" --inside impl_item --contains await_expression --ast-kind function_item

# Find closures inside functions that contain loops
llmgrep --db code.db search --query ".*" --inside function_item --contains for_expression --ast-kind closure_expression
```

## Enriched AST context

### `--with-ast-context` flag

Include additional structural information in results:

```bash
llmgrep --db code.db search --query "process" --with-ast-context --output json
```

**Additional fields:**
- `depth` - Nesting depth from AST root (0 = top-level)
- `parent_kind` - Kind of parent AST node
- `children_count_by_kind` - Count of direct children grouped by kind
- `decision_points` - Number of decision point children

## Magellan Algorithm Integration (v1.4)

### `--from-symbol-set` FILE

Load a pre-computed SymbolSet from a JSON file and filter search results to only include symbols in the set.

**SymbolSet file format:**
```json
{
  "symbol_ids": [
    "abc123def456789012345678901234ab",
    "0123456789abcdef0123456789abcdef"
  ]
}
```

**Example:**
```bash
# Generate reachable set with magellan
magellan reachable --from main --db code.db --output reachable.json

# Search only reachable symbols
llmgrep --db code.db search --query "handler" --from-symbol-set reachable.json
```

### `--reachable-from` SYMBOL

One-shot filter: find all symbols reachable from the specified symbol.

**Example:**
```bash
# Find all functions called from main (directly or transitively)
llmgrep --db code.db search --query ".*" --reachable-from main
```

### `--dead-code-in` SYMBOL

One-shot filter: find all symbols NOT reachable from the specified symbol (dead code).

**Example:**
```bash
# Find unused functions
llmgrep --db code.db search --query ".*" --kind Function --dead-code-in main
```

### `--in-cycle` SYMBOL

One-shot filter: find all symbols that participate in a dependency cycle with the specified symbol.

**Example:**
```bash
# Find functions in dependency cycles
llmgrep --db code.db search --query ".*" --kind Function --in-cycle process
```

### `--slice-backward-from` SYMBOL

One-shot filter: find all symbols that affect the specified symbol (backward slice).

**Example:**
```bash
# Find code that affects error handling
llmgrep --db code.db search --query "parse" --slice-backward-from handle_error
```

### `--slice-forward-from` SYMBOL

One-shot filter: find all symbols affected by the specified symbol (forward slice).

**Example:**
```bash
# Find code affected by configuration loading
llmgrep --db code.db search --query "validate" --slice-forward-from load_config
```

### Error Codes

- **LLM-E105**: Magellan CLI not found. Install magellan to use algorithm features.
- **LLM-E106**: Ambiguous symbol name. Multiple symbols match the provided name. Use `--symbol-id` for unambiguous reference.
- **LLM-E107**: Magellan version mismatch. Required version not available.
- **LLM-E108**: Magellan execution failed. The algorithm command exited with an error.

### Composed Workflow Example

```bash
# Step 1: Run magellan condense to reduce graph to important symbols
magellan condense --db code.db --output condense.json

# Step 2: Run backward slice from entry point
magellan slice --db code.db --target main --direction backward --output slice.json

# Step 3: Search within sliced symbols for specific pattern
llmgrep --db code.db search --from-symbol-set slice.json --query "error" --output json

# Step 4: Pipe results to LLM for analysis
llmgrep --db code.db search --from-symbol-set slice.json --query "error" --output json | llm "Analyze these error handling patterns"
```


## Database compatibility

AST features require Magellan databases with `ast_nodes` table. If the table doesn't exist:
- AST filters are silently ignored (graceful degradation)
- No errors occur
- Results include all symbols (no AST filtering)

## Output formats

### Human (default)
Human-readable text with color-coded results when output is a terminal.

### JSON
Schema-aligned JSON for programmatic use.

### Pretty
Formatted JSON with indentation for readability.

## UTF-8 safety (v1.1)

llmgrep v1.1 uses Magellan 1.8.0's safe UTF-8 extraction functions:
- Multi-byte characters (emoji, CJK, accented) handled correctly
- No panics on character boundary splits
- Chunk-based retrieval preserves encoding

## Error handling

llmgrep uses structured error codes (LLM-E###) with:
- **Severity**: error, warning, info
- **Remediation hints**: Actionable guidance for each error type
- **Dual-mode output**: Human-friendly text or structured JSON

Error codes include:
- `LLM-E001`: Database not found
- `LLM-E002`: Database corrupted
- `LLM-E011`: Invalid query (empty)
- `LLM-E012`: Invalid SymbolId format (must be 32 hex characters)
- `LLM-E101`: Regex rejected (too complex or invalid)
- `LLM-E102`: Resource limit exceeded
- `LLM-E103`: Path validation failed

## Version history

See `CHANGELOG.md` for detailed version history.
