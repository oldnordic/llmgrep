# llmgrep

[![Crates.io](https://img.shields.io/crates/v/llmgrep)](https://crates.io/crates/llmgrep)
[![Documentation](https://docs.rs/llmgrep/badge.svg)](https://docs.rs/llmgrep)

**Version:** 3.1.6

Pattern-based code search for Magellan databases. Fast, deterministic symbol search with JSON output.

**Positioning:** Read-only query tool for codebases indexed by Magellan. Use to find symbols, references, and call relationships. Emits structured JSON for code analysis.

## Purpose

**llmgrep queries Magellan databases.** Built for:

- **Code Analysis** — Structured JSON output for programmatic analysis
- **Developers** — Fast symbol search without IDE overhead
- **Tooling** — Scriptable code analysis with precise byte spans

## Features

- **Symbol search** — Find functions, structs, traits by name or regex
- **Reference lookup** — Find all callers/callees of a function
- **Call graph traversal** — Navigate caller → callee relationships
- **AST filtering** — Filter by node kind, nesting depth, complexity
- **Geometric backend** — Native .geo file support with chunk retrieval
- **Path normalization** — Consistent path handling across platforms
- **Explicit ambiguity handling** — Structured results for disambiguation
- **Parameterized SQL** — All database queries use bound parameters (no string interpolation)

## Quick Start

```bash
# Install
cargo install llmgrep

# Create Geometric database (recommended)
magellan watch --root ./src --db code.geo

# Search symbols
llmgrep --db code.geo search --query "parse"

# Find references
llmgrep --db code.geo search --query "MyType" --mode references
```

## Installation

```bash
# Install with default features
cargo install llmgrep

# Install with geometric backend support
cargo install llmgrep --features geometric-backend
```

## Backends

| Backend | File | Features | Status |
|---------|------|----------|--------|
| **Geometric** | `.geo` | Full search + chunk retrieval + path normalization | Requires `--features geometric-backend` |
| | **SQLite** | `.db` | Full search | Default (always available) |

**Geometric (.geo) features:**
- Path normalization before queries
- Explicit ambiguity error handling
- Code chunk retrieval (no file I/O)
- No SQLite dependencies

**Backend detection is automatic** — no flags needed. Detects by file extension (`.geo`) and header bytes.

## Requirements

- **[Magellan](https://github.com/oldnordic/magellan)** 3.1.7 — Required for indexing
- **[sqlitegraph](https://crates.io/crates/sqlitegraph)** 2.0.7+ — Included automatically

## Documentation

- **[MANUAL.md](MANUAL.md)** — Complete command reference and examples
- **[CHANGELOG.md](CHANGELOG.md)** — Version history
- **[API_INTEGRATION.md](API_INTEGRATION.md)** — Magellan contract details
- **[INVARIANTS.md](INVARIANTS.md)** — Behavior guarantees

## What llmgrep Does NOT Do

- ❌ Index code (use [Magellan](https://github.com/oldnordic/magellan))
- ❌ Type checking or semantic analysis
- ❌ Background file watching
- ❌ Code editing (use [splice](https://github.com/oldnordic/splice))
- ❌ CFG analysis (use [Mirage](https://github.com/oldnordic/mirage))

## License

GPL-3.0-only. See [LICENSE](LICENSE).
