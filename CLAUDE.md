# llmgrep - Pattern-Based Code Search

**Project:** llmgrep
**Last Updated:** 2026-03-02
**Magellan Version:** 3.0.0+

---

## What is llmgrep?

llmgrep is a pattern-based code search tool that queries Magellan's database using fuzzy matching and scoring. It's faster and more accurate than text-based grep.

**Note:** llmgrep 3.0.11+ is compatible with Magellan v3.0.0 databases.

---

## Quick Start

```bash
# 1. Ensure Magellan has indexed your code (v3.0.0+)
magellan watch --root . --db .codemcp/codegraph.db --scan-initial

# 2. Search for symbols
llmgrep --db .codemcp/codegraph.db search --query "function_name" --output human

# 3. (Optional) Build LLM context for AI queries
magellan context build --db .codemcp/codegraph.db
magellan context summary --db .codemcp/codegraph.db
```

---

## Essential Commands

```bash
# Basic search (ALWAYS use --output flag)
llmgrep --db .codemcp/codegraph.db search --query "symbol_name" --output human

# Search by kind (Function, Struct, Enum, etc.)
llmgrep --db .codemcp/codegraph.db search --query "parse" --kind Function --output human

# Search in specific path
llmgrep --db .codemcp/codegraph.db search --query "config" --path "src/" --output human

# Search modes
llmgrep --db .codemcp/codegraph.db search --query "Token" --mode symbols --output human
llmgrep --db .codemcp/codegraph.db search --query "parse" --mode references --output human
llmgrep --db .codemcp/codegraph.db search --query "main" --mode calls --output human

# Sort by metrics
llmgrep --db .codemcp/codegraph.db search --query ".*" --sort-by fan-in --output human
llmgrep --db .codemcp/codegraph.db search --query ".*" --sort-by complexity --output human

# JSON output (for programmatic access)
llmgrep --db .codemcp/codegraph.db search --query "main" --output json
```

---

## Magellan v3.0.0 Integration

llmgrep works seamlessly with Magellan v3.0.0's new features:

### LLM Context Queries (Recommended for AI)

```bash
# Get project overview (~50 tokens - perfect for LLM context)
magellan context summary --db code.db

# List symbols with pagination
magellan context list --db code.db --kind fn --page 1 --page-size 50

# Get symbol details with call graph
magellan context symbol --db code.db --name main --callers --callees

# Get file context
magellan context file --db code.db --path src/main.rs
```

### Cross-Repository Search

```bash
# Export current project to LSIF
magellan export --db code.db --format lsif --output project.lsif

# Import external dependencies
magellan import-lsif --db code.db --input serde.lsif

# Now llmgrep can find symbols across imported packages
llmgrep --db code.db search --query "Deserialize" --output human
```

### LSP Enrichment

```bash
# Enrich symbols with type signatures from rust-analyzer
magellan enrich --db code.db

# Now llmgrep results include enriched type information
```

---

## Symbol Kinds

Common kinds for filtering:
- `Function` - Functions
- `Struct` - Structs
- `Enum` - Enums
- `Trait` - Traits
- `Impl` - Implementations
- `Const` - Constants
- `Static` - Static items
- `TypeAlias` - Type aliases

---

## Output Formats

**IMPORTANT:** Always specify `--output` flag for readable results:

| Format | Use Case |
|--------|----------|
| `--output human` | Terminal output (default) |
| `--output json` | Programmatic access, LLM input |

---

## Database Schema Compatibility

llmgrep reads directly from Magellan's SQLite database:

| Table | Purpose | llmgrep Usage |
|-------|---------|---------------|
| `ast_nodes` | AST hierarchy | Structural search |
| `graph_entities` | All symbols | Symbol lookup |
| `graph_edges` | Relationships | Call graph traversal |

**Schema Version:** Magellan v8 (compatible with llmgrep 3.0.11+)

---

## Troubleshooting

### "No symbols found"
1. Ensure Magellan has indexed the code: `magellan watch --root . --db code.db`
2. Check database exists: `ls -la .codemcp/codegraph.db`
3. Verify symbol exists: `magellan find --db code.db --name symbol_name`

### "Database locked"
- Magellan watch may be running. Stop it or use a different database file.

### Schema mismatch
- Update llmgrep: `cargo build --release`
- Rebuild Magellan database: `magellan watch --root . --db code.db --scan-initial`

---

## Related Tools

| Tool | Purpose |
|------|---------|
| **magellan** | Code indexing and graph database |
| **mirage** | CFG-based path analysis |
| **splice** | Refactoring with span safety |

All tools share the same database schema and are compatible with Magellan v3.0.0.
