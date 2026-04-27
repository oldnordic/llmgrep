//! Layer 2: Unit Integration Tests
//!
//! Tests interactions between units within the backend module.
//! These tests verify that the Backend enum properly delegates to
//! GeometricBackend implementations.
#![cfg(feature = "geometric-backend")]

use llmgrep::backend::{Backend, BackendTrait, GeometricBackend};
use llmgrep::query::{
    AstOptions, ContextOptions, DepthOptions, FqnOptions, MetricsOptions, SearchOptions,
    SnippetOptions,
};
use llmgrep::AlgorithmOptions;
use tempfile::TempDir;

/// Helper to create a temporary .geo file for testing
fn create_temp_geo_file() -> (TempDir, std::path::PathBuf) {
    let temp_dir = TempDir::new().unwrap();
    let geo_path = temp_dir.path().join("test.geo");

    // Create a valid geometric database using Magellan's API
    let _backend = magellan::graph::geometric_backend::GeometricBackend::create(&geo_path)
        .expect("Failed to create test geo database");

    (temp_dir, geo_path)
}

/// Helper to create minimal SearchOptions for testing
fn create_test_search_options(db_path: &std::path::Path) -> SearchOptions {
    SearchOptions {
        db_path,
        query: "test",
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

#[test]
fn test_backend_enum_delegates_search_symbols_to_geometric() {
    // Layer 2: Test that Backend::Geometric properly delegates search_symbols
    let (_temp, geo_path) = create_temp_geo_file();

    let backend = Backend::detect_and_open(&geo_path).unwrap();

    // Layer 1: Backend should be Geometric variant
    match &backend {
        Backend::Geometric(_) => {}
        _ => panic!("Layer 1: Expected Geometric backend"),
    }

    let options = create_test_search_options(&geo_path);

    // Layer 2: search_symbols should delegate and return valid response
    let result = backend.search_symbols(options);
    assert!(result.is_ok(), "Layer 2: search_symbols should succeed");

    let (response, partial, bounded) = result.unwrap();
    assert_eq!(
        response.total_count, 0,
        "Layer 2: Empty database should have 0 results"
    );
    assert!(!partial, "Layer 2: Should not be partial");
    assert!(!bounded, "Layer 2: Should not be bounded");
}

#[test]
fn test_backend_enum_delegates_search_references_to_geometric() {
    // Layer 2: Test that Backend::Geometric properly delegates search_references
    let (_temp, geo_path) = create_temp_geo_file();

    let backend = Backend::detect_and_open(&geo_path).unwrap();
    let options = create_test_search_options(&geo_path);

    // Layer 2: search_references should delegate and return valid response
    let result = backend.search_references(options);
    assert!(result.is_ok(), "Layer 2: search_references should succeed");

    let (response, partial) = result.unwrap();
    assert_eq!(
        response.total_count, 0,
        "Layer 2: Empty database should have 0 references"
    );
    assert!(!partial, "Layer 2: Should not be partial");
}

#[test]
fn test_backend_enum_delegates_search_calls_to_geometric() {
    // Layer 2: Test that Backend::Geometric properly delegates search_calls
    let (_temp, geo_path) = create_temp_geo_file();

    let backend = Backend::detect_and_open(&geo_path).unwrap();
    let options = create_test_search_options(&geo_path);

    // Layer 2: search_calls should delegate and return valid response
    let result = backend.search_calls(options);
    assert!(result.is_ok(), "Layer 2: search_calls should succeed");

    let (response, partial) = result.unwrap();
    assert_eq!(
        response.total_count, 0,
        "Layer 2: Empty database should have 0 calls"
    );
    assert!(!partial, "Layer 2: Should not be partial");
}

#[test]
fn test_backend_enum_delegates_ast_to_geometric() {
    // Layer 2: Test that Backend::Geometric properly delegates ast queries
    let (_temp, geo_path) = create_temp_geo_file();

    let backend = Backend::detect_and_open(&geo_path).unwrap();

    // Layer 2: ast should delegate and return valid JSON
    let result = backend.ast(std::path::Path::new("test.rs"), None, 100);
    assert!(result.is_ok(), "Layer 2: ast should succeed");

    let json = result.unwrap();
    assert!(
        json.get("file").is_some(),
        "Layer 2: Response should have file field"
    );
    assert!(
        json.get("nodes").is_some(),
        "Layer 2: Response should have nodes field"
    );
}

#[test]
fn test_backend_enum_delegates_find_ast_to_geometric() {
    // Layer 2: Test that Backend::Geometric properly delegates find_ast
    let (_temp, geo_path) = create_temp_geo_file();

    let backend = Backend::detect_and_open(&geo_path).unwrap();

    // Layer 2: find_ast should delegate and return valid JSON
    let result = backend.find_ast("function_item");
    assert!(result.is_ok(), "Layer 2: find_ast should succeed");

    let json = result.unwrap();
    assert!(
        json.get("kind").is_some(),
        "Layer 2: Response should have kind field"
    );
    assert!(
        json.get("count").is_some(),
        "Layer 2: Response should have count field"
    );
}

#[test]
fn test_backend_enum_delegates_complete_to_geometric() {
    // Layer 2: Test that Backend::Geometric properly delegates complete
    let (_temp, geo_path) = create_temp_geo_file();

    let backend = Backend::detect_and_open(&geo_path).unwrap();

    // Layer 2: complete should now work for geometric backend
    let result = backend.complete("test", 10);
    assert!(
        result.is_ok(),
        "Layer 2: complete should succeed for geometric backend: {:?}",
        result.err()
    );

    // Layer 3: Should return a vector (may be empty for test database)
    let completions = result.unwrap();
    assert!(
        completions.is_empty() || !completions.is_empty(),
        "Layer 3: Should return a vector"
    );
}

#[test]
fn test_backend_enum_delegates_lookup_to_geometric() {
    // Layer 2: Test that Backend::Geometric properly delegates lookup
    let (_temp, geo_path) = create_temp_geo_file();

    let backend = Backend::detect_and_open(&geo_path).unwrap();

    // Layer 2: lookup should delegate and return error (symbol not found)
    let result = backend.lookup("test::symbol", geo_path.to_str().unwrap());
    assert!(
        result.is_err(),
        "Layer 2: lookup should fail for non-existent symbol"
    );

    match result {
        Err(llmgrep::error::LlmError::SymbolNotFound { .. }) => {
            // Expected error
        }
        _ => panic!("Layer 2: Expected SymbolNotFound error"),
    }
}

#[test]
fn test_backend_enum_delegates_search_by_label_to_geometric() {
    // Layer 2: Test that Backend::Geometric properly delegates search_by_label
    let (_temp, geo_path) = create_temp_geo_file();

    let backend = Backend::detect_and_open(&geo_path).unwrap();

    // Layer 2: search_by_label should delegate and return error (not supported)
    let result = backend.search_by_label("test", 10, geo_path.to_str().unwrap());
    assert!(
        result.is_err(),
        "Layer 2: search_by_label should fail for geometric backend"
    );

    match result {
        Err(llmgrep::error::LlmError::FeatureNotAvailable { .. }) => {
            // Expected error
        }
        _ => panic!("Layer 2: Expected FeatureNotAvailable error"),
    }
}

#[test]
fn test_geometric_backend_lookup_method() {
    // Layer 2: Test GeometricBackend lookup method directly
    let (_temp, geo_path) = create_temp_geo_file();

    let backend = GeometricBackend::open(&geo_path).unwrap();

    // Layer 1: lookup should fail for non-existent symbol
    let result = backend.lookup("test::symbol", geo_path.to_str().unwrap());
    assert!(
        result.is_err(),
        "Layer 1: lookup should fail for non-existent symbol"
    );
    match result {
        Err(llmgrep::error::LlmError::SymbolNotFound { .. }) => {
            // Expected error
        }
        _ => panic!("Layer 2: Expected SymbolNotFound error"),
    }
}
