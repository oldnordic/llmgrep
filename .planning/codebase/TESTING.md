# Testing Patterns

**Analysis Date:** 2026-02-10

## Test Framework

**Runner:**
- Built-in Rust `cargo test`
- No external test framework dependency detected
- Config: `Cargo.toml` with standard `dev-dependencies`

**Assertion Library:**
- Standard `assert!`, `assert_eq!`, `assert!()` macros
- Custom match patterns for complex assertions

**Run Commands:**
```bash
cargo test                       # Run all tests
cargo test --test integration_tests  # Run specific test file
cargo test --test integration_tests test_utf8_emoji_extraction  # Run specific test
cargo test -- --nocapture        # Show println output
```

## Test File Organization

**Location:**
- Integration tests in `tests/` directory (project root)
- Unit tests in `src/` files (inline `#[cfg(test)]` modules)

**Naming:**
- Integration tests: `{feature}_tests.rs` (e.g., `integration_tests.rs`, `search_tests.rs`)
- Unit test modules: `mod tests` within source files

**Structure:**
```
tests/
├── integration_tests.rs     # Feature integration tests (v1.1 features)
├── search_tests.rs          # Search functionality tests
├── unit_tests.rs            # Internal logic unit tests
├── ast_tests.rs             # AST query tests
├── algorithm_tests.rs       # Algorithm integration tests
├── shorthand_tests.rs       # AST shorthand expansion tests
├── cli_integration_test.rs  # CLI parsing tests
├── backend_detection_test.rs # Backend detection tests
├── backend_parity_test.rs   # SQLite vs Native-V2 parity
└── native_v2_commands_test.rs # Native-V2 feature tests
```

## Test Structure

**Suite Organization:**
```rust
/// Integration tests for v1.1 features
///
/// Tests for:
/// - UTF-8 emoji extraction
/// - CJK character handling
/// - Chunk retrieval
/// - Metrics filtering
/// - SymbolId lookup
use llmgrep::query::{search_symbols, ...};

fn setup_db(path: &std::path::Path) -> Connection {
    // Database setup
}

#[test]
fn test_utf8_emoji_extraction() {
    // Arrange
    let temp_dir = tempfile::TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db(&db_path);
    // ... insert test data ...

    // Act
    let options = SearchOptions { ... };
    let response = search_symbols(options).expect("search should succeed");

    // Assert
    assert_eq!(response.0.results.len(), 1);
    assert!(response.0.results[0].snippet.is_some());
}
```

**Patterns:**
- `setup_db()` helper creates test database schema
- `insert_file()`, `insert_symbol()` helpers for data insertion
- `tempfile::TempDir` for temporary database files
- `.expect()` for test failures with context messages

**Setup pattern:**
```rust
fn setup_db(path: &std::path::Path) -> Connection {
    let conn = Connection::open(path).expect("open db");
    conn.execute_batch(
        "CREATE TABLE graph_entities (...);
         CREATE TABLE graph_edges (...);
         CREATE TABLE code_chunks (...);
         CREATE TABLE symbol_metrics (...);"
    ).expect("create tables");
    conn
}
```

**Teardown pattern:**
- Implicit - TempDir auto-cleans on drop
- No explicit teardown functions

**Assertion pattern:**
```rust
// Equality assertions
assert_eq!(response.0.results.len(), 1);
assert_eq!(response.0.results[0].name, "expected_name");

// Boolean assertions with messages
assert!(result.is_some(), "Chunk should exist");
assert!(hash.chars().all(|c| c.is_ascii_hexdigit()), "Hash should be valid hex");

// Optional unwrapping with assertions
let result = &response.0.results[0];
assert!(result.fqn.is_some(), "fqn should be populated");
```

## Mocking

**Framework:** No mocking framework used
- Direct in-memory SQLite databases for testing
- Real `rusqlite::Connection` with temporary files

**Patterns:**
```rust
// Create real but temporary database
let temp_dir = tempfile::TempDir::new().expect("tempdir");
let db_path = temp_dir.path().join("test.db");
let conn = Connection::open(&db_path).expect("open db");

// Insert test data directly
conn.execute(
    "INSERT INTO graph_entities (kind, name, data) VALUES (?1, ?2, ?3)",
    params!["Symbol", "test_func", data],
).expect("insert symbol");
```

**What to Mock:**
- Database connections: Use temp SQLite DB instead of mocking
- File I/O: Use TempDir for test files
- External processes: Magellan CLI calls in `algorithm.rs` are NOT mocked (real shell-out)

**What NOT to Mock:**
- Database queries (use real SQLite)
- Error types (use real `LlmError`)
- Data structures (use real structs)

## Fixtures and Factories

**Test Data:**
```rust
// Helper functions create test data
fn insert_file(conn: &Connection, path: &str) -> i64 {
    let data = json!({
        "path": path,
        "hash": "sha256:deadbeef",
        "last_indexed_at": 0,
        "last_modified": 0
    }).to_string();
    conn.execute("INSERT INTO graph_entities ...", params![...])
        .expect("insert file");
    conn.last_insert_rowid()
}

fn insert_symbol(conn: &Connection, name: &str, kind: &str, ...) -> i64 {
    // Creates symbol with JSON data
}

fn insert_metrics(conn: &Connection, symbol_row_id: i64, ...) {
    // Inserts symbol_metrics row
}
```

**Location:**
- Fixture helpers defined in each test file
- No shared fixture directory
- Each test file has its own `setup_db` and insert helpers

## Coverage

**Requirements:** No enforced coverage requirements
- No coverage tool configured (no tarpaulin, cargo-llvm-cov)

**View Coverage:**
- Not applicable - no coverage reporting setup

## Test Types

**Unit Tests:**
- Located in `src/` files within `#[cfg(test)]` modules
- Example: `src/algorithm.rs` has ~60 lines of unit tests
- Test internal functions and data validation

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_set_validation_valid() {
        let symbol_set = SymbolSet {
            symbol_ids: vec!["abc123def456789012345678901234ab".to_string()],
        };
        assert!(symbol_set.validate().is_ok());
        assert_eq!(symbol_set.len(), 1);
        assert!(!symbol_set.is_empty());
    }
}
```

**Integration Tests:**
- Located in `tests/` directory
- Test full query pipeline with real database
- Test search with various options

**E2E Tests:**
- `tests/cli_integration_test.rs` - Tests CLI argument parsing
- `tests/backend_parity_test.rs` - Cross-backend consistency
- No subprocess/e2e execution testing

## Common Patterns

**Async Testing:**
- Not applicable - synchronous codebase

**Error Testing:**
```rust
// Test error conditions
#[test]
fn test_invalid_flag() {
    let args = ["llmgrep", "--invalid-flag", "search", "--query", "test"];
    let result = Cli::try_parse_from(args);
    assert!(result.is_err(), "Should reject unknown flag");
}

#[test]
fn test_path_validation_sensitive_etc() {
    let path = Path::new("/etc/passwd");
    let result = validate_path(path, true);
    assert!(result.is_err(), "Should reject /etc/passwd");
    match result {
        Err(LlmError::PathValidationFailed { reason, .. }) => {
            assert!(reason.contains("not allowed"));
        }
        _ => panic!("Expected PathValidationFailed error"),
    }
}
```

**Option Testing:**
```rust
#[test]
fn test_search_symbols_with_fqn_toggle() {
    // Test with fqn fields disabled
    let options = SearchOptions {
        fqn: FqnOptions { fqn: false, canonical_fqn: false, display_fqn: false },
        ...
    };
    let response = search_symbols(options).expect("search");
    assert!(result.fqn.is_none());

    // Test with fqn fields enabled
    let options = SearchOptions {
        fqn: FqnOptions { fqn: true, canonical_fqn: true, display_fqn: true },
        ...
    };
    let response = search_symbols(options).expect("search");
    assert!(result.fqn.is_some() || result.display_fqn.is_some());
}
```

**Database Testing Pattern:**
```rust
#[test]
fn test_search_symbols_with_path_filter() {
    let temp_dir = tempfile::TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db(&db_path);

    // Insert test data
    let file_id = insert_file(&conn, "src/main.rs");
    let symbol_id = insert_symbol(&conn, "main", "Function", "fn", (0, 4));
    insert_define_edge(&conn, file_id, symbol_id);

    // Run search
    let options = SearchOptions {
        db_path: &db_path,
        path_filter: Some(&PathBuf::from("src/")),
        ...
    };
    let response = search_symbols(options).expect("search");

    // Assertions
    assert_eq!(response.0.results.len(), 1);
    assert_eq!(response.0.results[0].span.file_path, "src/main.rs");
}
```

## Test Data Creation

**JSON helpers:**
```rust
use serde_json::json;

fn insert_symbol(...) -> i64 {
    let data = json!({
        "symbol_id": format!("{}-id", name),
        "name": name,
        "kind": kind,
        "kind_normalized": kind_normalized,
        "fqn": format!("test::{}", name),
        ...
    }).to_string();
    conn.execute("INSERT INTO graph_entities ...", params![...])
}
```

**Temp files:**
```rust
let file_path = temp_dir.path().join("sample.rs");
std::fs::write(&file_path, "fn hello() {}\nfn world() {}\n").expect("write file");
```

## CLI Testing Pattern

```rust
#[test]
fn test_basic_search_command() {
    let temp_db = create_temp_db().expect("Failed to create temp db");
    let args = [
        "llmgrep",
        "--db",
        temp_db.to_str().unwrap(),
        "search",
        "--query",
        "test",
    ];
    let result = Cli::try_parse_from(args);
    assert!(result.is_ok(), "Should parse basic search command");

    let cli = result.unwrap();
    match cli.command {
        Command::Search { query, .. } => {
            assert_eq!(query, "test");
        }
        _ => panic!("Expected Command::Search"),
    }
}
```

## UTF-8 and Internationalization Testing

**Special focus on multi-byte character handling:**
```rust
#[test]
fn test_utf8_emoji_extraction() {
    // Test with emoji (4-byte UTF-8 sequences)
    let content = "fn test() { // 🚀🔥⭐\n  true\n}";

    // Verify emoji is preserved
    assert!(result.contains("🚀"));
    assert!(result.contains("🔥"));
}

#[test]
fn test_cjk_character_extraction() {
    // Test CJK characters (Chinese, Japanese, Korean)
    let cjk_content = "// 中文注释\n// 日本語コメント\n// 한국어 주석";

    assert!(chunk.content.contains("中文"));
    assert!(chunk.content.contains("日本語"));
    assert!(chunk.content.contains("한국어"));
}
```

---

*Testing analysis: 2026-02-10*
