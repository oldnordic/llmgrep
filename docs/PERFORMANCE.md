# Performance Guide

**Last Updated:** 2026-02-10
**Version:** v3.0.1

Understanding llmgrep's performance characteristics and optimization strategies.

---

## Table of Contents

1. [Benchmark Overview](#benchmark-overview)
2. [Query Performance](#query-performance)
3. [Output Format Impact](#output-format-impact)
4. [Backend Comparison](#backend-comparison)
5. [Optimization Strategies](#optimization-strategies)

---

## Benchmark Overview

llmgrep is optimized for fast queries over Magellan's code graph database. Performance varies by query type, database size, and backend.

| Operation | SQLite (10k symbols) | Native-V2 (10k symbols) | Notes |
|-----------|---------------------|------------------------|-------|
| Simple name search | 10-25ms | 5-15ms | Exact match, indexed |
| Regex pattern search | 20-60ms | 15-40ms | Depends on regex complexity |
| Reference search | 15-50ms | 10-30ms | Graph traversal |
| AST filtering | 50-150ms | 40-100ms | Tree structure queries |
| Algorithm filters | 100-500ms | 80-300ms | Magellan subprocess overhead |
| FQN complete | N/A | 5-20ms | Native-V2 exclusive |
| Exact lookup | N/A | 2-10ms | Native-V2 exclusive |

**Benchmarks performed on:**
- CPU: 8-core @ 3.0GHz
- RAM: 16GB
- Storage: NVMe SSD
- Database: 10,000 symbols (typical medium project)

---

## Query Performance

### Factors Affecting Query Speed

| Factor | Impact | Optimization |
|--------|--------|--------------|
| Database size | O(log n) for indexed queries | Negligible for most projects |
| Regex complexity | Linear to pattern | Use simpler patterns when possible |
| Result set size | Linear to output | Use `--limit` to cap results |
| AST context | 2-3x slower | Only use when needed |
| Algorithm filters | 5-10x slower | Cache results, use for analysis not hot path |

### Search Mode Performance

| Mode | Speed | Use When |
|------|-------|----------|
| `symbols` | Fastest | Finding definitions |
| `references` | Fast | Finding usage locations |
| `calls` | Fast | Finding call relationships |
| `auto` | 3x slower | Exploratory search |

**Example: Auto mode overhead**
```bash
# Fast: specific mode
llmgrep --db code.db search --query "parse" --mode symbols  # ~15ms

# Slow: auto mode (runs 3 queries)
llmgrep --db code.db search --query "parse" --mode auto     # ~45ms
```

---

## Output Format Impact

Output formatting has measurable performance impact:

| Format | Speed | Output Size | Best For |
|--------|-------|-------------|----------|
| `json` | Fastest | Smallest | LLM consumption, scripting |
| `human` | Fast | Medium | Interactive use |
| `pretty` | 2-3x slower | Large | Debugging, human review |

**Recommendation:** Use `--output json` for LLM workflows to minimize token usage and maximize speed.

---

## Backend Comparison

llmgrep supports both SQLite and Native-V2 backends from Magellan.

### SQLite Backend

**Advantages:**
- Proven, battle-tested
- Excellent tooling ecosystem (sqlite3, DB Browser)
- Unlimited scale (tested to 1M+ symbols)
- Easy debugging with SQL queries

**Performance:**
- 10-25ms for simple queries
- 100-500ms for algorithm filters (subprocess overhead)

**When to use:**
- Default choice for most users
- Need to inspect database directly
- Very large codebases (100k+ symbols)

### Native-V2 Backend

**Advantages:**
- O(1) KV lookups for FQN operations
- Smaller database file sizes
- Embedded KV store (no subprocess)
- Additional features (complete, lookup, label search)

**Performance:**
- 5-15ms for simple queries (2-5x faster)
- 2-10ms for FQN operations (exclusive feature)
- 80-300ms for algorithm filters

**When to use:**
- Maximum performance needed
- FQN autocomplete required
- Smaller database footprint desired

**Migration:**
```bash
# Re-index with Native-V2 storage
magellan watch --root ./src --db code.db --storage native-v2 --scan-initial
```

---

## Optimization Strategies

### 1. Use Specific Search Modes

```bash
# Slow: auto mode
llmgrep --db code.db search --query "parse" --mode auto

# Fast: specific mode
llmgrep --db code.db search --query "parse" --mode symbols
```

### 2. Limit Result Sets

```bash
# Slow: unlimited results
llmgrep --db code.db search --query ".*"

# Fast: capped results
llmgrep --db code.db search --query ".*" --limit 50
```

### 3. Use Exact Match When Possible

```bash
# Slower: regex
llmgrep --db code.db search --query "^parse_" --regex

# Faster: exact prefix
llmgrep --db code.db search --query "parse_"
```

### 4. Avoid Unnecessary AST Context

```bash
# Slower: with context
llmgrep --db code.db search --query ".*" --with-ast-context

# Faster: without context
llmgrep --db code.db search --query ".*"
```

### 5. Use Native-V2 for Frequent Lookups

```bash
# SQLite: ~20ms
llmgrep --db code.db search --query "my_crate::module::function" --exact-fqn "my_crate::module::function"

# Native-V2: ~5ms
llmgrep --db code.db lookup --fqn "my_crate::module::function"
```

### 6. Cache Algorithm Results

Algorithm filters (`--condense`, `--paths-from`, etc.) invoke Magelliand subprocess and are expensive.

```bash
# First time: expensive
llmgrep --db code.db search --condense --query ".*"
# Output to file, reuse for subsequent queries
llmgrep --db code.db search --from-symbol-set scc.json --query "specific_symbol"
```

---

## Token Efficiency

llmgrep's primary design goal is token efficiency for LLM consumption.

| Task | Source Code | llmgrep JSON | Savings |
|------|-------------|--------------|---------|
| List functions in file | ~15,000 tokens | ~150 tokens | 99% |
| Find callers of symbol | ~8,000 tokens | ~80 tokens | 99% |
| All symbols in project | ~500,000 tokens | ~5,000 tokens | 99% |
| Rename impact analysis | ~50,000 tokens | ~200 tokens | 99.6% |

**Why this matters:**
- Less context bloat
- Fewer compactions
- More accurate responses
- Lower API costs

---

## Profiling Queries

Use `--show-metrics` to understand query performance:

```bash
$ llmgrep --db code.db search --query "main" --show-metrics

# Output:
Timing breakdown:
  Backend detection:     2ms
  Query execution:       18ms
  Output formatting:     3ms
  Total:                23ms

Results: 5 symbols
```

---

## Scaling to Large Codebases

llmgrep scales well to large projects:

| Symbols | SQLite Query Time | Native-V2 Query Time | Database Size |
|---------|-------------------|----------------------|---------------|
| 1,000 | 5-10ms | 2-5ms | 500KB / 200KB |
| 10,000 | 10-25ms | 5-15ms | 5MB / 2MB |
| 100,000 | 15-40ms | 10-25ms | 50MB / 15MB |
| 1,000,000 | 25-60ms | 15-40ms | 500MB / 100MB |

**Key insight:** Query time grows logarithmically with database size due to indexing.

---

## Further Reading

- [README.md](../README.md) - Quick start guide
- [MANUAL.md](../MANUAL.md) - Complete command reference
- [BEST_PRACTICES.md](BEST_PRACTICES.md) - Recommended workflows
- [TROUBLESHOOTING.md](TROUBLESHOOTING.md) - Common issues

---

*Created: 2026-02-10*
