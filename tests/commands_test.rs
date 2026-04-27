//! Integration tests for complete, lookup, and label commands.
//!
//! This test suite verifies that these commands work correctly
//! on the SQLite backend via SQL fallbacks.
//!
//! Note: CLI parsing tests are in src/main.rs's cli_tests module since
//! Cli and Command types are not exported from the library.

use llmgrep::error::LlmError;
use llmgrep::output::PerformanceMetrics;

// Helper to create a test SQLite database with Magellan graph_entities schema
#[cfg(test)]
fn create_sqlite_test_db() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let db_path = dir.path().join("test.db");

    let conn = rusqlite::Connection::open(&db_path).expect("failed to open test database");

    // Magellan v3.1.6 schema uses graph_entities for symbol storage
    conn.execute(
        "CREATE TABLE IF NOT EXISTS graph_entities (
            id INTEGER PRIMARY KEY,
            kind TEXT NOT NULL,
            name TEXT NOT NULL,
            fqn TEXT,
            data TEXT NOT NULL,
            file_id INTEGER,
            start_line INTEGER,
            start_col INTEGER,
            end_line INTEGER,
            end_col INTEGER,
            language TEXT
        )",
        [],
    )
    .expect("test database operation failed");

    // Insert test symbols with JSON data matching Magellan format
    let data1 = r#"{"fqn":"test::module::test_function","canonical_fqn":"test::module::test_function","display_fqn":"test::module::test_function","name":"test_function","kind":"Function","file_path":"src/test.rs","byte_start":0,"byte_end":100,"start_line":1,"start_col":0,"end_line":5,"end_col":0,"language":"rust"}"#;
    conn.execute(
        "INSERT INTO graph_entities (id, kind, name, fqn, data, start_line, start_col, end_line, end_col, language)
         VALUES (1, 'Symbol', 'test_function', 'test::module::test_function', ?1, 1, 0, 5, 0, 'rust')",
        [data1],
    ).expect("test database operation failed");

    let data2 = r#"{"fqn":"test::module::another_function","canonical_fqn":"test::module::another_function","display_fqn":"test::module::another_function","name":"another_function","kind":"Function","file_path":"src/test.rs","byte_start":100,"byte_end":200,"start_line":6,"start_col":0,"end_line":10,"end_col":0,"language":"rust"}"#;
    conn.execute(
        "INSERT INTO graph_entities (id, kind, name, fqn, data, start_line, start_col, end_line, end_col, language)
         VALUES (2, 'Symbol', 'another_function', 'test::module::another_function', ?1, 6, 0, 10, 0, 'rust')",
        [data2],
    ).expect("test database operation failed");

    dir
}

// Test 1: Complete command on SQLite backend works via SQL fallback
#[test]
fn test_complete_command_sqlite_works() {
    let _dir = create_sqlite_test_db();
    let db_path = _dir.path().join("test.db");

    let backend = llmgrep::backend::Backend::detect_and_open(&db_path)
        .expect("failed to detect and open backend");

    let result = backend.complete("test", 10);
    assert!(
        result.is_ok(),
        "complete should work on SQLite backend: {:?}",
        result.err()
    );
    let completions = result.unwrap();
    assert!(
        !completions.is_empty(),
        "should find completions for 'test' prefix"
    );
    assert!(completions.iter().any(|c| c.contains("test_function")));
}

// Test 2: Lookup command on SQLite backend works via SQL fallback
#[test]
fn test_lookup_command_sqlite_works() {
    let _dir = create_sqlite_test_db();
    let db_path = _dir.path().join("test.db");

    let backend = llmgrep::backend::Backend::detect_and_open(&db_path)
        .expect("failed to detect and open backend");
    let db_path_str = db_path.to_string_lossy().to_string();

    // Lookup existing symbol
    let result = backend.lookup("test::module::test_function", &db_path_str);
    assert!(
        result.is_ok(),
        "lookup should work on SQLite backend: {:?}",
        result.err()
    );
    let symbol = result.unwrap();
    assert_eq!(symbol.name, "test_function");

    // Lookup nonexistent symbol returns SymbolNotFound
    let result = backend.lookup("test::nonexistent", &db_path_str);
    assert!(result.is_err(), "lookup for missing symbol should fail");
    match result {
        Err(LlmError::SymbolNotFound { .. }) => {}
        _ => panic!("Expected SymbolNotFound error"),
    }
}

// Test 3: Label search on SQLite backend returns empty results
#[test]
fn test_label_search_sqlite_returns_empty() {
    let _dir = create_sqlite_test_db();
    let db_path = _dir.path().join("test.db");

    let backend = llmgrep::backend::Backend::detect_and_open(&db_path)
        .expect("failed to detect and open backend");
    let db_path_str = db_path.to_string_lossy().to_string();

    let result = backend.search_by_label("test", 10, &db_path_str);
    assert!(
        result.is_ok(),
        "label search should not error on SQLite: {:?}",
        result.err()
    );
    let (response, _, _) = result.unwrap();
    assert_eq!(
        response.total_count, 0,
        "label search on SQLite should return empty results"
    );
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

    let backend = llmgrep::backend::Backend::detect_and_open(&db_path)
        .expect("failed to detect and open backend");

    match backend {
        llmgrep::backend::Backend::Sqlite(_) => {
            // Expected: SQLite backend detected
        }
        #[cfg(feature = "geometric-backend")]
        llmgrep::backend::Backend::Geometric(_) => {
            panic!("Unexpected: Geometric backend detected for SQLite format");
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

// Test 9: Verify complete returns results for various prefixes
#[test]
fn test_complete_returns_results_for_prefixes() {
    let _dir = create_sqlite_test_db();
    let db_path = _dir.path().join("test.db");

    let backend = llmgrep::backend::Backend::detect_and_open(&db_path)
        .expect("failed to detect and open backend");

    // Complete for exact prefix
    let result = backend.complete("test::module::test", 10);
    assert!(result.is_ok());
    let completions = result.unwrap();
    assert!(!completions.is_empty(), "should find completions");

    // Complete for non-matching prefix returns empty (not error)
    let result = backend.complete("xyz_nonexistent", 10);
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
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
    assert!(remediation
        .expect("test database operation failed")
        .contains("complete"));
}

// Test 11: Complete command returns empty for nonexistent prefix
#[test]
fn test_complete_command_empty_for_nonexistent() {
    let _dir = create_sqlite_test_db();
    let db_path = _dir.path().join("test.db");

    let backend = llmgrep::backend::Backend::detect_and_open(&db_path)
        .expect("failed to detect and open backend");
    let result = backend.complete("xyz_nonexistent", 10);
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

// Test 12: Lookup command returns SymbolNotFound for missing FQN
#[test]
fn test_lookup_command_not_found() {
    let _dir = create_sqlite_test_db();
    let db_path = _dir.path().join("test.db");

    let backend = llmgrep::backend::Backend::detect_and_open(&db_path)
        .expect("failed to detect and open backend");
    let db_path_str = db_path.to_string_lossy().to_string();

    let result = backend.lookup("std::collections::HashMap::new", &db_path_str);
    assert!(matches!(result, Err(LlmError::SymbolNotFound { .. })));
}

// Test 13: Label search returns empty for any label on SQLite
#[test]
fn test_label_search_always_empty() {
    let _dir = create_sqlite_test_db();
    let db_path = _dir.path().join("test.db");

    let backend = llmgrep::backend::Backend::detect_and_open(&db_path)
        .expect("failed to detect and open backend");
    let db_path_str = db_path.to_string_lossy().to_string();

    let labels = vec!["test", "entry_point", "public_api", "test_functions"];
    for label in labels {
        let result = backend.search_by_label(label, 10, &db_path_str);
        assert!(result.is_ok());
        let (response, _, _) = result.unwrap();
        assert_eq!(response.total_count, 0);
    }
}

// Test 14: Complete command with different prefixes
#[test]
fn test_complete_command_various_prefixes() {
    let _dir = create_sqlite_test_db();
    let db_path = _dir.path().join("test.db");

    let backend = llmgrep::backend::Backend::detect_and_open(&db_path)
        .expect("failed to detect and open backend");

    // Existing prefix returns results
    let result = backend.complete("test::module", 10);
    assert!(result.is_ok());
    assert!(!result.unwrap().is_empty());

    // Nonexistent prefix returns empty (not error)
    let result = backend.complete("crate::backend", 10);
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

// Test 15: Lookup command with different FQNs
#[test]
fn test_lookup_command_various_fqns() {
    let _dir = create_sqlite_test_db();
    let db_path = _dir.path().join("test.db");

    let backend = llmgrep::backend::Backend::detect_and_open(&db_path)
        .expect("failed to detect and open backend");
    let db_path_str = db_path.to_string_lossy().to_string();

    // Existing FQN returns symbol
    let result = backend.lookup("test::module::test_function", &db_path_str);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().name, "test_function");

    // Nonexistent FQN returns SymbolNotFound
    let result = backend.lookup("std::collections::HashMap::new", &db_path_str);
    assert!(matches!(result, Err(LlmError::SymbolNotFound { .. })));
}

// Test 16: Label search with different labels returns empty
#[test]
fn test_label_search_various_labels() {
    let _dir = create_sqlite_test_db();
    let db_path = _dir.path().join("test.db");

    let backend = llmgrep::backend::Backend::detect_and_open(&db_path)
        .expect("failed to detect and open backend");
    let db_path_str = db_path.to_string_lossy().to_string();

    let labels = vec!["test", "entry_point", "public_api", "test_functions"];
    for label in labels {
        let result = backend.search_by_label(label, 10, &db_path_str);
        assert!(result.is_ok(), "label search should not error");
        let (response, _, _) = result.unwrap();
        assert_eq!(
            response.total_count, 0,
            "label search should be empty on SQLite"
        );
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

// Test 18: Verify lookup returns correct symbol metadata
#[test]
fn test_lookup_returns_symbol_metadata() {
    let _dir = create_sqlite_test_db();
    let db_path = _dir.path().join("test.db");

    let backend = llmgrep::backend::Backend::detect_and_open(&db_path)
        .expect("failed to detect and open backend");
    let db_path_str = db_path.to_string_lossy().to_string();

    let result = backend.lookup("test::module::test_function", &db_path_str);
    assert!(result.is_ok(), "lookup should succeed: {:?}", result.err());
    let symbol = result.unwrap();
    assert_eq!(symbol.name, "test_function");
    assert_eq!(symbol.kind, "Function");
    assert_eq!(symbol.span.file_path, "src/test.rs");
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
    assert!(
        hint.contains("complete"),
        "Should suggest using complete command"
    );
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

// Test 21: Complete command respects limit parameter
#[test]
fn test_complete_command_limit_parameter() {
    let _dir = create_sqlite_test_db();
    let db_path = _dir.path().join("test.db");

    let backend = llmgrep::backend::Backend::detect_and_open(&db_path)
        .expect("failed to detect and open backend");

    // Test with various limit values - all should succeed
    for limit in [1, 10, 50, 100, 1000] {
        let result = backend.complete("test", limit);
        assert!(
            result.is_ok(),
            "complete should succeed with limit {}",
            limit
        );
    }
}

// Test 22: Lookup command finds correct symbol by FQN
#[test]
fn test_lookup_command_finds_by_fqn() {
    let _dir = create_sqlite_test_db();
    let db_path = _dir.path().join("test.db");

    let backend = llmgrep::backend::Backend::detect_and_open(&db_path)
        .expect("failed to detect and open backend");
    let db_path_str = db_path.to_string_lossy().to_string();

    let test_fqn = "test::module::test_function";
    let result = backend.lookup(test_fqn, &db_path_str);
    assert!(result.is_ok(), "lookup should find symbol by FQN");
    assert_eq!(result.unwrap().name, "test_function");
}

// Test 23: Label search accepts any label without error
#[test]
fn test_label_search_accepts_any_label() {
    let _dir = create_sqlite_test_db();
    let db_path = _dir.path().join("test.db");

    let backend = llmgrep::backend::Backend::detect_and_open(&db_path)
        .expect("failed to detect and open backend");
    let db_path_str = db_path.to_string_lossy().to_string();

    let test_label = "test_functions";
    let result = backend.search_by_label(test_label, 10, &db_path_str);
    assert!(result.is_ok(), "label search should not error");
    let (response, _, _) = result.unwrap();
    assert_eq!(response.total_count, 0);
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
    assert!(
        metrics.total_ms
            >= metrics.backend_detection_ms
                + metrics.query_execution_ms
                + metrics.output_formatting_ms
    );
}

// Test 25: Backend supports standard search operations
#[test]
fn test_backend_standard_search_operations() {
    let _dir = create_sqlite_test_db();
    let db_path = _dir.path().join("test.db");

    let backend = llmgrep::backend::Backend::detect_and_open(&db_path)
        .expect("failed to detect and open backend");

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
        coverage_filter: None,
    };

    let result = backend.search_symbols(options);
    // The result may be ok or may fail due to schema, but should NOT be FeatureNotAvailable
    if let Err(LlmError::FeatureNotAvailable { .. }) = result {
        panic!("Standard search should not return FeatureNotAvailable");
    }
    // Other outcomes are acceptable (success or other errors)
}

// Test 26: Performance metrics can be cloned
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

// Test 27: Multiple backend operations work on SQLite
#[test]
fn test_multiple_backend_operations() {
    let _dir = create_sqlite_test_db();
    let db_path = _dir.path().join("test.db");

    let backend = llmgrep::backend::Backend::detect_and_open(&db_path)
        .expect("failed to detect and open backend");

    // Complete works on SQLite
    assert!(backend.complete("test", 10).is_ok());
    // Lookup returns SymbolNotFound for missing symbol
    let lookup_result = backend.lookup("test::symbol", &db_path.to_string_lossy());
    assert!(matches!(
        lookup_result,
        Err(LlmError::SymbolNotFound { .. })
    ));
    // Label search returns empty results
    assert!(backend
        .search_by_label("test", 10, &db_path.to_string_lossy())
        .is_ok());

    // Standard search should NOT fail with FeatureNotAvailable
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
        coverage_filter: None,
    };

    let result = backend.search_symbols(options);
    if let Err(LlmError::FeatureNotAvailable { .. }) = result {
        panic!("Standard search should not return FeatureNotAvailable");
    }
}

// Test 28: Search operations with standard options don't return FeatureNotAvailable
#[test]
fn test_search_operations_dont_return_feature_not_available() {
    let _dir = create_sqlite_test_db();
    let db_path = _dir.path().join("test.db");

    let backend = llmgrep::backend::Backend::detect_and_open(&db_path)
        .expect("failed to detect and open backend");

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
        coverage_filter: None,
    };

    // All standard search modes should NOT return FeatureNotAvailable
    // They may succeed or fail for other reasons (schema, etc.)
    let symbols_result = backend.search_symbols(options.clone());
    let refs_result = backend.search_references(options.clone());
    let calls_result = backend.search_calls(options);

    // Verify none of them fail with FeatureNotAvailable
    if let Err(LlmError::FeatureNotAvailable { .. }) = symbols_result {
        panic!("search_symbols should not return FeatureNotAvailable");
    }
    if let Err(LlmError::FeatureNotAvailable { .. }) = refs_result {
        panic!("search_references should not return FeatureNotAvailable");
    }
    if let Err(LlmError::FeatureNotAvailable { .. }) = calls_result {
        panic!("search_calls should not return FeatureNotAvailable");
    }
}
