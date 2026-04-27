//! Layer 3: Component Integration Tests
//!
//! Tests cross-crate component interactions between llmgrep and
//! the Magellan geometric backend. These tests verify end-to-end
//! workflows work correctly.
#![cfg(feature = "geometric-backend")]

use llmgrep::backend::Backend;
use llmgrep::query::{
    AstOptions, ContextOptions, DepthOptions, FqnOptions, MetricsOptions, SearchOptions,
    SnippetOptions,
};
use llmgrep::AlgorithmOptions;
use std::io::Write;
use tempfile::{NamedTempFile, TempDir};

/// Helper to create a temporary .geo file for testing
fn create_temp_geo_file() -> (TempDir, std::path::PathBuf) {
    let temp_dir = TempDir::new().unwrap();
    let geo_path = temp_dir.path().join("test.geo");

    // Create a valid geometric database using Magellan's API
    let _backend = magellan::graph::geometric_backend::GeometricBackend::create(&geo_path)
        .expect("Failed to create test geo database");

    (temp_dir, geo_path)
}

/// Helper to create SearchOptions for testing
fn create_test_search_options<'a>(
    db_path: &'a std::path::Path,
    query: &'a str,
) -> SearchOptions<'a> {
    SearchOptions {
        db_path,
        query,
        path_filter: None,
        kind_filter: None,
        language_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions {
            include: false,
            lines: 3,
            max_lines: 20,
        },
        snippet: SnippetOptions {
            include: false,
            max_bytes: 200,
        },
        fqn: FqnOptions {
            fqn: false,
            canonical_fqn: false,
            display_fqn: false,
        },
        include_score: true,
        sort_by: llmgrep::SortMode::Relevance,
        metrics: MetricsOptions {
            min_complexity: None,
            max_complexity: None,
            min_fan_in: None,
            min_fan_out: None,
        },
        ast: AstOptions {
            ast_kinds: Vec::new(),
            with_ast_context: false,
            _phantom: std::marker::PhantomData,
        },
        depth: DepthOptions {
            min_depth: None,
            max_depth: None,
            inside: None,
            contains: None,
        },
        algorithm: AlgorithmOptions::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
        coverage_filter: None,
    }
}

/// Test that the full workflow of opening a .geo file and performing
/// multiple operations works correctly.
#[test]
fn test_full_workflow_open_and_search() {
    // Layer 3: Test complete workflow
    let (_temp, geo_path) = create_temp_geo_file();

    // Step 1: Open the database
    let backend = Backend::detect_and_open(&geo_path).unwrap();

    // Step 2: Perform symbol search
    let options = create_test_search_options(&geo_path, "test_function");
    let result = backend.search_symbols(options);
    assert!(result.is_ok(), "Layer 3: Symbol search should succeed");

    // Step 3: Perform reference search
    let options = create_test_search_options(&geo_path, "test_function");
    let result = backend.search_references(options);
    assert!(result.is_ok(), "Layer 3: Reference search should succeed");

    // Step 4: Perform call search
    let options = create_test_search_options(&geo_path, "test_function");
    let result = backend.search_calls(options);
    assert!(result.is_ok(), "Layer 3: Call search should succeed");

    // Step 5: Query AST
    let result = backend.ast(std::path::Path::new("test.rs"), None, 100);
    assert!(result.is_ok(), "Layer 3: AST query should succeed");

    // All operations completed successfully
}

/// Test that multiple backends can be opened and used independently.
#[test]
fn test_multiple_backends_independent() {
    // Layer 3: Test multiple backend instances work independently
    let (_temp1, geo_path1) = create_temp_geo_file();
    let (_temp2, geo_path2) = create_temp_geo_file();

    // Open two independent backends
    let backend1 = Backend::detect_and_open(&geo_path1).unwrap();
    let backend2 = Backend::detect_and_open(&geo_path2).unwrap();

    // Both should be Geometric variant
    match (&backend1, &backend2) {
        (Backend::Geometric(_), Backend::Geometric(_)) => {
            // Both are geometric - good
        }
        _ => panic!("Layer 3: Both backends should be Geometric variant"),
    }

    // Operations on backend1 should not affect backend2
    let options1 = create_test_search_options(&geo_path1, "query1");
    let result1 = backend1.search_symbols(options1);
    assert!(result1.is_ok(), "Layer 3: Backend1 search should succeed");

    let options2 = create_test_search_options(&geo_path2, "query2");
    let result2 = backend2.search_symbols(options2);
    assert!(result2.is_ok(), "Layer 3: Backend2 search should succeed");

    // Both should return empty results (empty databases)
    let (response1, _, _) = result1.unwrap();
    let (response2, _, _) = result2.unwrap();
    assert_eq!(
        response1.total_count, 0,
        "Layer 3: Backend1 should have 0 results"
    );
    assert_eq!(
        response2.total_count, 0,
        "Layer 3: Backend2 should have 0 results"
    );
}

/// Test error handling across the full workflow.
#[test]
fn test_error_handling_workflow() {
    // Layer 3: Test error handling in complete workflow

    // Test 1: Non-existent database
    let fake_path = std::path::Path::new("/nonexistent/test.geo");
    let result = Backend::detect_and_open(fake_path);
    assert!(
        result.is_err(),
        "Layer 3: Should fail for non-existent database"
    );
    match result {
        Err(llmgrep::error::LlmError::DatabaseNotFound { .. }) => {}
        _ => panic!("Layer 3: Expected DatabaseNotFound error"),
    }

    // Test 2: Invalid file extension
    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(b"INVALID").unwrap();
    let result = Backend::detect_and_open(temp_file.path());
    // Should try SQLite fallback and fail
    assert!(result.is_err(), "Layer 3: Should fail for invalid database");
}

/// Test that path handling works correctly with different path formats.
#[test]
fn test_path_handling_variations() {
    // Layer 3: Test path normalization and handling
    let (_temp, geo_path) = create_temp_geo_file();

    let backend = Backend::detect_and_open(&geo_path).unwrap();

    // Test with absolute path
    let abs_path = geo_path.canonicalize().unwrap_or_else(|_| geo_path.clone());
    let backend_abs = Backend::detect_and_open(&abs_path);
    assert!(
        backend_abs.is_ok(),
        "Layer 3: Should work with absolute paths"
    );

    // Test AST query with various path formats
    let result = backend.ast(std::path::Path::new("./src/main.rs"), None, 100);
    assert!(
        result.is_ok(),
        "Layer 3: AST query should handle relative paths"
    );

    let result = backend.ast(std::path::Path::new("src/main.rs"), None, 100);
    assert!(
        result.is_ok(),
        "Layer 3: AST query should handle paths without ./"
    );
}

/// Test performance characteristics - operations should complete in reasonable time.
#[test]
fn test_performance_characteristics() {
    // Layer 3: Test that operations complete within reasonable time
    let (_temp, geo_path) = create_temp_geo_file();

    let backend = Backend::detect_and_open(&geo_path).unwrap();

    // Measure search performance
    let start = std::time::Instant::now();
    let options = create_test_search_options(&geo_path, "test");
    let _ = backend.search_symbols(options);
    let search_duration = start.elapsed();

    // Should complete in less than 1 second (even for empty database)
    assert!(
        search_duration.as_secs() < 1,
        "Layer 3: Search should complete in < 1 second, took {:?}",
        search_duration
    );

    // Measure AST query performance
    let start = std::time::Instant::now();
    let _ = backend.ast(std::path::Path::new("test.rs"), None, 100);
    let ast_duration = start.elapsed();

    // Should complete in less than 1 second
    assert!(
        ast_duration.as_secs() < 1,
        "Layer 3: AST query should complete in < 1 second, took {:?}",
        ast_duration
    );
}

/// Test that all BackendTrait methods are implemented and callable.
#[test]
fn test_all_trait_methods_callable() {
    // Layer 3: Test that all BackendTrait methods can be called
    let (_temp, geo_path) = create_temp_geo_file();

    let backend = Backend::detect_and_open(&geo_path).unwrap();
    let options = create_test_search_options(&geo_path, "test");

    // All methods should be callable without panicking
    let _ = backend.search_symbols(options.clone());
    let _ = backend.search_references(options.clone());
    let _ = backend.search_calls(options);
    let _ = backend.ast(std::path::Path::new("test.rs"), None, 100);
    let _ = backend.find_ast("function_item");
    let _ = backend.complete("test", 10);
    let _ = backend.lookup("test::symbol", geo_path.to_str().unwrap());
    let _ = backend.search_by_label("test", 10, geo_path.to_str().unwrap());

    // If we get here, all methods were callable
}
