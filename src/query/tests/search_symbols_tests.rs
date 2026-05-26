use super::*;

#[test]
fn test_search_symbols_basic() {
    let (_db_file, _conn) = create_test_db();
    let db_path = _db_file.path();

    let options = SearchOptions {
        db_path,
        query: "test_func",
        path_filter: None,
        kind_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions::default(),
        include_score: false,
        sort_by: SortMode::default(),
        metrics: MetricsOptions::default(),
        ast: AstOptions::default(),
        depth: DepthOptions::default(),
        algorithm: AlgorithmOptions::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
        language_filter: None,
        coverage_filter: None,
    };

    let (response, partial, _) = search_symbols(options).expect("search_symbols should succeed");
    assert!(!partial, "Should not be partial");
    assert_eq!(response.results.len(), 1, "Should find 1 result");
    assert_eq!(
        response.results[0].name, "test_func",
        "Should match test_func"
    );
}

#[test]
fn test_search_symbols_empty_results() {
    let (_db_file, _conn) = create_test_db();
    let db_path = _db_file.path();

    let options = SearchOptions {
        db_path,
        query: "nonexistent",
        path_filter: None,
        kind_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions::default(),
        include_score: false,
        sort_by: SortMode::default(),
        metrics: MetricsOptions::default(),
        ast: AstOptions::default(),
        depth: DepthOptions::default(),
        algorithm: AlgorithmOptions::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
        language_filter: None,
        coverage_filter: None,
    };

    let (response, partial, _) = search_symbols(options).expect("search_symbols should succeed");
    assert!(!partial, "Should not be partial");
    assert_eq!(response.results.len(), 0, "Should find 0 results");
}

#[test]
fn test_search_symbols_prefix_match() {
    let (_db_file, _conn) = create_test_db();
    let db_path = _db_file.path();

    let options = SearchOptions {
        db_path,
        query: "test",
        path_filter: None,
        kind_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions::default(),
        include_score: false,
        sort_by: SortMode::default(),
        metrics: MetricsOptions::default(),
        ast: AstOptions::default(),
        depth: DepthOptions::default(),
        algorithm: AlgorithmOptions::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
        language_filter: None,
        coverage_filter: None,
    };

    let (response, partial, _) = search_symbols(options).expect("search_symbols should succeed");
    assert!(!partial, "Should not be partial");
    assert_eq!(response.results.len(), 2, "Should find 2 results");

    let names: Vec<&str> = response.results.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"test_func"), "Should contain test_func");
    assert!(names.contains(&"TestStruct"), "Should contain TestStruct");
}

#[test]
fn test_search_symbols_contains_match() {
    let (_db_file, _conn) = create_test_db();
    let db_path = _db_file.path();

    let options = SearchOptions {
        db_path,
        query: "helper",
        path_filter: None,
        kind_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions::default(),
        include_score: false,
        sort_by: SortMode::default(),
        metrics: MetricsOptions::default(),
        ast: AstOptions::default(),
        depth: DepthOptions::default(),
        algorithm: AlgorithmOptions::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
        language_filter: None,
        coverage_filter: None,
    };

    let (response, partial, _) = search_symbols(options).expect("search_symbols should succeed");
    assert!(!partial, "Should not be partial");
    assert_eq!(response.results.len(), 1, "Should find 1 result");
    assert_eq!(response.results[0].name, "helper", "Should match helper");
}

#[test]
fn test_search_symbols_kind_filter() {
    let (_db_file, _conn) = create_test_db();
    let db_path = _db_file.path();

    let options = SearchOptions {
        db_path,
        query: "test",
        path_filter: None,
        kind_filter: Some("Function"),
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions::default(),
        include_score: false,
        sort_by: SortMode::default(),
        metrics: MetricsOptions::default(),
        ast: AstOptions::default(),
        depth: DepthOptions::default(),
        algorithm: AlgorithmOptions::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
        language_filter: None,
        coverage_filter: None,
    };

    let (response, partial, _) = search_symbols(options).expect("search_symbols should succeed");
    assert!(!partial, "Should not be partial");
    assert_eq!(response.results.len(), 1, "Should find 1 Function result");
    assert_eq!(response.results[0].name, "test_func", "Should be test_func");
    assert_eq!(
        response.results[0].kind, "Function",
        "Should be Function kind"
    );
}

#[test]
fn test_search_symbols_limit() {
    let (_db_file, _conn) = create_test_db();
    let db_path = _db_file.path();

    let options = SearchOptions {
        db_path,
        query: "test",
        path_filter: None,
        kind_filter: None,
        limit: 1,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions::default(),
        include_score: false,
        sort_by: SortMode::default(),
        metrics: MetricsOptions::default(),
        ast: AstOptions::default(),
        depth: DepthOptions::default(),
        algorithm: AlgorithmOptions::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
        language_filter: None,
        coverage_filter: None,
    };

    let (response, partial, _) = search_symbols(options).expect("search_symbols should succeed");
    assert!(!partial, "Should not be partial");
    assert_eq!(
        response.results.len(),
        1,
        "Should return at most 1 result due to limit"
    );
}

#[test]
fn test_search_symbols_regex_mode() {
    let (_db_file, _conn) = create_test_db();
    let db_path = _db_file.path();

    let options = SearchOptions {
        db_path,
        query: "test.*",
        path_filter: None,
        kind_filter: None,
        limit: 10,
        use_regex: true,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions::default(),
        include_score: false,
        sort_by: SortMode::default(),
        metrics: MetricsOptions::default(),
        ast: AstOptions::default(),
        depth: DepthOptions::default(),
        algorithm: AlgorithmOptions::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
        language_filter: None,
        coverage_filter: None,
    };

    let (response, partial, _) = search_symbols(options).expect("search_symbols should succeed");
    assert!(!partial, "Should not be partial");
    assert_eq!(
        response.results.len(),
        1,
        "Should find 1 result matching regex"
    );
    assert_eq!(response.results[0].name, "test_func", "Should be test_func");
}

#[test]
fn test_search_symbols_regex_no_match() {
    let (_db_file, _conn) = create_test_db();
    let db_path = _db_file.path();

    let options = SearchOptions {
        db_path,
        query: "xyz.*",
        path_filter: None,
        kind_filter: None,
        limit: 10,
        use_regex: true,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions::default(),
        include_score: false,
        sort_by: SortMode::default(),
        metrics: MetricsOptions::default(),
        ast: AstOptions::default(),
        depth: DepthOptions::default(),
        algorithm: AlgorithmOptions::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
        language_filter: None,
        coverage_filter: None,
    };

    let (response, partial, _) = search_symbols(options).expect("search_symbols should succeed");
    assert!(!partial, "Should not be partial");
    assert_eq!(response.results.len(), 0, "Should find 0 results");
}

#[test]
fn test_search_symbols_score_exact_match() {
    let (_db_file, _conn) = create_test_db();
    let db_path = _db_file.path();

    let options = SearchOptions {
        db_path,
        query: "test_func",
        path_filter: None,
        kind_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions::default(),
        include_score: true,
        sort_by: SortMode::default(),
        metrics: MetricsOptions::default(),
        ast: AstOptions::default(),
        depth: DepthOptions::default(),
        algorithm: AlgorithmOptions::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
        language_filter: None,
        coverage_filter: None,
    };

    let (response, partial, _) = search_symbols(options).expect("search_symbols should succeed");
    assert!(!partial, "Should not be partial");
    assert_eq!(response.results.len(), 1, "Should find 1 result");
    assert_eq!(
        response.results[0].score,
        Some(100),
        "Exact match should have score 100"
    );
}

#[test]
fn test_search_symbols_score_prefix_match() {
    let (_db_file, _conn) = create_test_db();
    let db_path = _db_file.path();

    let options = SearchOptions {
        db_path,
        query: "test",
        path_filter: None,
        kind_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions::default(),
        include_score: true,
        sort_by: SortMode::default(),
        metrics: MetricsOptions::default(),
        ast: AstOptions::default(),
        depth: DepthOptions::default(),
        algorithm: AlgorithmOptions::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
        language_filter: None,
        coverage_filter: None,
    };

    let (response, partial, _) = search_symbols(options).expect("search_symbols should succeed");
    assert!(!partial, "Should not be partial");
    assert_eq!(response.results.len(), 2, "Should find 2 results");

    let test_func = response
        .results
        .iter()
        .find(|r| r.name == "test_func")
        .expect("test_func should be in results");
    assert_eq!(
        test_func.score,
        Some(80),
        "test_func should have prefix score 80"
    );

    let test_struct = response
        .results
        .iter()
        .find(|r| r.name == "TestStruct")
        .expect("TestStruct should be in results");
    assert_eq!(
        test_struct.score,
        Some(0),
        "TestStruct should have score 0 (case mismatch)"
    );
}

#[test]
fn test_search_symbols_partial_result() {
    let (_db_file, _conn) = create_test_db();
    let db_path = _db_file.path();

    let options = SearchOptions {
        db_path,
        query: "test",
        path_filter: None,
        kind_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 1,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions::default(),
        include_score: false,
        sort_by: SortMode::default(),
        metrics: MetricsOptions::default(),
        ast: AstOptions::default(),
        depth: DepthOptions::default(),
        algorithm: AlgorithmOptions::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
        language_filter: None,
        coverage_filter: None,
    };

    let (response, partial, _) = search_symbols(options).expect("search_symbols should succeed");
    assert!(partial, "Should be partial since candidates < total count");
    assert_eq!(response.results.len(), 1, "Should return at most 1 result");
}

#[test]
fn test_search_symbols_total_count() {
    let (_db_file, _conn) = create_test_db();
    let db_path = _db_file.path();

    let options = SearchOptions {
        db_path,
        query: "test",
        path_filter: None,
        kind_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions::default(),
        include_score: false,
        sort_by: SortMode::default(),
        metrics: MetricsOptions::default(),
        ast: AstOptions::default(),
        depth: DepthOptions::default(),
        algorithm: AlgorithmOptions::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
        language_filter: None,
        coverage_filter: None,
    };

    let (response, partial, _) = search_symbols(options).expect("search_symbols should succeed");
    assert!(!partial, "Should not be partial");
    assert_eq!(response.total_count, 2, "Total count should be 2");
}

#[test]
fn test_search_symbols_ordering() {
    let (_db_file, _conn) = create_test_db();
    let db_path = _db_file.path();

    let options = SearchOptions {
        db_path,
        query: "test",
        path_filter: None,
        kind_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions::default(),
        include_score: true,
        sort_by: SortMode::default(),
        metrics: MetricsOptions::default(),
        ast: AstOptions::default(),
        depth: DepthOptions::default(),
        algorithm: AlgorithmOptions::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
        language_filter: None,
        coverage_filter: None,
    };

    let (response, partial, _) = search_symbols(options).expect("search_symbols should succeed");
    assert!(!partial, "Should not be partial");
    assert_eq!(response.results.len(), 2, "Should find 2 results");

    assert_eq!(
        response.results[0].name, "test_func",
        "test_func should be first (higher score)"
    );
    assert_eq!(
        response.results[0].score,
        Some(80),
        "test_func should have prefix score 80"
    );
    assert_eq!(
        response.results[1].name, "TestStruct",
        "TestStruct should be second"
    );
    assert_eq!(
        response.results[1].score,
        Some(0),
        "TestStruct should have score 0"
    );
}

#[test]
fn test_search_symbols_include_score() {
    let (_db_file, _conn) = create_test_db();
    let db_path = _db_file.path();

    let options = SearchOptions {
        db_path,
        query: "test_func",
        path_filter: None,
        kind_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions::default(),
        include_score: true,
        sort_by: SortMode::default(),
        metrics: MetricsOptions::default(),
        ast: AstOptions::default(),
        depth: DepthOptions::default(),
        algorithm: AlgorithmOptions::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
        language_filter: None,
        coverage_filter: None,
    };

    let (response, partial, _) = search_symbols(options).expect("search_symbols should succeed");
    assert!(!partial, "Should not be partial");
    assert_eq!(response.results.len(), 1, "Should find 1 result");
    assert!(
        response.results[0].score.is_some(),
        "Score should be included"
    );
    assert_eq!(response.results[0].score, Some(100), "Score should be 100");
}

#[test]
fn test_search_symbols_with_fqn() {
    let (_db_file, _conn) = create_test_db();
    let db_path = _db_file.path();

    let options = SearchOptions {
        db_path,
        query: "test_func",
        path_filter: None,
        kind_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions {
            fqn: true,
            canonical_fqn: false,
            display_fqn: false,
        },
        include_score: false,
        sort_by: SortMode::default(),
        metrics: MetricsOptions::default(),
        ast: AstOptions::default(),
        depth: DepthOptions::default(),
        algorithm: AlgorithmOptions::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
        language_filter: None,
        coverage_filter: None,
    };

    let (response, partial, _) = search_symbols(options).expect("search_symbols should succeed");
    assert!(!partial, "Should not be partial");
    assert_eq!(response.results.len(), 1, "Should find 1 result");
    assert_eq!(
        response.results[0].fqn,
        Some("module::test_func".to_string()),
        "FQN should be included"
    );
    assert!(
        response.results[0].display_fqn.is_none(),
        "display_fqn should not be included"
    );
}
