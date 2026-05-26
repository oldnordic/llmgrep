use super::*;
use rusqlite::Connection;
use std::path::PathBuf;
use tempfile::NamedTempFile;

fn create_test_db_with_calls() -> (NamedTempFile, Connection) {
    let db_file = NamedTempFile::new().expect("failed to create temp file");
    let conn = Connection::open(db_file.path()).expect("failed to open database");

    conn.execute(
        "CREATE TABLE graph_entities (
            id INTEGER PRIMARY KEY,
            kind TEXT NOT NULL,
            data TEXT NOT NULL
        )",
        [],
    )
    .expect("failed to create graph_entities table");

    conn.execute(
        "INSERT INTO graph_entities (id, kind, data) VALUES
            (10, 'Call', '{\"file\":\"/test/file.rs\",\"caller\":\"main\",\"callee\":\"test_func\",\"caller_symbol_id\":\"sym1\",\"callee_symbol_id\":\"sym2\",\"byte_start\":50,\"byte_end\":70,\"start_line\":5,\"start_col\":4,\"end_line\":5,\"end_col\":24}'),
            (11, 'Call', '{\"file\":\"/test/file.rs\",\"caller\":\"main\",\"callee\":\"helper\",\"caller_symbol_id\":\"sym1\",\"callee_symbol_id\":\"sym3\",\"byte_start\":100,\"byte_end\":115,\"start_line\":10,\"start_col\":4,\"end_line\":10,\"end_col\":19}'),
            (12, 'Call', '{\"file\":\"/test/other.rs\",\"caller\":\"process\",\"callee\":\"test_func\",\"caller_symbol_id\":\"sym4\",\"callee_symbol_id\":\"sym2\",\"byte_start\":200,\"byte_end\":220,\"start_line\":20,\"start_col\":0,\"end_line\":20,\"end_col\":20}')",
        [],
    ).expect("failed to execute SQL");

    (db_file, conn)
}

#[test]
fn test_search_calls_basic() {
    let (_db_file, _conn) = create_test_db_with_calls();

    let options = SearchOptions {
        db_path: _db_file.path(),
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

    let (response, _partial) = search_calls(options).expect("search_calls should succeed");

    assert_eq!(response.results.len(), 2);
    assert_eq!(response.total_count, 2);
    assert_eq!(response.query, "test_func");

    for result in &response.results {
        assert_eq!(result.callee, "test_func");
    }
}

#[test]
fn test_search_calls_caller_match() {
    let (_db_file, _conn) = create_test_db_with_calls();

    let options = SearchOptions {
        db_path: _db_file.path(),
        query: "main",
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

    let (response, _partial) = search_calls(options).expect("search_calls should succeed");

    assert_eq!(response.results.len(), 2);
    assert_eq!(response.total_count, 2);

    for result in &response.results {
        assert_eq!(result.caller, "main");
    }
}

#[test]
fn test_search_calls_empty_results() {
    let (_db_file, _conn) = create_test_db_with_calls();

    let options = SearchOptions {
        db_path: _db_file.path(),
        query: "nonexistent",
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

    let (response, _partial) = search_calls(options).expect("search_calls should succeed");

    assert_eq!(response.results.len(), 0);
    assert_eq!(response.total_count, 0);
}

#[test]
fn test_search_calls_regex_mode() {
    let (_db_file, _conn) = create_test_db_with_calls();

    let options = SearchOptions {
        db_path: _db_file.path(),
        query: "test.*",
        path_filter: None,
        kind_filter: None,
        limit: 10,
        use_regex: true,
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

    let (response, _partial) = search_calls(options).expect("search_calls should succeed");

    assert_eq!(response.results.len(), 2);
    assert_eq!(response.total_count, 2);

    for result in &response.results {
        assert_eq!(result.callee, "test_func");
    }
}

#[test]
fn test_search_calls_regex_no_match() {
    let (_db_file, _conn) = create_test_db_with_calls();

    let options = SearchOptions {
        db_path: _db_file.path(),
        query: "xyz.*",
        path_filter: None,
        kind_filter: None,
        limit: 10,
        use_regex: true,
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

    let (response, _partial) = search_calls(options).expect("search_calls should succeed");

    assert_eq!(response.results.len(), 0);
    assert_eq!(response.total_count, 0);
}

#[test]
fn test_search_calls_score_callee_match() {
    let (_db_file, _conn) = create_test_db_with_calls();

    let options = SearchOptions {
        db_path: _db_file.path(),
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

    let (response, _partial) = search_calls(options).expect("search_calls should succeed");

    assert!(!response.results.is_empty());

    for result in &response.results {
        assert!(result.score.is_some());
        assert_eq!(result.score.expect("score should be Some"), 100);
    }
}

#[test]
fn test_search_calls_score_caller_match() {
    let (_db_file, _conn) = create_test_db_with_calls();

    let options = SearchOptions {
        db_path: _db_file.path(),
        query: "main",
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

    let (response, _partial) = search_calls(options).expect("search_calls should succeed");

    assert!(!response.results.is_empty());

    for result in &response.results {
        assert!(result.score.is_some());
        assert_eq!(result.score.expect("score should be Some"), 100);
    }
}

#[test]
fn test_search_calls_limit() {
    let (_db_file, _conn) = create_test_db_with_calls();

    let options = SearchOptions {
        db_path: _db_file.path(),
        query: "test_func",
        path_filter: None,
        kind_filter: None,
        limit: 1,
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

    let (response, _partial) = search_calls(options).expect("search_calls should succeed");

    assert_eq!(response.results.len(), 1);
    assert_eq!(response.total_count, 2);
}

#[test]
fn test_search_calls_total_count() {
    let (_db_file, _conn) = create_test_db_with_calls();

    let options = SearchOptions {
        db_path: _db_file.path(),
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

    let (response, _partial) = search_calls(options).expect("search_calls should succeed");

    assert_eq!(response.total_count, 2);
}

#[test]
fn test_search_calls_path_filter() {
    let (_db_file, _conn) = create_test_db_with_calls();

    let path = PathBuf::from("/test/file.rs");
    let options = SearchOptions {
        db_path: _db_file.path(),
        query: "test_func",
        path_filter: Some(&path),
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

    let (response, _partial) = search_calls(options).expect("search_calls should succeed");

    assert_eq!(response.results.len(), 1);
    assert_eq!(response.total_count, 1);
    assert_eq!(response.results[0].span.file_path, "/test/file.rs");
}

#[test]
fn test_search_calls_include_score() {
    let (_db_file, _conn) = create_test_db_with_calls();

    let options = SearchOptions {
        db_path: _db_file.path(),
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

    let (response, _partial) = search_calls(options).expect("search_calls should succeed");

    for result in &response.results {
        assert!(result.score.is_some());
        assert!(result.score.expect("score should be Some") > 0);
    }
}

#[test]
fn test_search_calls_ordering() {
    let (_db_file, _conn) = create_test_db_with_calls();

    let options = SearchOptions {
        db_path: _db_file.path(),
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

    let (response, _partial) = search_calls(options).expect("search_calls should succeed");

    if response.results.len() > 1 {
        for i in 1..response.results.len() {
            let prev = &response.results[i - 1];
            let curr = &response.results[i];
            assert!(
                prev.score.expect("score should be Some")
                    >= curr.score.expect("score should be Some")
            );
            if prev.score == curr.score {
                assert!(prev.span.start_line <= curr.span.start_line);
            }
        }
    }
}
