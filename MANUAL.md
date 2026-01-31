# llmgrep Manual

**v1.1.1** (shipped 2026-01-31)

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
