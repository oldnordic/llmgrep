# Codebase Concerns

**Analysis Date:** 2026-02-10

## Tech Debt

### Native-V2 Backend Debug Output
- Issue: Production code contains debug `eprintln!` statements in the `complete()` method
- Files: `src/backend/native_v2.rs` (lines 522-559)
- Impact: Excessive stdout pollution in production use; indicates incomplete development work
- Fix approach: Remove all `eprintln!("DEBUG: ...")` statements or conditionally compile behind a `debug` feature flag

### Shell-Out External Process Dependency
- Issue: SQLite backend shells out to `magellan` CLI for AST queries (`ast`, `find-ast` commands)
- Files: `src/backend/sqlite.rs` (lines 87-146)
- Impact: Requires magellan binary in PATH; slower than native API; fragile to PATH changes
- Fix approach: Consider using libmagellan API directly or accepting this as permanent hybrid approach

### Hardcoded Language String
- Issue: Native-V2 backend hardcodes `language: Some("Rust".to_string())` in all search results
- Files: `src/backend/native_v2.rs` (lines 121, 228)
- Impact: Incorrect language reporting for non-Rust codebases; misleading results
- Fix approach: Infer language from file extension using `infer_language()` function from query.rs

### SQL Query Rebuild Pattern
- Issue: `search_symbols_impl()` builds SQL query twice when AST filtering is active (lines 369-422)
- Files: `src/query.rs`
- Impact: Unnecessary computation; confusing control flow
- Fix approach: Check for AST table existence before initial query build, not after

### Duplicate Search Options Construction
- Issue: Three separate mode handlers (Symbols, References, Calls) construct identical SearchOptions with minor variations
- Files: `src/main.rs` (lines 731-785, 851-879, 914-942)
- Impact: Code duplication; high maintenance burden for adding new options
- Fix approach: Extract common SearchOptions construction into helper function

## Known Bugs

### Empty Query with SymbolId bypasses validation
- Issue: Empty query string allowed when `--symbol-id` is provided, but validation happens before symbol_id check
- Files: `src/main.rs` (line 673)
- Symptoms: `--query "" --symbol-id abc...` passes validation unexpectedly
- Trigger: Empty query with valid symbol ID
- Workaround: Provide non-empty query string

### Auto Mode Missing Label Search
- Issue: Auto mode combines symbols/references/calls but does not include label search
- Files: `src/main.rs` (lines 976-1141)
- Symptoms: Cannot use `--mode auto --label test` for combined label search
- Workaround: Use `--mode labels` directly

## Security Considerations

### Regex ReDoS Mitigation Partial
- Area: User-provided regex patterns
- Risk: Complex regex patterns could cause catastrophic backtracking
- Files: `src/query.rs` (line 20: MAX_REGEX_SIZE = 10KB)
- Current mitigation: Size limit on regex pattern via `RegexBuilder::size_limit()`
- Recommendations: Add complexity detection (nested quantifiers, excessive alternation)

### Path Traversal Protection
- Area: Database and file path inputs
- Risk: Access to sensitive system directories
- Files: `src/main.rs` (lines 396-447: `validate_path()`)
- Current mitigation: Blocks `/etc`, `/root`, `/boot`, `/sys`, `/proc`, `/dev`, `/run`, `/var/run`, `/var/tmp`, `~/.ssh`, `~/.config`
- Recommendations: Add whitelist-based approach for project directories

### Shell Injection via Magellan Command
- Area: Algorithm shell-out commands
- Risk: Arguments passed to magellan subprocess not properly escaped
- Files: `src/algorithm.rs` (lines 328-365)
- Current mitigation: Uses `Command::args()` which handles basic escaping
- Recommendations: Validate all algorithm parameters before shell execution

## Performance Bottlenecks

### Native-V2 Backend Linear File Scanning
- Problem: `search_symbols()` iterates through all files and all symbols within files
- Files: `src/backend/native_v2.rs` (lines 140-254)
- Cause: No indexed symbol name lookup; must scan all symbol names for substring match
- Improvement path: Use CodeGraph's KV store for symbol name indexing if available

### SQLite Backend Shell-Out Overhead
- Problem: Each AST query spawns separate magellan process
- Files: `src/backend/sqlite.rs` (lines 87-146)
- Cause: No native API access for AST operations on SQLite backend
- Improvement path: Link against magellan library or create persistent daemon process

### String Cloning in Search Loops
- Problem: Excessive `.to_string()` calls in hot loops
- Files: `src/backend/native_v2.rs` (~17 clone operations per search result)
- Cause: String ownership patterns requiring copies
- Improvement path: Use `Cow<str>` or references where possible

## Fragile Areas

### Native-V2 Backend UnsafeCell Usage
- Files: `src/backend/native_v2.rs` (lines 33-73, 148, 264, 366, 481, 503)
- Why fragile: Requires manual verification that no concurrent access occurs
- Safe modification: Never call backend methods from multiple threads simultaneously
- Test coverage: No tests for concurrent access scenarios

### Algorithm JSON Parsing Fragility
- Files: `src/algorithm.rs` (lines 388-455: `extract_symbol_ids_from_magellan_json()`)
- Why fragile: Hardcoded JSON path access; breaks on magellan output format changes
- Safe modification: Add version check to magellan JSON format parsing
- Test coverage: Unit tests exist but use mock JSON; may not catch real format changes

### AST Node Kind String Matching
- Files: `src/ast.rs` (language-specific shorthand expansions)
- Why fragile: Depends on exact string matches from tree-sitter grammars
- Safe modification: Add fuzzy matching for node kind variations
- Test coverage: Tests use hardcoded node kinds; may miss grammar updates

## Scaling Limits

### SymbolSet Temp Table Threshold
- Current capacity: 1000 symbols triggers temporary table creation
- Files: `src/algorithm.rs` (line 885: `SYMBOL_SET_TEMP_TABLE_THRESHOLD`)
- Limit: SQLite IN clause performance degrades beyond ~1000 items
- Scaling path: Always use temp tables for symbol sets; remove threshold logic

### Path Enumeration Bounds
- Current capacity: max-depth=100, max-paths=1000 (hardcoded in algorithm.rs line 836-839)
- Limit: Large codebases may exceed these bounds silently
- Scaling path: Make bounds configurable via command-line flags

### Candidates Limit
- Current capacity: max 10,000 candidates via CLI arg validation
- Files: `src/main.rs` (line 76: `ranged_usize(1, 10000)`)
- Limit: Large codebases may need more candidates for comprehensive results
- Scaling path: Remove upper bound or make it configurable

## Dependencies at Risk

### Magellan CLI Runtime Dependency
- Risk: Shell-out pattern depends on external binary availability and version
- Impact: All algorithm features break without magellan CLI
- Files: `src/algorithm.rs`, `src/backend/sqlite.rs`
- Migration plan: Add optional `libmagellan` feature for direct library linking

### SQLiteGraph Version Compatibility
- Risk: Native-V2 backend depends on specific sqlitegraph KV store API
- Impact: Database format changes require coordinated updates
- Files: `src/backend/native_v2.rs`
- Migration plan: Version the database format and support migration paths

## Missing Critical Features

### Native-V2 Backend Metrics Filtering
- Problem: `fan_in`, `fan_out`, `cyclomatic_complexity` always return `None`
- Files: `src/backend/native_v2.rs` (lines 224-226, 331-333, 446-448)
- Blocks: Metrics-based filtering (`--min-complexity`, `--min-fan-in`) doesn't work with native-v2
- Priority: High for feature parity

### Native-V2 Backend Context/Snippet Extraction
- Problem: `context` and `snippet` fields always return `None`
- Files: `src/backend/native_v2.rs` (lines 103, 116, 226, 336, 450)
- Blocks: `--with-context` and `--with-snippet` don't work with native-v2
- Priority: High for feature parity

### Native-V2 Backend Score Calculation
- Problem: `score` always returns `None`
- Files: `src/backend/native_v2.rs` (line 113)
- Blocks: Relevance sorting unavailable in native-v2 mode
- Priority: Medium for user experience

## Test Coverage Gaps

### Untested Area: Unsafe Block Concurrency
- What's not tested: Multiple threads calling `NativeV2Backend` methods simultaneously
- Files: `src/backend/native_v2.rs`
- Risk: Undefined behavior if backend is used from multiple threads
- Priority: High (safety critical)

### Untested Area: Path Traversal Edge Cases
- What's not tested: Symlink-based path traversal attempts
- Files: `src/main.rs` (validate_path function)
- Risk: Potential bypass of directory access controls
- Priority: Medium (security)

### Untested Area: Magelland CLI Not Found Handling
- What's not tested: System without magellan installed
- Files: `src/algorithm.rs`
- Risk: Poor error messages when magellan is missing
- Priority: Low (error handling exists but not tested)

### Untested Area: Large SymbolSet Performance
- What's not tested: SymbolSets larger than 1000 items
- Files: `src/algorithm.rs`, `src/query.rs`
- Risk: Performance regression at scale
- Priority: Medium (performance)

### Untested Area: Non-Rust Languages in Native-V2
- What's not tested: Searching Python, JavaScript, or other languages via native-v2
- Files: `src/backend/native_v2.rs`
- Risk: "Rust" hardcoded language field confuses users
- Priority: Low (cosmetic but misleading)

### Untested Area: Empty Database Handling
- What's not tested: All commands against empty/just-initialized databases
- Files: All backends
- Risk: Panics or poor UX on first use
- Priority: Low (edge case)

### Untested Area: Database Corruption Recovery
- What's not tested: Opening partially-written or corrupted database files
- Files: `src/backend/native_v2.rs` (line 57)
- Risk: Cryptic error messages
- Priority: Low (rare event)

### Untested Area: Unicode Edge Cases in Search
- What's not tested: Search queries with combining characters, RTL scripts, emoji
- Files: `src/query.rs`
- Risk: Incorrect matching or display issues
- Priority: Medium (internationalization)

---

*Concerns audit: 2026-02-10*
