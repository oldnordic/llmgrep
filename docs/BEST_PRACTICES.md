# Best Practices

**Last Updated:** 2026-02-10
**Version:** v3.0.1

Recommended workflows and patterns for using llmgrep effectively.

---

## Table of Contents

1. [LLM Integration](#llm-integration)
2. [Command Patterns](#command-patterns)
3. [Search Strategies](#search-strategies)
4. [Working with Large Codebases](#working-with-large-codebases)
5. [Common Workflows](#common-workflows)
6. [Anti-Patterns](#anti-patterns)

---

## LLM Integration

llmgrep is designed specifically for LLM consumption. Follow these patterns:

### 1. Always Use JSON Output for LLMs

```bash
# Good: LLM-parseable
llmgrep --db code.db search --query "auth" --output json

# Bad: Human-formatted
llmgrep --db code.db search --query "auth" --output human
```

### 2. Be Specific with Search Modes

```bash
# Good: Specific mode
llmgrep --db code.db search --query "process" --mode symbols

# Bad: Auto mode (3x slower, mixed results)
llmgrep --db code.db search --query "process" --mode auto
```

### 3. Use Filters Early

```bash
# Good: Filter at source
llmgrep --db code.db search --query "parse" --path "src/parser/" --kind Function

# Bad: Search everything, filter later
llmgrep --db code.db search --query "parse" | grep "src/parser/"
```

### 4. Request Only Fields You Need

```bash
# Good: Minimal output
llmgrep --db code.db search --query "parse" --fields name,file_path,byte_range

# Bad: Everything included
llmgrep --db code.db search --query "parse" --with-context --with-snippet --with-fqn
```

---

## Command Patterns

### Pattern: Find Symbol Definition

```bash
# Exact name match
llmgrep --db code.db search --query "function_name" --mode symbols --output json

# When name is ambiguous, add path filter
llmgrep --db code.db search --query "function_name" --path "src/module/" --mode symbols --output json
```

### Pattern: Find All Callers

```bash
# Find references to a function
llmgrep --db code.db search --query "function_name" --mode references --output json

# With file filter
llmgrep --db code.db search --query "function_name" --mode references --path "src/" --output json
```

### Pattern: Explore Call Graph

```bash
# Find what a function calls
llmgrep --db code.db search --query "function_name" --mode calls --output json

# Chain with jq for deeper analysis
llmgrep --db code.db search --query "function_name" --mode calls --output json | \
  jq -r '.results[] | .name' | \
  xargs -I {} llmgrep --db code.db search --query "{}" --mode calls --output json
```

### Pattern: AST Structural Queries

```bash
# Find all loops
llmgrep --db code.db search --query ".*" --ast-kind loops --output json

# Find deeply nested code
llmgrep --db code.db search --query ".*" --min-depth 3 --output json

# Find functions containing async operations
llmgrep --db code.db search --query ".*" --contains await_expression --ast-kind function_item --output json
```

---

## Search Strategies

### Strategy: Start Broad, Then Narrow

```bash
# Step 1: Broad search
llmgrep --db code.db search --query "auth" --limit 20 --output json

# Step 2: Narrow by kind
llmgrep --db code.db search --query "auth" --kind Function --limit 20 --output json

# Step 3: Narrow by path
llmgrep --db code.db search --query "auth" --kind Function --path "src/auth/" --output json
```

### Strategy: Use Regex for Patterns

```bash
# Find all test functions
llmgrep --db code.db search --query "^test_" --regex --output json

# Find all getters/setters
llmgrep --db code.db search --query "^(get|set)_" --regex --output json

# Find all event handlers
llmgrep --db code.db search --query "on_.*$" --regex --output json
```

### Strategy: Combine Filters

```bash
# Find complex, rarely-called functions
llmgrep --db code.db search --query ".*" \
  --min-complexity 10 \
  --max-fan-in 2 \
  --output json

# Find entry points (high fan-in, defined in main or lib)
llmgrep --db code.db search --query ".*" \
  --min-fan-in 5 \
  --path "src/" \
  --output json
```

---

## Working with Large Codebases

### Tip: Use Algorithm Filters for Analysis

```bash
# Find dead code
llmgrep --db code.db search --dead-code-in main --query ".*" --output json

# Find code in dependency cycles
llmgrep --db code.db search --in-cycle process --query ".*" --output json

# Find code on specific execution paths
llmgrep --db code.db search --paths-from main --query ".*" --output json
```

### Tip: Leverage Native-V2 Features

```bash
# Fast FQN autocomplete (Native-V2 only)
llmgrep --db code.db complete --prefix "my_crate::"

# O(1) exact lookup (Native-V2 only)
llmgrep --db code.db lookup --fqn "my_crate::module::function_name"
```

### Tip: Use Sorting for Discovery

```bash
# Find most complex functions
llmgrep --db code.db search --query ".*" --sort-by complexity --limit 20

# Find most-called functions
llmgrep --db code.db search --query ".*" --sort-by fan-in --limit 20

# Find functions that call the most others
llmgrep --db code.db search --query ".*" --sort-by fan-out --limit 20
```

---

## Common Workflows

### Workflow: Refactor a Function

```bash
# 1. Find the function
llmgrep --db code.db search --query "old_name" --mode symbols --output json

# 2. Find all references
llmgrep --db code.db search --query "old_name" --mode references --output json

# 3. Check impact (via Magellan)
magellan reachable --db code.db --symbol <ID>

# 4. Perform rename (via splice)
splice rename --symbol <ID> --file src/lib.rs --to new_name
```

### Workflow: Understand Code Structure

```bash
# 1. List all symbols in a file
llmgrep --db code.db search --query ".*" --path "src/parser.rs" --output json

# 2. Get AST structure
llmgrep --db code.db ast --file src/parser.rs

# 3. Find complex functions
llmgrep --db code.db search --query ".*" \
  --path "src/parser.rs" \
  --min-complexity 10 \
  --output json
```

### Workflow: Find Test Coverage Gaps

```bash
# 1. Find all production functions
llmgrep --db code.db search --query ".*" \
  --path "src/" \
  --kind Function \
  --output json > prod_funcs.json

# 2. Find all test functions
llmgrep --db code.db search --query ".*" \
  --path "tests/" \
  --output json > test_funcs.json

# 3. Analyze with jq or your tool of choice
```

### Workflow: Audit for Code Quality

```bash
# Find highly complex, deeply nested code
llmgrep --db code.db search --query ".*" \
  --min-complexity 15 \
  --min-depth 4 \
  --output json

# Find functions with high fan-out (too many responsibilities)
llmgrep --db code.db search --query ".*" \
  --min-fan-out 10 \
  --output json

# Find unused code (assuming main is entry point)
llmgrep --db code.db search --dead-code-in main --query ".*" --output json
```

---

## Anti-Patterns

### Don't: Use llmgrep for Text Search

```bash
# Wrong: llmgrep for substring search
llmgrep --db code.db search --query "TODO" --regex

# Right: use ripgrep
rg "TODO" ./src
```

### Don't: Ignore Database State

```bash
# Wrong: Query stale database
llmgrep --db code.db search --query "new_function"

# Right: Ensure Magellan watcher is running
magellan status --db code.db
llmgrep --db code.db search --query "new_function"
```

### Don't: Use Auto Mode Unnecessarily

```bash
# Wrong: 3x slower than needed
llmgrep --db code.db search --query "function_name" --mode auto

# Right: Use specific mode
llmgrep --db code.db search --query "function_name" --mode symbols
```

### Don't: Request Unneeded Context

```bash
# Wrong: Sends entire file content
llmgrep --db code.db search --query "parse" --with-snippet --max-snippet-bytes 10000

# Right: Get just the facts
llmgrep --db code.db search --query "parse" --output json
```

---

## Quick Reference Card

| Task | Command |
|------|---------|
| Find definition | `search --mode symbols` |
| Find callers | `search --mode references` |
| Find callees | `search --mode calls` |
| Explore AST | `ast --file <path>` |
| Find by kind | `search --ast-kind <kind>` |
| Find complex code | `search --min-complexity N` |
| Find dead code | `search --dead-code-in <entry>` |
| FQN autocomplete | `complete --prefix <prefix>` |
| Exact lookup | `lookup --fqn <fqn>` |

---

## Further Reading

- [README.md](../README.md) - Quick start guide
- [MANUAL.md](../MANUAL.md) - Complete command reference
- [PERFORMANCE.md](PERFORMANCE.md) - Benchmarks and optimization
- [TROUBLESHOOTING.md](TROUBLESHOOTING.md) - Common issues

---

*Created: 2026-02-10*
