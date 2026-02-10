# Roadmap: llmgrep

## Overview

llmgrep v3.0 adds native-v2 backend support alongside the existing SQLite backend, enabling O(1) KV lookups, smaller database sizes, and new capabilities enabled by native-v2's design. The milestone establishes a Backend trait abstraction, implements dual backend support with runtime detection, maintains full output parity across backends, and delivers native-v2 exclusive features (autocomplete, exact lookup, performance metrics).

## Milestones

- ✅ **v1.0 Production-Ready CLI** - Phases 1-6 (shipped 2026-01-25)
- ✅ **v1.1 Magellan v1.8.0 Integration** - Phase 7 (completed 2026-01-31)
- ✅ **v1.1.1 Critical Bugfix** - Phase 7.1 (completed 2026-01-31)
- ✅ **v2.0 AST-Enhanced Search** - Phases 8-10 (shipped 2026-02-01)
- ✅ **v1.4 Magellan Algorithm Integration** - Phase 11 (completed 2026-02-03)
- ✅ **v2.1 Magellan v2.1 Parity** - Phases 12-16 (shipped 2026-02-04)
- 🚧 **v3.0 Native-V2 Backend Support** - Phases 17-21 (in development)

## Phases

<details>
<summary>✅ v1.0 Production-Ready CLI (Phases 1-6) — SHIPPED 2026-01-25</summary>

**Archived to:** `.planning/milestones/v1.0-ROADMAP.md`

- [x] Phase 1: CLI Foundation (1/1 plan) — completed 2026-01-24
- [x] Phase 2: Security Hardening (4/4 plans) — completed 2026-01-24
- [x] Phase 3: Code Quality Refactoring (4/4 plans) — completed 2026-01-24
- [x] Phase 4: Test Coverage (8/8 plans) — completed 2026-01-25
- [x] Phase 5: Developer Experience (3/3 plans) — completed 2026-01-25
- [x] Phase 6: Performance & Documentation (3/3 plans) — completed 2026-01-25

</details>

<details>
<summary>✅ v1.1 Magellan v1.8.0 Integration (Phase 7) — SHIPPED 2026-01-31</summary>

**Goal:** Deep integration with Magellan v1.8.0 for safer UTF-8 handling, metrics-based queries, chunk-based retrieval, and symbol disambiguation.

**Features Delivered:**
1. Safe UTF-8 extraction with `extract_symbol_content_safe()` — handles emoji (4B), CJK (3B), accented Latin (2B)
2. SymbolId-based precise lookups with `--symbol-id` flag
3. Metrics filtering: `--min-complexity`, `--max-complexity`, `--min-fan-in`, `--min-fan-out`
4. Metrics sorting: `--sort-by fan-in|fan-out|complexity`
5. Chunk-based snippet retrieval from `code_chunks` table
6. FQN filtering with `--fqn` and `--exact-fqn` flags
7. Ambiguity detection and warning for symbol collisions
8. Language label filtering with `--language` flag

**Plans:**
- [x] 07-01 — Update dependencies and add magellan safe extraction
- [x] 07-02 — Integrate chunk-based snippet retrieval
- [x] 07-03 — Add metrics-based filtering and sorting
- [x] 07-04 — Add SymbolId, FQN, and ambiguity detection
- [x] 07-05 — Add label-based filtering
- [x] 07-06 — Update tests and documentation

**Status:** 6/6 plans complete, 194 tests passing, verification passed (40/40 must-haves)

</details>

<details>
<summary>✅ Phase 7.1: Critical Bugfix — Metrics JOIN Regression — COMPLETED 2026-01-31</summary>

**Goal:** Fix critical bug where metrics are not returned in search results due to incorrect JOIN condition.

**Bug:**
- `symbol_metrics.symbol_id` stores graph_entities `id` (integer)
- Query JOINs on `json_extract(s.data, $.symbol_id)` (SHA hash string)
- String-to-integer comparison never matches → all metrics are NULL

**Fix:**
- Line 1237 in `src/query.rs`: Changed to `LEFT JOIN symbol_metrics sm ON s.id = sm.symbol_id`
- Updated all test fixtures to match production schema (INTEGER primary key)

**Plans:**
- [x] 07.1-01 — Fix metrics JOIN condition
- [x] 07.1-02 — Add integration test for metrics
- [x] 07.1-03 — Re-verify and close v1.1

**Status:** 3/3 plans complete, 195 tests passing, verification complete

</details>

<details>
<summary>✅ Phase 8: AST Module Foundation (v2.0) — COMPLETED 2026-02-01</summary>

**Goal:** Create the AST query module and basic --ast-kind filtering for structural code search using Magellan 1.9.0's ast_nodes table.

**Features Delivered:**
1. src/ast.rs module (215 lines) with AstContext and table checking
2. `--ast-kind` CLI flag for filtering by node kind
3. AST filtering in SQL queries with LEFT JOIN pattern
4. ast_context field in JSON output with backward compatibility
5. Table existence checking for graceful degradation
6. Comprehensive test suite (8 new tests, 221 total)

**Plans:**
- [x] 08-01 — Create AST module and add --ast-kind filtering

**Status:** 1/1 plans complete, 221 tests passing, verification passed (7/7 must-haves)

**Dependencies:**
- Requires: Phase 7 (Magellan v1.8.0 integration pattern)
- Uses: Magellan 1.9.0 ast_nodes table

**AST Node Kinds Supported:**
- function_item — Function definitions
- block — Code blocks
- call_expression — Function calls
- let_declaration — Variable declarations
- expression_statement — Expression statements
- attribute_item — Attributes/macros
- mod_item — Module declarations

</details>

<details>
<summary>✅ Phase 8.2: AST Context Output and Sorting (v2.0) — COMPLETED 2026-02-01</summary>

**Goal:** Populate enriched AstContext fields and add depth/complexity-based sorting for structural code analysis.

**Features Delivered:**
1. `--with-ast-context` flag to enable enriched AST context in output
2. Enriched AstContext fields: depth, parent_kind, children_count_by_kind, decision_points
3. `--sort-by nesting-depth` option (sort by AST depth descending)
4. `--sort-by ast-complexity` option (reuse cyclomatic_complexity metric)
5. AST context calculation functions in src/ast.rs (526 lines)
6. JSON output examples in documentation
7. Comprehensive test suite (7 new tests, 227 total)

**Plans:**
- [x] 08.2-01 — Add AST context enrichment and sorting

**Status:** 1/1 plans complete, 227 tests passing, verification passed (7/7 must-haves)

**Dependencies:**
- Requires: Phase 8 (AST module foundation, AstContext struct)
- Uses: Recursive CTE for depth calculation, symbol_metrics for complexity

**Enriched AstContext Fields:**
- `depth` — Nesting depth from AST root (0 = top-level), calculated via recursive CTE
- `parent_kind` — Kind of parent AST node (null for root nodes)
- `children_count_by_kind` — Count of direct children grouped by kind
- `decision_points` — Number of branching control flow structures (if, match, while, for, loop, conditional)

</details>

<details>
<summary>✅ Phase 9: Depth and Structural Search (v2.0) — COMPLETED 2026-02-01</summary>

**Goal:** Implement depth filtering (--min-depth, --max-depth) and structural search (--inside, --contains) using recursive CTEs.

**Features Delivered:**
1. Recursive CTE for depth calculation (decision points only)
2. `--min-depth` and `--max-depth` CLI flags (range: 0-100)
3. `--inside <KIND>` flag (find children within parent of type KIND)
4. `--contains <KIND>` flag (find parents with specific children)
5. ast_depth field in SymbolMatch output
6. Post-query depth filtering for performance
7. Comprehensive test suite (10 new tests, 251 total)

**Plans:**
- [x] 09-01 — Add depth filtering and structural search

**Status:** 1/1 plans complete, 251 tests passing, verification passed (7/7 must-haves)

**Dependencies:**
- Requires: Phase 8.2 (AST context enrichment)
- Uses: Recursive CTE pattern, idx_ast_nodes_parent index

**Performance Characteristics:**
- <50K AST nodes: Fast (5-20ms per query)
- 50K-100K nodes: Acceptable performance
- >100K nodes: Future optimization (materialized depth column)

</details>

<details>
<summary>✅ Phase 10: Polish and Shorthands (v2.0) — COMPLETED 2026-02-01</summary>

**Goal:** Add shorthand groups for common AST node kinds, expand language support to Python/TypeScript/JavaScript, update documentation with AST examples, and add integration tests for multi-language scenarios.

**Features Delivered:**
1. Shorthand expansion system (--ast-kind loops → for_expression,while_expression,loop_expression)
2. Multi-language node kind mappings (Python, TypeScript, JavaScript)
3. Language-aware AST filtering (--ast-kind functions --language python)
4. Updated README.md with AST filtering examples
5. Complete MANUAL.md AST flag documentation with node kind reference
6. Integration tests for shorthands and cross-language scenarios (43 new tests, 277 total)

**Plans:**
- [x] 10-01 — Add shorthand expansion and multi-language AST support

**Status:** 1/1 plans complete, 277 tests passing, verification passed (7/7 must_haves)

**Shorthand Groups:**
| Shorthand | Expands To |
|-----------|------------|
| `loops` | for_expression,while_expression,loop_expression |
| `conditionals` | if_expression,match_expression,match_arm |
| `functions` | function_item,closure_expression |
| `declarations` | struct_item,enum_item,let_declaration,const_item,const_item |
| `unsafe` | unsafe_block |

**Multi-Language Support:**
- Python: function_definition, class_definition, async_function_definition, lambda, etc.
- JavaScript: function_declaration, arrow_function, class_declaration, etc.
- TypeScript: type_alias_declaration, interface_declaration, tsx_jsx_element, etc.

**Dependencies:**
- Requires: Phase 8 (AST module foundation)
- Uses: expand_shorthand(), get_node_kinds_for_language()

</details>

<details>
<summary>✅ Phase 11: Magellan Algorithm Integration (v1.4) — COMPLETED 2026-02-03</summary>

**Goal:** Integrate Magellan 2.0's executable graph algorithms (reachable, dead-code, slice) into llmgrep search workflow for reasoned code navigation.

**Depends on:** Phase 10

**Plans:** 4 plans

Plans:
- [x] 11-01 — Create algorithm module with SymbolSet and shell-out helpers
- [x] 11-02 — Add AlgorithmOptions to SearchOptions and CLI flags
- [x] 11-03 — Implement SymbolSet SQL filtering and algorithm filter application
- [x] 11-04 — Wire algorithm filters into search, add tests, update documentation

**Status:** 4/4 plans complete, 310 tests passing, verification passed (18/18 must-haves)

**Key Features:**
1. SymbolSet as first-class input — accept pre-computed Magellan algorithm results
2. Algorithm-aware filters — `--reachable-from`, `--dead-code-in`, `--in-cycle`, `--slice-backward-from`, `--slice-forward-from`
3. Symbol-set file input — `--from-symbol-set reachable.json`
4. FQN ergonomics — resolve simple names to SymbolId internally
5. Composed workflow: `condense → slice → reachable → llmgrep → LLM`

**Integration Pattern:**
```bash
# Explicit composition
magellan reachable --from main --output set.json
llmgrep search --db repo.db --from-symbol-set set.json --query "process"

# One-shot algorithm invocation (llmgrep shells out to magellan)
llmgrep search --db repo.db --reachable-from main --query "process"
llmgrep search --db repo.db --dead-code-in main --query "unused"
```

</details>

---

<details>
<summary>✅ v2.1 Magellan v2.1 Parity (Phases 12-16) — SHIPPED 2026-02-04</summary>

**Milestone Goal:** Achieve full feature parity with Magellan v2.1.0 by integrating missing graph algorithms (condense, paths), AST query commands (ast, find-ast), and updating dependencies.

#### Phase 12: Dependency Updates

**Goal**: Update magellan to 2.1.0 and sqlitegraph to 1.3.0, then verify Magellan availability checks before shell-out commands.

**Depends on**: Phase 11

**Requirements**: DEP-v2.1-01, DEP-v2.1-02, DEP-v2.1-03

**Success Criteria** (what must be TRUE):
  1. llmgrep compiles successfully with magellan 2.1.0 and sqlitegraph 1.3.0
  2. All 310 existing tests pass after dependency update
  3. User receives clear error with installation hint if Magellan binary is not available
  4. Existing `--reachable-from` and other algorithm flags still work after upgrade

**Plans**: 3 plans

Plans:
- [x] 12-01: Update Cargo.toml dependencies (magellan 2.1.0, sqlitegraph 1.3.0) and verify compilation
- [x] 12-02: Add Magellan availability check before shell-out with clear error message
- [x] 12-03: Run full test suite and verify backward compatibility

#### Phase 13: condense Integration

**Goal**: Integrate `magellan condense` algorithm for SCC detection and condensation-based filtering.

**Depends on**: Phase 12

**Requirements**: COND-01, COND-02

**Success Criteria** (what must be TRUE):
  1. User can run `llmgrep search --condense --db <DB>` to filter search results to symbols within SCCs
  2. Condense output is correctly parsed: supernodes[].members[] extracted into SymbolSet
  3. Search returns empty results gracefully when condense finds no SCCs
  4. User receives clear error message if condense command fails

**Plans**: 3 plans

Plans:
- [x] 13-01: Add parse_condense_output() to src/algorithm.rs with supernode member extraction
- [x] 13-02: Add --condense CLI flag and wire into SearchOptions
- [x] 13-03: Add condense integration tests and error handling

#### Phase 14: paths Integration

**Goal**: Integrate `magellan paths` algorithm for execution path enumeration with bounds to prevent explosion.

**Depends on**: Phase 13

**Requirements**: PATH-01, PATH-02, PATH-03, PATH-04

**Success Criteria** (what must be TRUE):
  1. User can run `llmgrep search --paths-from <SYMBOL> --db <DB>` to filter by execution paths
  2. User can optionally specify `--paths-to <SYMBOL>` for destination-filtered paths
  3. Default bounds (max-depth=100, max-paths=1000) are applied to prevent exponential explosion
  4. User receives warning when path enumeration hits bounds (bounded_hit: true)
  5. Empty paths and no-path scenarios are handled gracefully

**Plans**: 4 plans in 4 waves

Plans:
- [x] 14-01-PLAN.md — Add parse_paths_output() to src/algorithm.rs with path symbol extraction
- [x] 14-02-PLAN.md — Add --paths-from and --paths-to CLI flags with default bounds
- [x] 14-03-PLAN.md — Implement bounds checking and bounded_hit warning
- [x] 14-04-PLAN.md — Add paths integration tests for edge cases (empty, no path, bounds hit)

#### Phase 15: ast Command

**Goal**: Add `llmgrep ast` command for raw AST tree queries returning hierarchical JSON structure.

**Depends on**: Phase 14

**Requirements**: ASTQ-01, ASTQ-02, ASTQ-03

**Success Criteria** (what must be TRUE):
  1. User can run `llmgrep ast --db <DB> --file <PATH>` to get raw AST tree for a file
  2. User can optionally specify `--position <OFFSET>` to query node at specific position
  3. User can specify `--limit N` to limit output for large files
  4. AST output preserves parent_id relationships (hierarchical, not flat)
  5. User receives warning if output was truncated due to limit

**Plans**: 2 plans

Plans:
- [x] 15-01-PLAN.md — Add Command::Ast variant to CLI and implement run_ast() function
- [x] 15-02-PLAN.md — Complete position mode, limit handling, and integration tests

**Status:** 2/2 plans complete, 180 tests passing, verification passed (7/7 must-haves)

#### Phase 16: find-ast Command

**Goal**: Add `llmgrep find-ast` command for AST node search by kind.

**Depends on**: Phase 15

**Requirements**: FNA-01, FNA-02

**Success Criteria** (what must be TRUE):
  1. User can run `llmgrep find-ast --db <DB> --kind <KIND>` to find AST nodes by kind
  2. User receives clear message when no nodes found for given kind
  3. User receives clear error when invalid kind is specified
  4. find-ast output returns raw JSON node list (not SymbolMatch format)

**Plans**: 2 plans

Plans:
- [x] 16-01-PLAN.md — Add Command::FindAst variant to CLI and implement run_find_ast() function
- [x] 16-02-PLAN.md — Complete empty results handling, error handling, and integration tests

**Status:** 2/2 plans complete, 210 tests passing, verification passed (6/6 must-haves)

</details>

---

### 🚧 v3.0 Native-V2 Backend Support (In Development)

**Milestone Goal:** Add native-v2 backend support alongside SQLite backend, enabling O(1) KV lookups, smaller database sizes, and new capabilities enabled by native-v2's design.

#### Phase 17: Backend Infrastructure
**Goal**: Establish Backend trait abstraction and dual backend foundation
**Depends on**: Phase 16 (v2.1 complete)
**Requirements**: INFRA-01, INFRA-02, INFRA-03, INFRA-04, INFRA-05, DEP-01, DEP-02, DEP-03
**Success Criteria** (what must be TRUE):
  1. Backend trait defines search_symbols, search_references, search_calls, ast, find_ast methods
  2. Backend enum provides runtime dispatch to SqliteBackend or NativeV2Backend variants
  3. native-v2 feature flag enables native-v2 backend compilation (disabled by default)
  4. Backend format is auto-detected from database file header (MAG2 vs SQLite)
  5. SqliteBackend maintains existing rusqlite implementation with zero breaking changes
**Plans**: 5 plans in 3 waves

Plans:
- [x] 17-01-PLAN.md — Define Backend trait in src/backend/mod.rs (Wave 1)
- [x] 17-02-PLAN.md — Implement Backend enum with Sqlite and NativeV2 variants (Wave 3)
- [x] 17-03-PLAN.md — Add native-v2 feature flag to Cargo.toml (Wave 2)
- [x] 17-04-PLAN.md — Update dependencies (magellan 2.2.0+, sqlitegraph 1.5.5+) (Wave 1)
- [x] 17-05-PLAN.md — Implement backend format detection and integration tests (Wave 3)

**Status:** 5/5 plans complete, 342 tests passing, verification passed (10/10 must-haves) — Completed 2026-02-09

#### Phase 18: SqliteBackend Refactor
**Goal**: Refactor existing SQL code into SqliteBackend trait implementation
**Depends on**: Phase 17
**Requirements**: PARITY-01, PARITY-02, PARITY-03
**Success Criteria** (what must be TRUE):
  1. SqliteBackend implements Backend trait for search_symbols command
  2. SqliteBackend implements Backend trait for search_references command
  3. SqliteBackend implements Backend trait for search_calls command
  4. All existing tests pass without modification (zero logic changes)
  5. Output is identical to pre-refactor implementation
**Plans**: 6 plans in 4 waves

Plans:
- [x] 18-01-PLAN.md — Add db_path field to SqliteBackend and migrate helper functions (Wave 1)
- [x] 18-02-PLAN.md — Create search_symbols_impl() with Connection parameter (Wave 2)
- [x] 18-03-PLAN.md — Create search_references_impl() with Connection parameter (Wave 2)
- [x] 18-04-PLAN.md — Create search_calls_impl() with Connection parameter (Wave 2)
- [x] 18-05-PLAN.md — Implement BackendTrait methods on SqliteBackend (Wave 3)
- [x] 18-06-PLAN.md — Verify all existing tests pass (checkpoint) (Wave 4)

**Status:** 6/6 plans complete, 324 tests passing, verification passed (5/5 must-haves) — Completed 2026-02-09

#### Phase 19: NativeV2Backend Implementation
**Goal**: Implement native-v2 backend using CodeGraph API
**Depends on**: Phase 18
**Requirements**: PARITY-04, PARITY-05, PARITY-06
**Success Criteria** (what must be TRUE):
  1. NativeV2Backend implements ast command using CodeGraph API
  2. NativeV2Backend implements find-ast command using KV store queries
  3. Cross-backend integration tests verify ast output parity (SQLite vs native-v2)
  4. Cross-backend integration tests verify find-ast output parity
  5. Native-v2 queries produce identical results to SQLite for same input data
**Plans**: 6 plans in 4 waves

Plans:
- [x] 19-01-PLAN.md — Implement NativeV2Backend::ast using CodeGraph API (Wave 1)
- [x] 19-02-PLAN.md — Implement NativeV2Backend::find_ast using KV store queries (Wave 1)
- [x] 19-03-PLAN.md — Implement search_symbols, search_references, search_calls methods (Wave 2)
- [x] 19-04-PLAN.md — Write cross-backend integration tests for ast command (Wave 3)
- [x] 19-05-PLAN.md — Write cross-backend integration tests for find-ast command (Wave 3)
- [x] 19-06-PLAN.md — Verify output parity on test fixtures (checkpoint) (Wave 4)
**Status:** 6/6 plans complete, 333 tests passing, verification: 3/5 core must-haves passed (gaps in full parity testing due to Magellan storage design) — Completed 2026-02-09

#### Phase 20: CLI Integration ✅
**Goal**: Wire backend detection into CLI and add error handling
**Depends on**: Phase 19
**Requirements**: PARITY-07
**Status**: COMPLETE — 2026-02-10
**Success Criteria** (what must be TRUE):
  1. CLI auto-detects backend format before executing any command
  2. Clear error message when native-v2 database detected but llmgrep built without native-v2 feature
  3. Clear error message when native-v2-only command used on SQLite database
  4. Backend routing is transparent to users (no --backend flag needed)
  5. Error messages include remediation steps (e.g., "Rebuild with: cargo install llmgrep --features native-v2")
**Plans**: 5 plans in 3 waves

Plans:
- [x] 20-01-PLAN.md — Replace direct query module calls with Backend enum delegation (Wave 1)
- [x] 20-02-PLAN.md — Verify NativeV2BackendNotSupported error handling (Wave 1)
- [x] 20-03-PLAN.md — Add RequiresNativeV2Backend error for Phase 21 (Wave 2)
- [x] 20-04-PLAN.md — Add CLI integration tests (Wave 2)
- [x] 20-05-PLAN.md — Verify Phase 20 completion (checkpoint) (Wave 3)

#### Phase 21: Native-V2 Exclusive Features ✅
**Goal**: Deliver features only possible with native-v2 backend
**Depends on**: Phase 20
**Requirements**: NATIVE-01, NATIVE-02, NATIVE-03, NATIVE-04
**Status**: COMPLETE — 2026-02-10
**Success Criteria** (what must be TRUE):
  1. complete command provides prefix autocomplete via KV prefix scan
  2. lookup command provides O(1) exact symbol lookup by FQN
  3. label search mode enables purpose-based semantic search (test functions, entry points)
  4. Performance metrics display query timing breakdown (backend detection, query execution, output formatting)
  5. New commands fail gracefully on SQLite backend with clear error messages
**Plans**: 6 plans in 4 waves

Plans:
- [x] 21-01-PLAN.md — Implement complete command with KV prefix scan (Wave 1)
- [x] 21-02-PLAN.md — Implement lookup command with O(1) exact FQN lookup (Wave 1)
- [x] 21-03-PLAN.md — Add label search mode for purpose-based queries (Wave 2)
- [x] 21-04-PLAN.md — Add performance metrics instrumentation (Wave 2)
- [x] 21-05-PLAN.md — Write tests for native-v2 exclusive features (Wave 3)
- [x] 21-06-PLAN.md — Verify graceful degradation on SQLite backend (Wave 4)

### 📋 v4.0 Future Enhancements (Planned)

**Milestone Goal:** Advanced features requiring native-v2 watch/snapshot APIs

Deferred to future release. Requirements tracked but not scheduled.

- **WATCH-01**: watch command for real-time updates via pub/sub
- **SNAPSHOT-01**: snapshot export/import for state comparison

## Progress

**Execution Order:**
Phases execute in numeric order: 17 → 18 → 19 → 20 → 21

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. CLI Foundation | v1.0 | 1/1 | Complete | 2026-01-24 |
| 2. Security Hardening | v1.0 | 4/4 | Complete | 2026-01-24 |
| 3. Code Quality Refactoring | v1.0 | 4/4 | Complete | 2026-01-24 |
| 4. Test Coverage | v1.0 | 8/8 | Complete | 2026-01-25 |
| 5. Developer Experience | v1.0 | 3/3 | Complete | 2026-01-25 |
| 6. Performance & Documentation | v1.0 | 3/3 | Complete | 2026-01-25 |
| 7. Magellan v1.8.0 Integration | v1.1 | 6/6 | Complete | 2026-01-31 |
| 7.1 | Metrics JOIN Bugfix | v1.1.1 | 3/3 | Complete | 2026-01-31 |
| 8. AST Module Foundation | v2.0 | 1/1 | Complete | 2026-02-01 |
| 8.2 | AST Context Output & Sorting | v2.0 | 1/1 | Complete | 2026-02-01 |
| 9. Depth/Structural Search | v2.0 | 1/1 | Complete | 2026-02-01 |
| 10. Polish and Shorthands | v2.0 | 1/1 | Complete | 2026-02-01 |
| 11. Magellan Algorithm Integration | v1.4 | 4/4 | Complete | 2026-02-03 |
| 12. Dependency Updates | v2.1 | 3/3 | Complete | 2026-02-04 |
| 13. condense Integration | v2.1 | 3/3 | Complete | 2026-02-04 |
| 14. paths Integration | v2.1 | 4/4 | Complete | 2026-02-04 |
| 15. ast Command | v2.1 | 2/2 | Complete | 2026-02-04 |
| 16. find-ast Command | v2.1 | 2/2 | Complete | 2026-02-04 |
| 17. Backend Infrastructure | v3.0 | 5/5 | Complete | 2026-02-09 |
| 18. SqliteBackend Refactor | v3.0 | 6/6 | Complete | 2026-02-09 |
| 19. NativeV2Backend | v3.0 | 6/6 | Complete | 2026-02-09 |
| 20. CLI Integration | v3.0 | 5/5 | Complete | 2026-02-10 |
| 21. Native-V2 Features | v3.0 | 6/6 | Complete | 2026-02-10 |

**Recent Trend:**
- Phase 17 (5 plans): ~5 min each
- Phase 18 (6 plans): ~2 min each
- Phase 19 (6 plans): ~5 min each
- Phase 20 (5 plans): ~15 min per plan (integration work)
- **Phase 21 target**: 6 plans planned, ready for execution

*Updated after phase completion*
