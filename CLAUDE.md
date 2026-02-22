# llmgrep - Pattern-Based Code Search

**Project:** llmgrep
**Last Updated:** 2026-02-04

---

## What is llmgrep?

llmgrep is a pattern-based code search tool that queries Magellan's database using fuzzy matching and scoring. It's faster and more accurate than text-based grep.

---

## Quick Start

```bash
# 1. Ensure Magellan has indexed your code
magellan watch --root . --db .codemcp/codegraph.db

# 2. Search for symbols
llmgrep --db .codemcp/codegraph.db search --query "function_name" --output human
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

## Search Modes

| Mode | Description |
|------|-------------|
| `symbols` | Search symbol definitions |
| `references` | Search references to symbols |
| `calls` | Search function calls |

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

```bash
--output human   # Readable text with scores
--output json    # Compact JSON
--output pretty  # Formatted JSON
```

---

## Example Workflow

```bash
# 1. Find all functions related to "config"
llmgrep --db .codemcp/codegraph.db search --query "config" --kind Function --output human

# 2. Find what references a specific symbol
llmgrep --db .codemcp/codegraph.db search --query "MyStruct" --mode references --output human

# 3. Find all function calls from main
llmgrep --db .codemcp/codegraph.db search --query "main" --mode calls --output human

# 4. Get JSON for parsing
llmgrep --db .codemcp/codegraph.db search --query "parse" --output json
```

---

## Scoring

Results are ranked by relevance score:
- `100` = Exact name match
- Lower scores = Fuzzy matches

---

## Installation

```bash
cargo install llmgrep
```

---

## Troubleshooting

**No results returned**
- Database may be outdated - re-index (see below)
- Check Magellan status: `magellan status --db .codemcp/codegraph.db`
- Try broader search query
- Use `--mode symbols` for definitions only

**"Database not found"**
- Ensure Magellan has indexed: `magellan watch --root . --db .codemcp/codegraph.db`
- Check database path is correct

---

## Re-Indexing When Database is Outdated

```bash
# 1. Stop any running watcher
pkill -f "magellan watch"

# 2. Delete old database
rm -rf .codemcp/*.db*

# 3. Re-index (respects .gitignore)
magellan watch --root . --db .codemcp/codegraph.db

# 4. WAIT for "Scanned N files" message

# 5. Verify completion
magellan status --db .codemcp/codegraph.db
```

---

## Editing Code with Splice

After finding symbols with llmgrep, use splice for precise code editing:

```bash
# Single edit (patch function body)
cat > new_func.rs << 'EOF'
pub fn process(data: &str) -> Result<String> {
    Ok(data.to_uppercase())
}
EOF

# Preview first
splice patch --file src/lib.rs --symbol process --with new_func.rs --preview

# Apply the change
splice patch --file src/lib.rs --symbol process --with new_func.rs

# Multi-file pattern replacement
splice apply-files --glob "src/**/*.rs" --find "old_func" --replace "new_func"

# Cross-file rename
splice find --name "my_function" --path "src/lib.rs"
splice rename --symbol <id> --file src/lib.rs --to new_name
```

---
