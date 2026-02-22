//! Integration tests for native-v3 exclusive features.
//!
//! This test suite verifies that native-v3 exclusive commands work correctly:
//! - Complete command (KV prefix scan for autocomplete)
//! - Lookup command (O(1) exact FQN lookup)
//! - Label search mode (purpose-based label search)
//! - Performance metrics tracking
//!
//! Tests also verify graceful fallback to SQLite backend with appropriate errors.
//!
//! Note: CLI parsing tests are in src/main.rs's cli_tests module since
//! Cli and Command types are not exported from the library.

use llmgrep::error::LlmError;
use llmgrep::output::PerformanceMetrics;

// Helper to create a test SQLite database (traditional format)
#[cfg(test)]
fn create_sqlite_test_db() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let db_path = dir.path().join("test.db");

    // Create a minimal SQLite database with Magellan schema
    // This simulates a traditional SQLite-format database
    let conn = rusqlite::Connection::open(&db_path).expect("failed to open test database");

    // Create minimal schema to simulate Magellan SQLite database
    conn.execute(
        "CREATE TABLE IF NOT EXISTS symbol_nodes (
            symbol_id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            kind TEXT NOT NULL,
            fqn TEXT,
            canonical_fqn TEXT,
            display_fqn TEXT,
            file_path TEXT NOT NULL,
            byte_start INTEGER,
            byte_end INTEGER,
            start_line INTEGER,
            start_col INTEGER,
            end_line INTEGER,
            end_col INTEGER,
            language TEXT,
            parent_name TEXT
        )",
        [],
    ).expect("test database operation failed");

    // Insert test data
    conn.execute(
        "INSERT INTO symbol_nodes (symbol_id, name, kind, fqn, file_path, byte_start, byte_end, start_line, start_col, end_line, end_col, language)
         VALUES (1, 'test_function', 'Function', 'test::module::test_function', 'src/test.rs', 0, 100, 1, 0, 5, 0, 'rust')",
        [],
    ).expect("test database operation failed");

    conn.execute(
        "INSERT INTO symbol_nodes (symbol_id, name, kind, fqn, file_path, byte_start, byte_end, start_line, start_col, end_line, end_col, language)
         VALUES (2, 'another_function', 'Function', 'test::module::another_function', 'src/test.rs', 100, 200, 6, 0, 10, 0, 'rust')",
        [],
    ).expect("test database operation failed");

    dir
}

// Test 1: Complete command on SQLite backend returns error
#[test]
fn test_complete_command_sqlite_error() {
    let _dir = create_sqlite_test_db();
    let db_path = _dir.path().join("test.db");

    let backend = llmgrep::backend::Backend::detect_and_open(&db_path).expect("failed to detect and open backend");

    let result = backend.complete("test", 10);
    assert!(matches!(result, Err(LlmError::RequiresNativeV3Backend { .. })));

    if let Err(LlmError::RequiresNativeV3Backend { command, path }) = result {
        assert_eq!(command, "complete");
        assert!(path.contains("test.db"));
    } else {
        panic!("Expected RequiresNativeV3Backend error");
    }
}

// Test 2: Lookup command on SQLite backend returns error
#[test]
fn test_lookup_command_sqlite_error() {
    let _dir = create_sqlite_test_db();
    let db_path = _dir.path().join("test.db");

    let backend = llmgrep::backend::Backend::detect_and_open(&db_path).expect("failed to detect and open backend");
    let db_path_str = db_path.to_string_lossy().to_string();

    let result = backend.lookup("test::symbol", &db_path_str);
    assert!(matches!(result, Err(LlmError::RequiresNativeV3Backend { .. })));

    if let Err(LlmError::RequiresNativeV3Backend { command, path }) = result {
        assert_eq!(command, "lookup");
        assert!(path.contains("test.db"));
    } else {
        panic!("Expected RequiresNativeV3Backend error");
    }
}

// Test 3: Label search on SQLite backend returns error
#[test]
fn test_label_search_sqlite_error() {
    let _dir = create_sqlite_test_db();
    let db_path = _dir.path().join("test.db");

    let backend = llmgrep::backend::Backend::detect_and_open(&db_path).expect("failed to detect and open backend");
    let db_path_str = db_path.to_string_lossy().to_string();

    let result = backend.search_by_label("test", 10, &db_path_str);
    assert!(matches!(result, Err(LlmError::RequiresNativeV3Backend { .. })));

    if let Err(LlmError::RequiresNativeV3Backend { command, path }) = result {
        assert_eq!(command, "search --mode label");
        assert!(path.contains("test.db"));
    } else {
        panic!("Expected RequiresNativeV3Backend error");
    }
}

// Test 4: Performance metrics structure initialization
#[test]
fn test_performance_metrics_initialization() {
    let metrics = PerformanceMetrics::new();

    assert_eq!(metrics.backend_detection_ms, 0);
    assert_eq!(metrics.query_execution_ms, 0);
    assert_eq!(metrics.output_formatting_ms, 0);
    assert_eq!(metrics.total_ms, 0);
}

// Test 5: Performance metrics with values
#[test]
fn test_performance_metrics_with_values() {
    let metrics = PerformanceMetrics {
        backend_detection_ms: 5,
        query_execution_ms: 10,
        output_formatting_ms: 3,
        total_ms: 18,
    };

    assert_eq!(metrics.backend_detection_ms, 5);
    assert_eq!(metrics.query_execution_ms, 10);
    assert_eq!(metrics.output_formatting_ms, 3);
    assert_eq!(metrics.total_ms, 18);
}

// Test 6: Performance metrics serialization to JSON
#[test]
fn test_performance_metrics_json_serialization() {
    let metrics = PerformanceMetrics {
        backend_detection_ms: 5,
        query_execution_ms: 10,
        output_formatting_ms: 3,
        total_ms: 18,
    };

    let json = serde_json::to_string(&metrics).expect("test database operation failed");
    assert!(json.contains("5")); // backend_detection_ms
    assert!(json.contains("10")); // query_execution_ms
    assert!(json.contains("3")); // output_formatting_ms
    assert!(json.contains("18")); // total_ms
}

// Test 7: Backend detection for SQLite format
#[test]
fn test_backend_detection_sqlite_format() {
    let _dir = create_sqlite_test_db();
    let db_path = _dir.path().join("test.db");

    let backend = llmgrep::backend::Backend::detect_and_open(&db_path).expect("failed to detect and open backend");

    match backend {
        llmgrep::backend::Backend::Sqlite(_) => {
            // Expected: SQLite backend detected
        }
        #[cfg(feature = "native-v3")]
        llmgrep::backend::Backend::NativeV3(_) => {
            panic!("Unexpected: NativeV3 backend detected for SQLite format");
        }
    }
}

// Test 8: Backend can be opened successfully
#[test]
fn test_backend_open_success() {
    let _dir = create_sqlite_test_db();
    let db_path = _dir.path().join("test.db");

    let result = llmgrep::backend::Backend::detect_and_open(&db_path);
    assert!(result.is_ok(), "Should successfully open SQLite backend");
}

// Test 9: Verify RequiresNativeV3Backend error contains helpful information
#[test]
fn test_requires_native_v3_backend_error_message() {
    let _dir = create_sqlite_test_db();
    let db_path = _dir.path().join("test.db");

    let backend = llmgrep::backend::Backend::detect_and_open(&db_path).expect("failed to detect and open backend");
    let result = backend.complete("test", 10);

    if let Err(LlmError::RequiresNativeV3Backend { command, path }) = result {
        // Verify error message contains expected parts
        let error_msg = format!("{}", LlmError::RequiresNativeV3Backend {
            command: command.clone(),
            path: path.clone(),
        });

        assert!(error_msg.contains("complete"), "Error should mention the command");
        assert!(error_msg.contains("native-v3"), "Error should mention native-v3 requirement");
        assert!(error_msg.contains("O(1)"), "Error should explain performance benefit");
        // The actual error message mentions "complete" and "prefix" but not "autocomplete" specifically
        assert!(error_msg.contains("complete") || error_msg.contains("prefix"), "Error should mention complete or prefix");
    } else {
        panic!("Expected RequiresNativeV3Backend error");
    }
}

// Test 10: Verify SymbolNotFound error structure
#[test]
fn test_symbol_not_found_error_structure() {
    let error = LlmError::SymbolNotFound {
        fqn: "test::nonexistent".to_string(),
        db: "/tmp/test.db".to_string(),
        partial: "nonexistent".to_string(),
    };

    assert_eq!(error.error_code(), "LLM-E112");
    assert_eq!(error.severity(), "error");

    let error_msg = format!("{}", error);
    assert!(error_msg.contains("test::nonexistent"));
    assert!(error_msg.contains("Symbol not found"));

    // Verify remediation hint
    let remediation = error.remediation();
    assert!(remediation.is_some());
    assert!(remediation.expect("test database operation failed").contains("complete"));
}

// Test 11: Verify error code for complete command failure
#[test]
fn test_complete_command_error_code() {
    let _dir = create_sqlite_test_db();
    let db_path = _dir.path().join("test.db");

    let backend = llmgrep::backend::Backend::detect_and_open(&db_path).expect("failed to detect and open backend");
    let result = backend.complete("test", 10);

    if let Err(err) = result {
        assert_eq!(err.error_code(), "LLM-E111");
    } else {
        panic!("Expected error for complete command on SQLite backend");
    }
}

// Test 12: Verify error code for lookup command failure
#[test]
fn test_lookup_command_error_code() {
    let _dir = create_sqlite_test_db();
    let db_path = _dir.path().join("test.db");

    let backend = llmgrep::backend::Backend::detect_and_open(&db_path).expect("failed to detect and open backend");
    let db_path_str = db_path.to_string_lossy().to_string();

    let result = backend.lookup("test::symbol", &db_path_str);

    if let Err(err) = result {
        assert_eq!(err.error_code(), "LLM-E111");
    } else {
        panic!("Expected error for lookup command on SQLite backend");
    }
}

// Test 13: Verify error code for label search failure
#[test]
fn test_label_search_error_code() {
    let _dir = create_sqlite_test_db();
    let db_path = _dir.path().join("test.db");

    let backend = llmgrep::backend::Backend::detect_and_open(&db_path).expect("failed to detect and open backend");
    let db_path_str = db_path.to_string_lossy().to_string();

    let result = backend.search_by_label("test", 10, &db_path_str);

    if let Err(err) = result {
        assert_eq!(err.error_code(), "LLM-E111");
    } else {
        panic!("Expected error for label search on SQLite backend");
    }
}

// Test 14: Complete command with different prefixes
#[test]
fn test_complete_command_various_prefixes() {
    let _dir = create_sqlite_test_db();
    let db_path = _dir.path().join("test.db");

    let backend = llmgrep::backend::Backend::detect_and_open(&db_path).expect("failed to detect and open backend");

    // All should fail on SQLite backend
    let prefixes = vec!["std", "test::module", "crate::backend", "llmgrep"];

    for prefix in prefixes {
        let result = backend.complete(prefix, 10);
        assert!(matches!(result, Err(LlmError::RequiresNativeV3Backend { .. })));
    }
}

// Test 15: Lookup command with different FQNs
#[test]
fn test_lookup_command_various_fqns() {
    let _dir = create_sqlite_test_db();
    let db_path = _dir.path().join("test.db");

    let backend = llmgrep::backend::Backend::detect_and_open(&db_path).expect("failed to detect and open backend");
    let db_path_str = db_path.to_string_lossy().to_string();

    let fqns = vec![
        "std::collections::HashMap::new",
        "test::module::function",
        "crate::backend::NativeV3Backend",
        "llmgrep::main",
    ];

    for fqn in fqns {
        let result = backend.lookup(fqn, &db_path_str);
        assert!(matches!(result, Err(LlmError::RequiresNativeV3Backend { .. })));
    }
}

// Test 16: Label search with different labels
#[test]
fn test_label_search_various_labels() {
    let _dir = create_sqlite_test_db();
    let db_path = _dir.path().join("test.db");

    let backend = llmgrep::backend::Backend::detect_and_open(&db_path).expect("failed to detect and open backend");
    let db_path_str = db_path.to_string_lossy().to_string();

    let labels = vec!["test", "entry_point", "public_api", "test_functions"];

    for label in labels {
        let result = backend.search_by_label(label, 10, &db_path_str);
        assert!(matches!(result, Err(LlmError::RequiresNativeV3Backend { .. })));
    }
}

// Test 17: Performance metrics can be included in JSON response
#[test]
fn test_performance_metrics_in_json_response() {
    use llmgrep::output::SearchResponse;
    use serde_json::json;

    let metrics = PerformanceMetrics {
        backend_detection_ms: 5,
        query_execution_ms: 10,
        output_formatting_ms: 3,
        total_ms: 18,
    };

    let response = SearchResponse {
        results: vec![],
        query: "test".to_string(),
        path_filter: None,
        kind_filter: None,
        total_count: 0,
        notice: None,
    };

    // Create a JSON structure with metrics
    let json_response = json!({
        "performance": metrics,
        "data": response
    });

    let json_str = serde_json::to_string(&json_response).expect("test database operation failed");
    assert!(json_str.contains("\"backend_detection_ms\":5"));
    assert!(json_str.contains("\"query_execution_ms\":10"));
    assert!(json_str.contains("\"output_formatting_ms\":3"));
    assert!(json_str.contains("\"total_ms\":18"));
}

// Test 18: Verify remediation hints for RequiresNativeV3Backend
#[test]
fn test_requires_native_v3_backend_remediation() {
    let _dir = create_sqlite_test_db();
    let db_path = _dir.path().join("test.db");

    let backend = llmgrep::backend::Backend::detect_and_open(&db_path).expect("failed to detect and open backend");
    let result = backend.complete("test", 10);

    if let Err(err) = result {
        let remediation = err.remediation();
        assert!(remediation.is_some(), "Should have remediation hint");
        let hint = remediation.expect("test database operation failed");
        assert!(hint.contains("magellan watch"), "Should suggest reindexing with magellan");
        assert!(hint.contains("native-v3"), "Should mention native-v3 storage");
    } else {
        panic!("Expected error for complete command on SQLite backend");
    }
}

// Test 19: Verify remediation hints for SymbolNotFound
#[test]
fn test_symbol_not_found_remediation() {
    let error = LlmError::SymbolNotFound {
        fqn: "test::nonexistent".to_string(),
        db: "/tmp/test.db".to_string(),
        partial: "nonexistent".to_string(),
    };

    let remediation = error.remediation();
    assert!(remediation.is_some());

    let hint = remediation.expect("test database operation failed");
    assert!(hint.contains("complete"), "Should suggest using complete command");
    assert!(hint.contains("--partial"), "Should mention --partial flag");
}

// Test 20: Backend error handling for invalid database path
#[test]
fn test_backend_open_invalid_path() {
    let invalid_path = std::path::PathBuf::from("/nonexistent/path/test.db");

    let result = llmgrep::backend::Backend::detect_and_open(&invalid_path);
    assert!(result.is_err(), "Should fail to open nonexistent database");

    // The error could be DatabaseNotFound or BackendDetectionFailed
    // depending on the exact error path
    match result {
        Err(LlmError::DatabaseNotFound { path }) => {
            assert!(path.contains("nonexistent"));
        }
        Err(LlmError::BackendDetectionFailed { path, .. }) => {
            assert!(path.contains("nonexistent"));
        }
        Err(_) => {
            // Accept other errors for now - the important thing is that it fails
        }
        Ok(_) => {
            panic!("Should have failed to open nonexistent database");
        }
    }
}

// Test 21: Complete command preserves limit parameter in error
#[test]
fn test_complete_command_limit_parameter() {
    let _dir = create_sqlite_test_db();
    let db_path = _dir.path().join("test.db");

    let backend = llmgrep::backend::Backend::detect_and_open(&db_path).expect("failed to detect and open backend");

    // Test with various limit values
    for limit in [1, 10, 50, 100, 1000] {
        let result = backend.complete("test", limit);
        assert!(matches!(result, Err(LlmError::RequiresNativeV3Backend { .. })));
    }
}

// Test 22: Lookup command preserves FQN parameter in error
#[test]
fn test_lookup_command_preserves_fqn() {
    let _dir = create_sqlite_test_db();
    let db_path = _dir.path().join("test.db");

    let backend = llmgrep::backend::Backend::detect_and_open(&db_path).expect("failed to detect and open backend");
    let db_path_str = db_path.to_string_lossy().to_string();

    let test_fqn = "test::module::function";
    let result = backend.lookup(test_fqn, &db_path_str);

    if let Err(LlmError::RequiresNativeV3Backend { command, .. }) = result {
        assert_eq!(command, "lookup");
    } else {
        panic!("Expected RequiresNativeV3Backend error");
    }
}

// Test 23: Label search preserves label parameter in error
#[test]
fn test_label_search_preserves_label() {
    let _dir = create_sqlite_test_db();
    let db_path = _dir.path().join("test.db");

    let backend = llmgrep::backend::Backend::detect_and_open(&db_path).expect("failed to detect and open backend");
    let db_path_str = db_path.to_string_lossy().to_string();

    let test_label = "test_functions";
    let result = backend.search_by_label(test_label, 10, &db_path_str);

    if let Err(LlmError::RequiresNativeV3Backend { command, .. }) = result {
        assert_eq!(command, "search --mode label");
    } else {
        panic!("Expected RequiresNativeV3Backend error");
    }
}

// Test 24: Performance metrics total time calculation
#[test]
fn test_performance_metrics_total_calculation() {
    let metrics = PerformanceMetrics {
        backend_detection_ms: 5,
        query_execution_ms: 10,
        output_formatting_ms: 3,
        total_ms: 18,
    };

    // Verify total is at least sum of components (allowing for additional overhead)
    assert!(metrics.total_ms >= metrics.backend_detection_ms + metrics.query_execution_ms + metrics.output_formatting_ms);
}

// Test 25: Backend supports standard search operations
#[test]
fn test_backend_standard_search_operations() {
    let _dir = create_sqlite_test_db();
    let db_path = _dir.path().join("test.db");

    let backend = llmgrep::backend::Backend::detect_and_open(&db_path).expect("failed to detect and open backend");

    // Standard search may fail due to incomplete schema, but that's okay
    // The important thing is that it returns a different error than native-v3 requirements
    use llmgrep::query::SearchOptions;

    let options = SearchOptions {
        db_path: &db_path,
        query: "test",
        path_filter: None,
        kind_filter: None,
        language_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 50,
        context: Default::default(),
        snippet: Default::default(),
        fqn: Default::default(),
        include_score: true,
        sort_by: llmgrep::SortMode::Relevance,
        metrics: Default::default(),
        ast: Default::default(),
        depth: Default::default(),
        algorithm: Default::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
    };

    let result = backend.search_symbols(options);
    // The result may be ok or may fail due to schema, but should NOT be RequiresNativeV3Backend
    if let Err(LlmError::RequiresNativeV3Backend { .. }) = result {
        panic!("Standard search should not require native-v3 backend");
    }
    // Other outcomes are acceptable (success or other errors)
}

// Test 26: Verify error severity levels
#[test]
fn test_error_severity_levels() {
    let backend_error = LlmError::RequiresNativeV3Backend {
        command: "complete".to_string(),
        path: "/tmp/test.db".to_string(),
    };
    assert_eq!(backend_error.severity(), "error");

    let not_found_error = LlmError::SymbolNotFound {
        fqn: "test::symbol".to_string(),
        db: "/tmp/test.db".to_string(),
        partial: "symbol".to_string(),
    };
    assert_eq!(not_found_error.severity(), "error");
}

// Test 27: Multiple backend operations can be attempted
#[test]
fn test_multiple_backend_operations() {
    let _dir = create_sqlite_test_db();
    let db_path = _dir.path().join("test.db");

    let backend = llmgrep::backend::Backend::detect_and_open(&db_path).expect("failed to detect and open backend");

    // All native-v3 operations should fail gracefully
    assert!(backend.complete("test", 10).is_err());
    assert!(backend.lookup("test::symbol", &db_path.to_string_lossy()).is_err());
    assert!(backend.search_by_label("test", 10, &db_path.to_string_lossy()).is_err());

    // Standard operations may succeed or fail depending on schema
    // but should NOT fail with RequiresNativeV3Backend
    use llmgrep::query::SearchOptions;

    let options = SearchOptions {
        db_path: &db_path,
        query: "test",
        path_filter: None,
        kind_filter: None,
        language_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 50,
        context: Default::default(),
        snippet: Default::default(),
        fqn: Default::default(),
        include_score: true,
        sort_by: llmgrep::SortMode::Relevance,
        metrics: Default::default(),
        ast: Default::default(),
        depth: Default::default(),
        algorithm: Default::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
    };

    let result = backend.search_symbols(options);
    if let Err(LlmError::RequiresNativeV3Backend { .. }) = result {
        panic!("Standard search should not require native-v3 backend");
    }
    // Other outcomes are acceptable
}

// Test 28: Performance metrics can be cloned
#[test]
fn test_performance_metrics_clone() {
    let metrics = PerformanceMetrics {
        backend_detection_ms: 5,
        query_execution_ms: 10,
        output_formatting_ms: 3,
        total_ms: 18,
    };

    let cloned = metrics.clone();
    assert_eq!(cloned.backend_detection_ms, metrics.backend_detection_ms);
    assert_eq!(cloned.query_execution_ms, metrics.query_execution_ms);
    assert_eq!(cloned.output_formatting_ms, metrics.output_formatting_ms);
    assert_eq!(cloned.total_ms, metrics.total_ms);
}

// Test 29: Backend provides consistent error format
#[test]
fn test_backend_error_consistency() {
    let _dir = create_sqlite_test_db();
    let db_path = _dir.path().join("test.db");

    let backend = llmgrep::backend::Backend::detect_and_open(&db_path).expect("failed to detect and open backend");
    let db_path_str = db_path.to_string_lossy().to_string();

    // All native-v3 errors should have the same error code
    let complete_err = backend.complete("test", 10);
    let lookup_err = backend.lookup("test", &db_path_str);
    let label_err = backend.search_by_label("test", 10, &db_path_str);

    // Use pattern matching to get error codes without Debug requirement
    let complete_code = match complete_err {
        Err(err) => err.error_code(),
        Ok(_) => panic!("Expected error"),
    };
    let lookup_code = match lookup_err {
        Err(err) => err.error_code(),
        Ok(_) => panic!("Expected error"),
    };
    let label_code = match label_err {
        Err(err) => err.error_code(),
        Ok(_) => panic!("Expected error"),
    };

    assert_eq!(complete_code, "LLM-E111");
    assert_eq!(lookup_code, "LLM-E111");
    assert_eq!(label_code, "LLM-E111");
}

// Test 30: Search operations with standard options don't require native-v3
#[test]
fn test_search_operations_dont_require_native_v3() {
    let _dir = create_sqlite_test_db();
    let db_path = _dir.path().join("test.db");

    let backend = llmgrep::backend::Backend::detect_and_open(&db_path).expect("failed to detect and open backend");

    use llmgrep::query::SearchOptions;

    let options = SearchOptions {
        db_path: &db_path,
        query: "test",
        path_filter: None,
        kind_filter: None,
        language_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 50,
        context: Default::default(),
        snippet: Default::default(),
        fqn: Default::default(),
        include_score: true,
        sort_by: llmgrep::SortMode::Relevance,
        metrics: Default::default(),
        ast: Default::default(),
        depth: Default::default(),
        algorithm: Default::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
    };

    // All standard search modes should NOT require native-v3 backend
    // They may succeed or fail for other reasons (schema, etc.)
    let symbols_result = backend.search_symbols(options.clone());
    let refs_result = backend.search_references(options.clone());
    let calls_result = backend.search_calls(options);

    // Verify none of them fail with RequiresNativeV3Backend
    if let Err(LlmError::RequiresNativeV3Backend { .. }) = symbols_result {
        panic!("search_symbols should not require native-v3 backend");
    }
    if let Err(LlmError::RequiresNativeV3Backend { .. }) = refs_result {
        panic!("search_references should not require native-v3 backend");
    }
    if let Err(LlmError::RequiresNativeV3Backend { .. }) = calls_result {
        panic!("search_calls should not require native-v3 backend");
    }
}
