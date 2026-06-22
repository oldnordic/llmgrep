# llmgrep

[![Crates.io](https://img.shields.io/crates/v/llmgrep)](https://crates.io/crates/llmgrep)
[![Documentation](https://docs.rs/llmgrep/badge.svg)](https://docs.rs/llmgrep)

**Version:** 3.10.0

Pattern-based code search for Magellan databases. Fast, deterministic symbol search with JSON output.

**Positioning:** Read-only query tool for codebases indexed by Magellan. Use to find symbols, references, call relationships, source documents, and knowledge triples. Emits structured JSON for code analysis.

## Purpose

**llmgrep queries Magellan databases.** Built for:

- **Code Analysis** — Structured JSON output for programmatic analysis
- **Developers** — Fast symbol search without IDE overhead
- **Tooling** — Scriptable code analysis with precise byte spans

## Features

- **Symbol search** — Find functions, structs, traits by name or regex
- **Semantic search** — Natural-language code search via HNSW vector similarity (requires Magellan embeddings)
- **Reference lookup** — Find all callers/callees of a function
- **Call graph traversal** — Navigate caller → callee relationships
- **Graph navigation** — Stepable depth-aware traversal via `navigate` command
- **Symbol export** — Export all symbols to JSON for O(1) lookups without database queries
- **Source document search** — Query wiki pages, specs, messages by tags and wikilinks
- **Knowledge triple search** — Query candidate facts by subject, predicate, object
- **AST filtering** — Filter by node kind, nesting depth, complexity
- **Path normalization** — Consistent path handling across platforms
- **Explicit ambiguity handling** — Structured results for disambiguation
- **Parameterized SQL** — All database queries use bound parameters (no string interpolation)

## Quick Start

```bash
# Install
cargo install llmgrep

# Create SQLite database
magellan watch --root ./src --db code.db

# Search symbols
llmgrep --db code.db search --query "parse"

# Find references
llmgrep --db code.db search --query "MyType" --mode references

# Semantic search (requires `magellan embed` first)
llmgrep --db code.db search --query "parse command line arguments" --mode semantic

# Export all symbols to JSON for fast lookups
llmgrep --db code.db export-symbols --file symbolindex.json
```

## Installation

```bash
# Install with default features
cargo install llmgrep

cargo install llmgrep
```

## Backends

| Backend | File | Features | Status |
|---------|------|----------|--------|
| **SQLite** | `.db` | Full search | Default (always available) |

**Backend detection is automatic** for supported SQLite databases.

## Requirements

- **[Magellan](https://github.com/oldnordic/magellan)** with SQLite `.db` output — schema v18 supported
- **[sqlitegraph](https://crates.io/crates/sqlitegraph)** 3.0+ — Included automatically

## Documentation

- **[MANUAL.md](MANUAL.md)** — Complete command reference and examples
- **[CHANGELOG.md](CHANGELOG.md)** — Version history
- **[API_INTEGRATION.md](API_INTEGRATION.md)** — Magellan contract details
- **[INVARIANTS.md](INVARIANTS.md)** — Behavior guarantees

## What llmgrep Does NOT Do

- ❌ Index code (use [Magellan](https://github.com/oldnordic/magellan))
- ❌ Generate embeddings (use `magellan embed --db <db>`)
- ❌ Type checking or semantic analysis
- ❌ Background file watching
- ❌ Code editing (use [splice](https://github.com/oldnordic/splice))
- ❌ CFG analysis (use [Mirage](https://github.com/oldnordic/mirage))

## Acknowledgements

The `blast-score` and `export-symbols` commands, and the repo-root `.magellan/`
export convention, were inspired by
[codeindex](https://github.com/scheidydude/codeindex) by scheidydude.

## License

GPL-3.0-only. See [LICENSE](LICENSE).
