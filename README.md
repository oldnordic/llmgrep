# llmgrep

**v1.1 Production-Ready CLI** (shipped 2026-01-31)

A command-line search tool for code indexed by [Magellan](https://github.com/oldnordic/magellan). Queries SQLite graph databases to find symbols, references, and call relationships. Outputs human-readable or structured JSON for programmatic use.

**Crates:**
- [magellan](https://crates.io/crates/magellan) - Code indexing and database creation
- [llmgrep](https://crates.io/crates/llmgrep) - This tool (query only)

**Note:** llmgrep requires a Magellan database. It does not build or modify databases — Magellan handles indexing and freshness.

## What it does

- Searches symbols, references, and calls from Magellan's code database
- Emits deterministic, schema-aligned JSON output for LLM consumption
- Supports regex, intelligent relevance ranking, and optional context/snippets
- `--sort-by {relevance|position|fan-in|fan-out|complexity}` flag for LLM-optimized or performance modes

## v1.1 Features

**Magellan 1.8.0 Integration:**
- Safe UTF-8 content extraction (no panics on emoji/CJK/accented characters)
- Chunk-based snippet retrieval (eliminates file I/O when chunks available)
- Metrics-based filtering and sorting (complexity, fan-in, fan-out)
- SymbolId lookups via `--symbol-id` for unambiguous reference
- FQN filtering: `--fqn` (pattern) and `--exact-fqn` (exact match)
- Language filtering via `--language` flag
- Enhanced JSON fields: `symbol_id`, `canonical_fqn`, `display_fqn`, metrics, `content_hash`

## Features

### LLM-Optimized Search
- **Relevance mode** (default): Intelligent scoring using regex matching (exact > prefix > contains)
- **Position mode**: Fast SQL-only sorting, skips in-memory scoring
- **Metrics modes**: Sort by fan-in, fan-out, or complexity (v1.1)

### Security
- ReDoS prevention via 10KB regex size limit
- Resource bounds validation on all parameters
- Path traversal blocking with canonicalize()

### Developer Experience
- LLM-E### error codes with remediation hints
- Database validation (checks existence before connection)
- Distinguishes "database not found" from "database corrupted" errors
- File read error logging with context

### Quality
- 149 unit and integration tests with comprehensive coverage
- Clippy-clean codebase with zero warnings
- Comprehensive CLI examples in help text

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

llmgrep requires a Magellan database. First, build one:

```bash
# Install Magellan (if not already available)
cargo install magellan

# Build a database from your code
magellan watch --root /path/to/repo --db /tmp/repo.db
```

Then query it with llmgrep:

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

## Documentation

- `llmgrep search --help` — Built-in usage examples
- `MANUAL.md` — Full manual with all options
- `CHANGELOG.md` — Version history

## Compatibility

- **Magellan schema**: 1.0.0 (JsonResponse wrapper)
- **Magellan export**: 2.0.0
- **Minimum Magellan**: 1.8.0 (v1.1 features require metrics/chunks tables)

## License

GPL-3.0-only. See `LICENSE.md`.
