# Phase 18: SqliteBackend Refactor - Context

**Created:** 2026-02-09
**Status:** Planning complete, execution pending

## Overview

Phase 18 refactors existing SQL query code from `src/query.rs` into `SqliteBackend` trait implementation in `src/backend/sqlite.rs`. This is a pure code-movement refactoring—no new logic is added, only reorganization. The three-step strategy ensures zero breaking changes:

1. **Wave 1**: Move helper functions to sqlite.rs, add db_path field
2. **Wave 2**: Create `_impl()` functions taking `&Connection`
3. **Wave 3**: Wire up trait methods calling `_impl()` functions with `&self.conn`
4. **Wave 4**: Verify all tests pass (checkpoint)

## Key Decisions

### Locked Decisions

From RESEARCH.md and ROADMAP.md, these decisions are non-negotiable:

1. **Keep query.rs wrappers** - Public functions in query.rs remain as wrappers calling _impl() functions. Tests use these wrappers and must not be modified.

2. **Verbatim code migration** - All SQL query logic moves verbatim. No "cleanup" or "improvement" during migration.

3. **db_path field required** - SqliteBackend needs `db_path: PathBuf` for magellan shell-out in ast/find_ast methods.

4. **algorithm.rs stays** - `apply_algorithm_filters()` is higher-level composition, remains in algorithm.rs.

5. **No test modifications** - All 342 existing tests must pass without modification. This proves zero breaking changes.

### Claude's Discretion

- Exact implementation of `_impl()` function signatures (subject to compilation requirements)
- Import organization in sqlite.rs
- Error message wording within existing patterns

## Dependencies

- **Phase 17**: Backend trait abstraction and SqliteBackend stub must exist
- **All prior phases**: Existing query.rs code structure is the source for migration

## Wave Structure

| Wave | Plans | Purpose |
|------|-------|---------|
| 1 | 18-01 | Add db_path field, migrate helper functions |
| 2 | 18-02, 18-03, 18-04 | Create _impl() functions for all three search methods |
| 3 | 18-05 | Implement trait methods calling _impl() functions |
| 4 | 18-06 | Verify tests pass (checkpoint) |

## Success Criteria

1. SqliteBackend implements all 5 BackendTrait methods
2. All 342 existing tests pass without modification
3. Output is identical to pre-refactor implementation
4. No SQL query logic changes (verbatim migration)
5. query.rs public functions still work as wrappers

## Risk Mitigation

- **Pitfall**: Modifying SQL queries during migration
  - **Mitigation**: Treat SQL as read-only, copy verbatim

- **Pitfall**: Breaking test compatibility
  - **Mitigation**: Keep query.rs wrappers unchanged

- **Pitfall**: Forgetting db_path field
  - **Mitigation**: Added in Wave 1, verified in Wave 3

## Next Steps

Execute: `/gsd:execute-phase 18`
