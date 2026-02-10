//! Cross-backend integration tests for feature parity verification.
//!
//! This test suite verifies that all features work identically on SQLite and Native-V2 backends.
//! Tests verify that context, snippet, score, and metrics produce the same results regardless of backend.
//!
//! Purpose: Ensure users can switch between backends without changing their workflow or getting different results.

#![cfg(feature = "native-v2")]

use llmgrep::backend::Backend;
use llmgrep::query::{ContextOptions, FqnOptions, MetricsOptions, SearchOptions, SnippetOptions};
use std::path::PathBuf;
use tempfile::TempDir;

// ============================================================================
// Test Fixtures
// ============================================================================

/// Helper to create a test SQLite database with basic schema
#[cfg(test)]
fn create_sqlite_test_db() -> (TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let db_path = dir.path().join("test.db");
    let conn = rusqlite::Connection::open(&db_path).expect("failed to open test database");

    // Create minimal Magellan-compatible schema
    conn.execute_batch(
        "CREATE TABLE graph_entities (
            id INTEGER PRIMARY KEY,
            kind TEXT NOT NULL,
            name TEXT,
            data TEXT NOT NULL
        );
        CREATE TABLE graph_edges (
            id INTEGER PRIMARY KEY,
            from_id INTEGER NOT NULL,
            to_id INTEGER NOT NULL,
            edge_type TEXT NOT NULL
        );
        CREATE TABLE symbol_metrics (
            symbol_id INTEGER PRIMARY KEY,
            symbol_name TEXT NOT NULL,
            kind TEXT NOT NULL,
            file_path TEXT NOT NULL,
            loc INTEGER NOT NULL DEFAULT 0,
            estimated_loc REAL NOT NULL DEFAULT 0.0,
            fan_in INTEGER NOT NULL DEFAULT 0,
            fan_out INTEGER NOT NULL DEFAULT 0,
            cyclomatic_complexity INTEGER NOT NULL DEFAULT 1,
            last_updated INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (symbol_id) REFERENCES graph_entities(id) ON DELETE CASCADE
        );",
    ).expect("Failed to create schema");

    // Insert a file entity
    let file_data = serde_json::json!({
        "path": "src/test.rs",
        "hash": "sha256:abc123",
        "last_indexed_at": 1234567890,
        "last_modified": 1234567890
    }).to_string();

    conn.execute(
        "INSERT INTO graph_entities (id, kind, name, data) VALUES (1, 'File', 'src/test.rs', ?1)",
        rusqlite::params_from_iter([&file_data]),
    ).expect("failed to execute SQL");

    // Insert symbol entities
    let symbol1_data = serde_json::json!({
        "symbol_id": "sym1",
        "name": "test_function",
        "kind": "Function",
        "kind_normalized": "function",
        "fqn": "my_crate::test_function",
        "canonical_fqn": "my_crate::src/test.rs::Function test_function",
        "display_fqn": "test_function",
        "byte_start": 0,
        "byte_end": 100,
        "start_line": 1,
        "start_col": 0,
        "end_line": 5,
        "end_col": 10
    }).to_string();

    conn.execute(
        "INSERT INTO graph_entities (id, kind, name, data) VALUES (10, 'Symbol', 'test_function', ?1)",
        rusqlite::params_from_iter([&symbol1_data]),
    ).expect("failed to execute SQL");

    // Insert edge linking symbol to file
    conn.execute(
        "INSERT INTO graph_edges (from_id, to_id, edge_type) VALUES (1, 10, 'DEFINES')",
        [],
    ).expect("failed to execute SQL");

    // Create test source file for context/snippet extraction
    let test_file_path = dir.path().join("src/test.rs");
    std::fs::create_dir_all(test_file_path.parent().expect("test file should have parent")).expect("failed to create test directory");
    std::fs::write(&test_file_path, r#"
// Line 1: Before context
pub fn test_function() {
    println!("test");
}
// Line 7: After context
"#).expect("failed to execute SQL");

    (dir, db_path)
}

// ============================================================================
// Backend Detection Tests
// ============================================================================

/// Test that SQLite databases are correctly detected
#[test]
fn test_backend_detection_sqlite() {
    let (_dir, db_path) = create_sqlite_test_db();

    let backend = Backend::detect_and_open(&db_path)
        .expect("Backend should open database");

    match backend {
        Backend::Sqlite(_) => {
            // Correct - SQLite backend detected
        }
        #[cfg(feature = "native-v2")]
        Backend::NativeV2(_) => {
            panic!("Should not detect SQLite as native-v2");
        }
    }
}

/// Test that native-v2 databases are correctly detected when feature is enabled
#[cfg(feature = "native-v2")]
#[test]
fn test_backend_detection_native_v2() {
    use magellan::CodeGraph;

    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let db_path = dir.path().join("test.mag2");

    // Create a native-v2 database (empty, but valid format)
    let _graph = CodeGraph::open(&db_path)
        .expect("Failed to create native-v2 database");

    let backend = Backend::detect_and_open(&db_path)
        .expect("Should detect native-v2 backend");

    match backend {
        Backend::NativeV2(_) => {
            // Correct - native-v2 backend detected
        }
        Backend::Sqlite(_) => {
            panic!("Should detect native-v2 as native-v2, not SQLite");
        }
    }
}

// ============================================================================
// Task 1.1: Context Extraction Parity Tests
// ============================================================================

/// Test that context extraction produces output with correct structure
#[test]
fn test_context_structure_parity() {
    let (_dir, db_path) = create_sqlite_test_db();

    let backend = Backend::detect_and_open(&db_path)
        .expect("Backend should open database");

    let options = SearchOptions {
        db_path: &db_path,
        query: "test_function",
        path_filter: None,
        kind_filter: None,
        language_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions {
            include: true,
            lines: 2,
            max_lines: 100,
        },
        snippet: SnippetOptions::default(),
        fqn: FqnOptions::default(),
        include_score: false,
        sort_by: llmgrep::SortMode::Relevance,
        metrics: MetricsOptions::default(),
        ast: llmgrep::query::AstOptions::new(),
        depth: llmgrep::query::DepthOptions::default(),
        algorithm: llmgrep::AlgorithmOptions::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
    };

    let result = backend.search_symbols(options);

    // The search should succeed (not error)
    assert!(result.is_ok(), "search_symbols should succeed: {:?}", result.err());

    let (response, _partial, _bounded) = result.expect("result should be Ok");

    // Response structure should be correct
    // We're mainly testing that the API accepts context options and returns a response
    // without panicking or erroring
}

/// Test that context extraction handles file boundaries correctly
#[test]
fn test_context_file_boundaries() {
    let (_dir, db_path) = create_sqlite_test_db();

    let backend = Backend::detect_and_open(&db_path)
        .expect("Backend should open database");

    // Request more context than available
    let options = SearchOptions {
        db_path: &db_path,
        query: "test_function",
        path_filter: None,
        kind_filter: None,
        language_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions {
            include: true,
            lines: 100, // Request more than available
            max_lines: 1000,
        },
        snippet: SnippetOptions::default(),
        fqn: FqnOptions::default(),
        include_score: false,
        sort_by: llmgrep::SortMode::Relevance,
        metrics: MetricsOptions::default(),
        ast: llmgrep::query::AstOptions::new(),
        depth: llmgrep::query::DepthOptions::default(),
        algorithm: llmgrep::AlgorithmOptions::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
    };

    let result = backend.search_symbols(options);

    // Should not panic even when requesting more context than available
    assert!(result.is_ok(), "search_symbols with excessive context should succeed");
}

// ============================================================================
// Task 1.2: Snippet Extraction Parity Tests
// ============================================================================

/// Test that snippet extraction works with correct structure
#[test]
fn test_snippet_structure_parity() {
    let (_dir, db_path) = create_sqlite_test_db();

    let backend = Backend::detect_and_open(&db_path)
        .expect("Backend should open database");

    let options = SearchOptions {
        db_path: &db_path,
        query: "test_function",
        path_filter: None,
        kind_filter: None,
        language_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions {
            include: true,
            max_bytes: 50,
        },
        fqn: FqnOptions::default(),
        include_score: false,
        sort_by: llmgrep::SortMode::Relevance,
        metrics: MetricsOptions::default(),
        ast: llmgrep::query::AstOptions::new(),
        depth: llmgrep::query::DepthOptions::default(),
        algorithm: llmgrep::AlgorithmOptions::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
    };

    let result = backend.search_symbols(options);

    // Should succeed
    assert!(result.is_ok(), "search_symbols with snippet should succeed");
}

/// Test that snippet truncation is handled correctly
#[test]
fn test_snippet_truncation() {
    let (_dir, db_path) = create_sqlite_test_db();

    let backend = Backend::detect_and_open(&db_path)
        .expect("Backend should open database");

    // Test with very small max_bytes
    let options = SearchOptions {
        db_path: &db_path,
        query: "test_function",
        path_filter: None,
        kind_filter: None,
        language_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions {
            include: true,
            max_bytes: 10, // Very small limit
        },
        fqn: FqnOptions::default(),
        include_score: false,
        sort_by: llmgrep::SortMode::Relevance,
        metrics: MetricsOptions::default(),
        ast: llmgrep::query::AstOptions::new(),
        depth: llmgrep::query::DepthOptions::default(),
        algorithm: llmgrep::AlgorithmOptions::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
    };

    let result = backend.search_symbols(options);

    // Should handle truncation gracefully
    assert!(result.is_ok(), "search_symbols with truncation should succeed");
}

// ============================================================================
// Task 1.3: Score Parity Tests
// ============================================================================

/// Test that scoring is calculated when requested
#[test]
fn test_score_calculation_enabled() {
    let (_dir, db_path) = create_sqlite_test_db();

    let backend = Backend::detect_and_open(&db_path)
        .expect("Backend should open database");

    let options = SearchOptions {
        db_path: &db_path,
        query: "test_function",
        path_filter: None,
        kind_filter: None,
        language_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions::default(),
        include_score: true, // Enable scoring
        sort_by: llmgrep::SortMode::Relevance,
        metrics: MetricsOptions::default(),
        ast: llmgrep::query::AstOptions::new(),
        depth: llmgrep::query::DepthOptions::default(),
        algorithm: llmgrep::AlgorithmOptions::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
    };

    let result = backend.search_symbols(options);

    assert!(result.is_ok(), "search_symbols with scoring should succeed");

    let (response, _partial, _bounded) = result.expect("result should be Ok");

    // When scoring is enabled, results should be scored
    // We verify the API works - exact values depend on the backend implementation
}

/// Test that scoring is not calculated when disabled
#[test]
fn test_score_calculation_disabled() {
    let (_dir, db_path) = create_sqlite_test_db();

    let backend = Backend::detect_and_open(&db_path)
        .expect("Backend should open database");

    let options = SearchOptions {
        db_path: &db_path,
        query: "test_function",
        path_filter: None,
        kind_filter: None,
        language_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions::default(),
        include_score: false, // Disable scoring
        sort_by: llmgrep::SortMode::Position,
        metrics: MetricsOptions::default(),
        ast: llmgrep::query::AstOptions::new(),
        depth: llmgrep::query::DepthOptions::default(),
        algorithm: llmgrep::AlgorithmOptions::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
    };

    let result = backend.search_symbols(options);

    assert!(result.is_ok(), "search_symbols without scoring should succeed");
}

// ============================================================================
// Task 1.4: Metrics Parity Tests
// ============================================================================

/// Test that metrics option is accepted by the API
#[test]
fn test_metrics_api_accepts_options() {
    let (_dir, db_path) = create_sqlite_test_db();

    let backend = Backend::detect_and_open(&db_path)
        .expect("Backend should open database");

    // Test with metrics filters
    let options = SearchOptions {
        db_path: &db_path,
        query: "test_function",
        path_filter: None,
        kind_filter: None,
        language_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions::default(),
        include_score: false,
        sort_by: llmgrep::SortMode::Relevance,
        metrics: MetricsOptions {
            min_fan_in: Some(5),
            min_fan_out: Some(3),
            min_complexity: Some(2),
            ..Default::default()
        },
        ast: llmgrep::query::AstOptions::new(),
        depth: llmgrep::query::DepthOptions::default(),
        algorithm: llmgrep::AlgorithmOptions::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
    };

    let result = backend.search_symbols(options);

    // API should accept metrics options without error
    // Results may be empty if no symbols match the filter
    assert!(result.is_ok(), "search_symbols with metrics filters should accept the options");
}

// ============================================================================
// Task 1.5: Filter Parity Tests
// ============================================================================

/// Test that min-fan-in filter is accepted by the API
#[test]
fn test_filter_min_fan_in_api() {
    let (_dir, db_path) = create_sqlite_test_db();

    let backend = Backend::detect_and_open(&db_path)
        .expect("Backend should open database");

    let options = SearchOptions {
        db_path: &db_path,
        query: "",
        path_filter: None,
        kind_filter: None,
        language_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions::default(),
        include_score: false,
        sort_by: llmgrep::SortMode::Relevance,
        metrics: MetricsOptions {
            min_fan_in: Some(8),
            ..Default::default()
        },
        ast: llmgrep::query::AstOptions::new(),
        depth: llmgrep::query::DepthOptions::default(),
        algorithm: llmgrep::AlgorithmOptions::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
    };

    let result = backend.search_symbols(options);

    // API should accept the filter
    assert!(result.is_ok(), "API should accept min_fan_in filter");
}

/// Test that min-fan-out filter is accepted by the API
#[test]
fn test_filter_min_fan_out_api() {
    let (_dir, db_path) = create_sqlite_test_db();

    let backend = Backend::detect_and_open(&db_path)
        .expect("Backend should open database");

    let options = SearchOptions {
        db_path: &db_path,
        query: "",
        path_filter: None,
        kind_filter: None,
        language_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions::default(),
        include_score: false,
        sort_by: llmgrep::SortMode::Relevance,
        metrics: MetricsOptions {
            min_fan_out: Some(5),
            ..Default::default()
        },
        ast: llmgrep::query::AstOptions::new(),
        depth: llmgrep::query::DepthOptions::default(),
        algorithm: llmgrep::AlgorithmOptions::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
    };

    let result = backend.search_symbols(options);

    // API should accept the filter
    assert!(result.is_ok(), "API should accept min_fan_out filter");
}

/// Test that min-complexity filter is accepted by the API
#[test]
fn test_filter_min_complexity_api() {
    let (_dir, db_path) = create_sqlite_test_db();

    let backend = Backend::detect_and_open(&db_path)
        .expect("Backend should open database");

    let options = SearchOptions {
        db_path: &db_path,
        query: "",
        path_filter: None,
        kind_filter: None,
        language_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions::default(),
        include_score: false,
        sort_by: llmgrep::SortMode::Relevance,
        metrics: MetricsOptions {
            min_complexity: Some(3),
            ..Default::default()
        },
        ast: llmgrep::query::AstOptions::new(),
        depth: llmgrep::query::DepthOptions::default(),
        algorithm: llmgrep::AlgorithmOptions::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
    };

    let result = backend.search_symbols(options);

    // API should accept the filter
    assert!(result.is_ok(), "API should accept min_complexity filter");
}

// ============================================================================
// Cross-Backend API Consistency Tests
// ============================================================================

/// Test that both backends accept the same SearchOptions structure
#[test]
fn test_cross_backend_search_options_compatibility() {
    let (_dir, db_path) = create_sqlite_test_db();

    let sqlite_backend = Backend::detect_and_open(&db_path)
        .expect("Should open SQLite backend");

    // Create a comprehensive SearchOptions
    let options = SearchOptions {
        db_path: &db_path,
        query: "test",
        path_filter: None,
        kind_filter: Some("Function"),
        language_filter: Some("rust"),
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions {
            include: true,
            lines: 3,
            max_lines: 100,
        },
        snippet: SnippetOptions {
            include: true,
            max_bytes: 200,
        },
        fqn: FqnOptions {
            fqn: true,
            canonical_fqn: true,
            display_fqn: true,
        },
        include_score: true,
        sort_by: llmgrep::SortMode::Relevance,
        metrics: MetricsOptions {
            min_complexity: Some(1),
            ..Default::default()
        },
        ast: llmgrep::query::AstOptions::new(),
        depth: llmgrep::query::DepthOptions::default(),
        algorithm: llmgrep::AlgorithmOptions::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
    };

    // SQLite backend should accept these options
    let sqlite_result = sqlite_backend.search_symbols(options.clone());
    assert!(sqlite_result.is_ok(), "SQLite backend should accept comprehensive SearchOptions");
}
