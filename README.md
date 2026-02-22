# llmgrep

[![Crates.io](https://img.shields.io/crates/v/llmgrep)](https://crates.io/crates/llmgrep)
[![Documentation](https://docs.rs/llmgrep/badge.svg)](https://docs.rs/llmgrep)

**Version:** 3.0.7

Pattern-based code search for Magellan databases. Fast, deterministic symbol search with LLM-optimized JSON output.

**Positioning:** Read-only query tool for codebases indexed by Magellan. Use to find symbols, references, and call relationships. Emits structured JSON for AI-assisted code understanding.

## Purpose

**llmgrep queries Magellan databases.** Built for:

- **AI Assistants** — Structured JSON output designed for LLM consumption
- **Developers** — Fast symbol search without IDE overhead
- **Tooling** — Scriptable code analysis with precise byte spans

## Features

- **Symbol search** — Find functions, structs, traits by name or regex
- **Reference lookup** — Find all callers/callees of a function
- **Call graph traversal** — Navigate caller → callee relationships
- **AST filtering** — Filter by node kind, nesting depth, complexity
- **Magellan algorithms** — Condense, paths, cycles, dead-code, slicing
- **FQN autocomplete** — Prefix-based symbol completion (Native-V3)
- **Exact lookup** — O(1) symbol resolution by fully-qualified name (Native-V3)

## Quick Start

```bash
# Install
cargo install llmgrep

# Requires Magellan database (create first)
magellan watch --root ./src --db code.v3

# Search symbols
llmgrep --db code.v3 search --query "parse"

# Find references
llmgrep --db code.v3 search --query "MyType" --mode references

# FQN autocomplete (Native-V3)
llmgrep --db code.v3 complete --prefix "my_crate::module::"

# Exact symbol lookup (Native-V3)
llmgrep --db code.v3 lookup --fqn "my_crate::module::function_name"

# Label-based search (Native-V3)
llmgrep --db code.v3 search --mode label --label test
```

## Installation

```bash
cargo install llmgrep
```

Or build from source with Native-V3 backend:

```bash
# Native-V3 backend (recommended - fastest, O(1) KV operations)
cargo install llmgrep --features native-v3
```

## Backends

| Feature | Description | File | Best For |
|---------|-------------|------|----------|
| `native-v3` | **High-performance binary backend** with KV store | `.v3` | Production (recommended) |
| (default) | SQLite backend via rusqlite | `.db` | Compatibility |

**Native-V3 exclusive features:**
- `complete` — FQN prefix autocomplete
- `lookup` — O(1) exact symbol lookup
- `search --mode label` — Purpose-based search (tests, entry points)

Both backends have full feature parity for standard search operations.

## Requirements

- **[Magellan](https://github.com/oldnordic/magellan)** 2.4.3+ — Required for code indexing
- **[sqlitegraph](https://crates.io/crates/sqlitegraph)** 2.0.3+ — Included automatically

## Documentation

- **[MANUAL.md](MANUAL.md)** — Complete command reference and examples
- **[CHANGELOG.md](CHANGELOG.md)** — Version history

## What llmgrep Does NOT Do

- ❌ Index code (use [Magellan](https://github.com/oldnordic/magellan))
- ❌ Type checking or semantic analysis
- ❌ Background file watching
- ❌ Code editing (use [splice](https://github.com/oldnordic/splice))
- ❌ CFG analysis (use [Mirage](https://github.com/oldnordic/mirage))

## License

GPL-3.0-only. See [LICENSE](LICENSE).
