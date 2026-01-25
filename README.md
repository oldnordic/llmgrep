# llmgrep

**v1.0 Production-Ready CLI** (shipped 2026-01-25)

A command-line search tool for code indexed by [Magellan](https://github.com/oldnordic/magellan). Queries SQLite graph databases to find symbols, references, and call relationships. Outputs human-readable or structured JSON for programmatic use.

**Crates:**
- [magellan](https://crates.io/crates/magellan) - Code indexing and database creation
- [llmgrep](https://crates.io/crates/llmgrep) - This tool (query only)

**Note:** llmgrep requires a Magellan database. It does not build or modify databases — Magellan handles indexing and freshness.

## What it does

- Searches symbols, references, and calls from Magellan's code database
- Emits deterministic, schema-aligned JSON output for LLM consumption
- Supports regex, intelligent relevance ranking, and optional context/snippets
- `--sort-by {relevance|position}` flag for LLM-optimized or performance modes

## Features

### LLM-Optimized Search
- **Relevance mode** (default): Intelligent scoring using regex matching (exact > prefix > contains)
- **Position mode**: Fast SQL-only sorting, skips in-memory scoring

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
- 118 unit tests with 87.72% coverage
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
llmgrep search --db /tmp/repo.db --query "parse"

# Position-only sorting (faster, no scoring)
llmgrep search --db /tmp/repo.db --query "parse" --sort-by position

# Regex search with JSON output
llmgrep search --db /tmp/repo.db --query "^Token" --regex --output json

# Reference search
llmgrep search --db /tmp/repo.db --query "MyType" --mode references
```

## Documentation

- `llmgrep search --help` — Built-in usage examples
- `MANUAL.md` — Full manual with all options
- `CHANGELOG.md` — Version history

## Compatibility

- **Magellen schema**: 1.0.0 (JsonResponse wrapper)
- **Magellen export**: 2.0.0
- **Minimum Magellen**: 1.7.0

## License

GPL-3.0-only. See `LICENSE.md`.
