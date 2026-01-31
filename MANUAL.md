# llmgrep Manual

**v1.1 Production-Ready CLI** (shipped 2026-01-31)

llmgrep is a read-only query tool for Magellan's code map. It does not build or modify the database — Magellan owns indexing and freshness.

**Magellan repository:** https://github.com/oldnordic/magellan
**Magellan crate:** https://crates.io/crates/magellan

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

### Key options

| Option | Description |
|--------|-------------|
| `--db <FILE>` | Path to Magellan SQLite database (required) |
| `--query <STRING>` | Search query string (required) |
| `--mode` | Search mode: symbols, references, calls, auto (default: symbols) |
| `--regex` | Treat query as regex pattern |
| `--path <PATH>` | Filter results by path prefix |
| `--kind <KIND>` | Filter by symbol kind (Function, Struct, etc.) - comma-separated values supported |
| `--language <LANG>` | Filter by programming language (rust, python, js, etc.) |
| `--sort-by` | Sort mode: relevance, position, fan-in, fan-out, complexity |
| `--limit <N>` | Max results per mode (default: 50) |
| `--output` | Output format: human, json, pretty (default: human) |
| `--with-context` | Include context lines in JSON output |
| `--with-snippet` | Include code snippets in JSON output |
| `--with-fqn` | Include fully-qualified names in JSON output |
| `--fields` | JSON-only field selector (overrides `--with-*` flags) |

### Metrics filtering (v1.1)

Metrics-based filtering uses the `symbol_metrics` table from Magellan v1.8.0.

| Flag | Description | Range |
|-----|-------------|-------|
| `--min-complexity <N>` | Minimum cyclomatic complexity | 0-1000 |
| `--max-complexity <N>` | Maximum cyclomatic complexity | 0-1000 |
| `--min-fan-in <N>` | Minimum incoming references | 0-10000 |
| `--min-fan-out <N>` | Minimum outgoing calls | 0-10000 |

### Metrics sorting (v1.1)

Sort results by metrics instead of relevance or position.

| Value | Description |
|-------|-------------|
| `relevance` | Intelligent scoring (default) |
| `position` | Fast SQL-only sorting |
| `fan-in` | Most referenced symbols first |
| `fan-out` | Symbols with most calls first |
| `complexity` | Highest complexity first |

### SymbolId and FQN filtering (v1.1)

Direct lookup by SymbolId or filter by Fully-Qualified Name.

| Flag | Description |
|-----|-------------|
| `--symbol-id <ID>` | Search by 32-char BLAKE3 hash (unambiguous) |
| `--fqn <PATTERN>` | Filter by FQN pattern (LIKE match, use % for wildcards) |
| `--exact-fqn <FQN>` | Exact FQN match |

### Filtering flags

| Flag | Description |
|-----|-------------|
| `--candidates` | Candidate limit for filtering (default: 500) |
| `--regex` | Enable regex matching mode |
| `--auto-limit` | Auto mode behavior: per-mode (default) or global |

### Context options

| Flag | Description | Default |
|-----|-------------|---------|
| `--with-context` | Enable context extraction | - |
| `--context-lines` | Context lines before/after | 3 |
| `--max-context-lines` | Maximum context lines | 100 |

### Snippet options

| Flag | Description | Default |
|-----|-------------|---------|
| `--with-snippet` | Enable snippet extraction | - |
| `--max-snippet-bytes` | Max snippet size in bytes | 200 |

## Output formats

### Human (default)
Human-readable text with color-coded results when output is a terminal.

### JSON
Schema-aligned JSON with the following structure:
- `results`: Array of search results
- `total_count`: Total matching results
- `partial`: Whether results were truncated (limit hit)

#### JSON output fields (v1.1)

**Basic fields:**
- `name`: Symbol name
- `kind`: Symbol kind (Function, Struct, etc.)
- `span`: Location information (file, line, column)
- `score`: Relevance score (0-100, if enabled)

**FQN fields (v1.1):**
- `fqn`: Basic fully-qualified name
- `canonical_fqn`: Unambiguous FQN with full path
- `display_fqn`: Human-readable FQN
- `symbol_id`: 32-char BLAKE3 hash for unique reference

**Metrics fields (v1.1):**
- `fan_in`: Number of incoming references
- `fan_out`: Number of outgoing calls
- `cyclomatic_complexity`: Complexity score
- `complexity_score`: Overall complexity (future)

**Content fields (v1.1):**
- `content_hash`: SHA-256 hash of snippet content
- `language`: Inferred programming language
- `kind_normalized`: Normalized kind name (lowercase)

**Example JSON output:**
```json
{
  "results": [
    {
      "match_id": "...",
      "name": "process_request",
      "kind": "Function",
      "span": {
        "file_path": "src/handler.rs",
        "byte_start": 1234,
        "byte_end": 5678,
        "start_line": 42,
        "start_col": 0,
        "end_line": 45,
        "end_col": 1
      },
      "score": 95,
      "symbol_id": "abc123def456789abc123def456789ab",
      "canonical_fqn": "my_crate::src::handler.rs::Function process_request",
      "display_fqn": "handler::process_request",
      "fan_in": 15,
      "fan_out": 8,
      "cyclomatic_complexity": 12,
      "content_hash": "a1b2c3d4...",
      "language": "rust",
      "kind_normalized": "fn"
    }
  ],
  "total_count": 1,
  "partial": false
}
```

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
- `LLM-E101`: Regex rejected (too complex or invalid)
- `LLM-E102`: Resource limit exceeded
- `LLM-E103`: Path validation failed
- `LLM-E104`: Invalid SymbolId format (v1.1)

## Examples

### Basic symbol search
```bash
llmgrep --db code.db search --query "parse"
```

### SymbolId lookup (unambiguous)
```bash
llmgrep --db code.db search --symbol-id abc123def456789abc123def456789ab
```

### Filter by complexity and language
```bash
llmgrep --db code.db search --query "handler" --min-complexity 10 --language rust
```

### Sort by fan-in to find hotspots
```bash
llmgrep --db code.db search --query ".*" --sort-by fan-in --limit 20
```

### FQN pattern matching
```bash
llmgrep --db code.db search --query "test" --fqn "%module::tests::%"
```

### Combined filters
```bash
llmgrep --db code.db search --query "^process" --regex --kind Function --path src/ --output json
```

### Auto mode with JSON output
```bash
llmgrep --db code.db search --query "parse" --mode auto --output json --with-snippet
```

### Regex search for pattern matching
```bash
llmgrep --db code.db search --query "^main" --regex
```

### Reference search
```bash
llmgrep --db code.db search --query "Token" --mode references
```

### Calls search
```bash
llmgrep --db code.db search --query "parse" --mode calls
```

See `llmgrep search --help` for more examples.

## Version history

See `CHANGELOG.md` for detailed version history.
