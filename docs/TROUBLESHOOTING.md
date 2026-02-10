# Troubleshooting

**Last Updated:** 2026-02-10
**Version:** v3.0.1

Common issues and their solutions when using llmgrep.

---

## Table of Contents

1. [Installation Issues](#installation-issues)
2. [Database Issues](#database-issues)
3. [Query Issues](#query-issues)
4. [Performance Issues](#performance-issues)
5. [Native-V2 Issues](#native-v2-issues)
6. [Integration Issues](#integration-issues)

---

## Installation Issues

### Error: `command not found: llmgrep`

**Symptom:** Command not found after installation.

**Solutions:**

1. Check if cargo bin is in PATH:
```bash
echo $PATH | grep -o "[^:]*cargo[^:]*"
```

2. Add cargo bin to PATH (add to `~/.bashrc` or `~/.zshrc`):
```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

3. Verify installation:
```bash
cargo install --list | grep llmgrep
```

4. Reinstall if needed:
```bash
cargo install llmgrep --force
```

### Error: `linking with cc failed`

**Symptom:** Compilation fails during installation.

**Solution:** Install build dependencies:

**Debian/Ubuntu:**
```bash
sudo apt install build-essential pkg-config libsqlite3-dev
```

**Fedora:**
```bash
sudo dnf install gcc sqlite-devel
```

**macOS:**
```bash
xcode-select --install
```

**Arch:**
```bash
sudo pacman -S base-devel sqlite
```

---

## Database Issues

### Error: `database is locked`

**Symptom:** Query fails with "database is locked" error.

**Cause:** Magellan watch process is writing to the database.

**Solution:** Wait a moment and retry. The lock is released quickly after file changes.

**If persistent:**
```bash
# Check if multiple watchers are running
ps aux | grep "magellan watch"

# Kill extra watchers
pkill -f "magellan watch"
```

### Error: `no such table: symbols`

**Symptom:** Query fails with missing table error.

**Cause:** Database was created with an old version of Magellan, or is corrupted.

**Solution:** Re-index the database:
```bash
# Stop the watcher
pkill -f "magellan watch"

# Remove old database
rm .codemcp/codegraph.db*

# Re-index
magellan watch --root ./src --db .codemcp/codegraph.db --scan-initial
```

### Error: `database disk image is malformed`

**Symptom:** SQLite reports database corruption.

**Cause:** Database file was corrupted (crash, disk error, etc.).

**Solution:** Recover or rebuild:
```bash
# Attempt recovery (may save some data)
sqlite3 .codemcp/codegraph.db "PRAGMA integrity_check;"
sqlite3 .codemcp/codegraph.db ".dump" > dump.sql
sqlite3 recovered.db < dump.sql

# Or rebuild entirely (recommended)
rm .codemcp/codegraph.db*
magellan watch --root ./src --db .codemcp/codegraph.db --scan-initial
```

### Query Returns No Results

**Symptom:** Valid search returns empty results.

**Possible Causes:**

1. **Database is empty** — Check status:
```bash
magellan status --db .codemcp/codegraph.db
```

2. **Wrong path filter** — Remove `--path` filter to verify:
```bash
llmgrep --db code.db search --query "function_name"
```

3. **Case sensitivity** — Try different casing:
```bash
llmgrep --db code.db search --query "FunctionName" --regex
```

4. **Database not updated** — Trigger re-scan:
```bash
# Touch a file to trigger watcher
touch src/lib.rs

# Or scan manually
magellan watch --root ./src --db .codemcp/codegraph.db --scan-initial
```

---

## Query Issues

### Error: LLM-E106: Ambiguous symbol name

**Symptom:** Query fails with ambiguous symbol error.

**Cause:** Multiple symbols with the same name exist.

**Solutions:**

1. **Add path filter:**
```bash
llmgrep --db code.db search --query "parse" --path "src/parser/"
```

2. **Add kind filter:**
```bash
llmgrep --db code.db search --query "State" --kind Struct
```

3. **Use exact FQN:**
```bash
llmgrep --db code.db search --query "parser::State" --exact-fqn "my_crate::parser::State"
```

### Error: LLM-E105: Magellan CLI not found

**Symptom:** Algorithm filter commands fail.

**Cause:** Magellan is not installed or not in PATH.

**Solution:**
```bash
# Install Magellan
cargo install magellan

# Verify installation
magellan --version

# Retry query
llmgrep --db code.db search --condense --query ".*"
```

### Error: LLM-E107: Magellan version mismatch

**Symptom:** Algorithm filter fails with version error.

**Cause:** Magellan version is too old.

**Solution:**
```bash
# Update Magellan
cargo install magellan --force

# Verify version
magellan --version  # Should be 2.1.0 or higher
```

### Error: Invalid regex

**Symptom:** Query fails with regex error.

**Cause:** Invalid regex pattern.

**Solution:**
```bash
# Use --regex flag intentionally
llmgrep --db code.db search --query "^test_.*" --regex

# Escape special characters properly
llmgrep --db code.db search --query "func\\(.*\\)" --regex
```

---

## Performance Issues

### Query is Slow

**Symptom:** Queries take more than a second.

**Solutions:**

1. **Use specific search mode:**
```bash
# Slow: auto mode
llmgrep --db code.db search --query "parse" --mode auto

# Fast: specific mode
llmgrep --db code.db search --query "parse" --mode symbols
```

2. **Use exact match instead of regex:**
```bash
# Slower: regex
llmgrep --db code.db search --query "^parse" --regex

# Faster: prefix match
llmgrep --db code.db search --query "parse"
```

3. **Limit results:**
```bash
llmgrep --db code.db search --query ".*" --limit 50
```

4. **Remove unnecessary AST context:**
```bash
# Slower: with context
llmgrep --db code.db search --query ".*" --with-ast-context

# Faster: without context
llmgrep --db code.db search --query ".*"
```

5. **Use Native-V2 backend:**
```bash
# Re-index with Native-V2
magellan watch --root ./src --db code.db --storage native-v2 --scan-initial

# Use exclusive features
llmgrep --db code.db lookup --fqn "my_crate::module::function"
```

### Database File is Large

**Symptom:** Database file grows beyond expected size.

**Cause:** SQLite backend creates larger databases than Native-V2.

**Comparison:**
| Symbols | SQLite Size | Native-V2 Size |
|---------|-------------|----------------|
| 10,000 | ~5MB | ~2MB |
| 100,000 | ~50MB | ~15MB |

**Solution:** Consider migrating to Native-V2:
```bash
magellan watch --root ./src --db code.db --storage native-v2 --scan-initial
```

---

## Native-V2 Issues

### Error: LLM-E111: The 'complete' command requires native-v2 backend

**Symptom:** Native-V2 exclusive commands fail.

**Cause:** Database was created with SQLite backend.

**Solution:** Re-index with Native-V2:
```bash
# Stop watcher
pkill -f "magellan watch"

# Remove old database
rm .codemcp/codegraph.db*

# Re-index with Native-V2
magellan watch --root ./src --db .codemcp/codegraph.db --storage native-v2 --scan-initial

# Rebuild llmgrep with native-v2 feature
cargo install llmgrep --features native-v2 --force
```

### Native-V2 features not available

**Symptom:** `complete` and `lookup` commands not found.

**Cause:** llmgrep was built without native-v2 feature.

**Solution:**
```bash
# Rebuild with native-v2 feature
cargo install llmgrep --features native-v2 --force

# Verify
llmgrep --help | grep -E "(complete|lookup)"
```

---

## Integration Issues

### llmgrep can't find database

**Symptom:** File not found error.

**Solution:** Use absolute path or check current directory:
```bash
# Absolute path
llmgrep --db /path/to/project/.codemcp/codegraph.db search --query "main"

# Or check relative path
ls -la .codemcp/codegraph.db
llmgrep --db .codemcp/codegraph.db search --query "main"
```

### LLM receives empty results

**Symptom:** JSON output is empty array.

**Debugging:**

1. **Verify database has content:**
```bash
magellan status --db .codemcp/codegraph.db
```

2. **Test simple query:**
```bash
llmgrep --db .codemcp/codegraph.db search --query ".*" --limit 5
```

3. **Check for path mismatch:**
```bash
# Verify file exists in database
llmgrep --db .codemcp/codegraph.db search --query ".*" --path "src/"
```

### Magellan watcher not updating database

**Symptom:** New code doesn't appear in queries.

**Diagnosis:**
```bash
# Check watcher status
magellan status --db .codemcp/codegraph.db

# If not running, start it
magellan watch --root ./src --db .codemcp/codegraph.db

# Force initial scan
magellan watch --root ./src --db .codemcp/codegraph.db --scan-initial
```

---

## Getting Help

If issues persist:

1. **Enable verbose output:**
```bash
# Some commands support verbose mode
RUST_LOG=debug llmgrep --db code.db search --query "test"
```

2. **Check database integrity:**
```bash
sqlite3 .codemcp/codegraph.db "PRAGMA integrity_check;"
```

3. **Report issues:** Include database size, Magellan version, and llmgrep version.

---

## Further Reading

- [README.md](../README.md) - Quick start guide
- [MANUAL.md](../MANUAL.md) - Complete command reference
- [PERFORMANCE.md](PERFORMANCE.md) - Performance optimization
- [BEST_PRACTICES.md](BEST_PRACTICES.md) - Recommended workflows

---

*Created: 2026-02-10*
