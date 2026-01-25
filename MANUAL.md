# llmgrep Manual

**v1.0 Production-Ready CLI** (shipped 2026-01-25)

llmgrep is a read-only query tool for Magellan's code map. It does not build or modify the database — Magellan owns indexing and freshness.

**Magellen repository:** https://github.com/oldnordic/magellan
**Magellen crate:** https://crates.io/crates/magellan

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
| `--kind <KIND>` | Filter by symbol kind (Function, Struct, etc.) |
| `--sort-by` | Sort mode: relevance (default) or position (faster, no scoring) |
| `--limit <N>` | Max results per mode (default: 50) |
| `--output` | Output format: human, json, pretty (default: human) |
| `--with-context` | Include context lines in JSON output |
| `--with-snippet` | Include code snippets in JSON output |
| `--with-fqn` | Include fully-qualified names in JSON output |
| `--fields` | JSON-only field selector (overrides `--with-*` flags) |

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

### Pretty
Formatted JSON with indentation for readability.

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

## Examples

See `llmgrep search --help` for comprehensive usage examples including:
- Basic symbol search
- Regex pattern matching
- JSON output for LLM consumption
- Path filtering
- Reference search
- Position-only sorting for performance
- Combined filters

## Version history

See `CHANGELOG.md` for detailed version history.
