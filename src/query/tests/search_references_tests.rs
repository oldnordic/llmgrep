use super::*;
use rusqlite::Connection;
use serde_json::json;
use std::path::PathBuf;
use tempfile::NamedTempFile;

fn create_test_db_with_references() -> (NamedTempFile, Connection) {
    let db_file = NamedTempFile::new().expect("failed to create temp file");
    let conn = Connection::open(db_file.path()).expect("failed to open database");

    conn.execute(
        "CREATE TABLE graph_entities (
            id INTEGER PRIMARY KEY,
            kind TEXT NOT NULL,
            data TEXT NOT NULL,
            name TEXT
        )",
        [],
    )
    .expect("failed to execute SQL");
    conn.execute(
        "CREATE TABLE graph_edges (
            id INTEGER PRIMARY KEY,
            from_id INTEGER NOT NULL,
            to_id INTEGER NOT NULL,
            edge_type TEXT NOT NULL
        )",
        [],
    )
    .expect("failed to execute SQL");

    let symbol_data = json!({
        "symbol_id": "sym1",
        "name": "test_func",
        "kind": "Function",
        "kind_normalized": "function"
    })
    .to_string();
    conn.execute(
        "INSERT INTO graph_entities (id, kind, data) VALUES (1, 'Symbol', ?1)",
        [symbol_data],
    )
    .expect("failed to execute SQL");

    let ref1_data = json!({
        "file": "/test/file.rs",
        "byte_start": 50,
        "byte_end": 60,
        "start_line": 3,
        "start_col": 5,
        "end_line": 3,
        "end_col": 14
    })
    .to_string();
    conn.execute(
        "INSERT INTO graph_entities (id, kind, name, data) VALUES
            (10, 'Reference', 'ref to test_func', ?1)",
        [ref1_data],
    )
    .expect("failed to execute SQL");

    let ref2_data = json!({
        "file": "/test/file.rs",
        "byte_start": 100,
        "byte_end": 112,
        "start_line": 7,
        "start_col": 0,
        "end_line": 7,
        "end_col": 12
    })
    .to_string();
    conn.execute(
        "INSERT INTO graph_entities (id, kind, name, data) VALUES
            (11, 'Reference', 'ref to TestStruct', ?1)",
        [ref2_data],
    )
    .expect("failed to execute SQL");

    let ref3_data = json!({
        "file": "/test/other.rs",
        "byte_start": 200,
        "byte_end": 210,
        "start_line": 10,
        "start_col": 0,
        "end_line": 10,
        "end_col": 10
    })
    .to_string();
    conn.execute(
        "INSERT INTO graph_entities (id, kind, name, data) VALUES
            (12, 'Reference', 'ref to helper', ?1)",
        [ref3_data],
    )
    .expect("failed to execute SQL");

    conn.execute(
        "INSERT INTO graph_edges (from_id, to_id, edge_type) VALUES (10, 1, 'REFERENCES')",
        [],
    )
    .expect("failed to execute SQL");

    (db_file, conn)
}

#[test]
fn test_search_references_basic() {
    let (db_file, _conn) = create_test_db_with_references();

    let options = SearchOptions {
        db_path: db_file.path(),
        query: "test_func",
        path_filter: None,
        kind_filter: None,
        limit: 100,
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

    let (result, _partial) = search_references(options).expect("search_references should succeed");
    assert_eq!(
        result.results.len(),
        1,
        "Should find 1 reference to test_func"
    );
    assert_eq!(result.results[0].referenced_symbol, "test_func");
    assert_eq!(result.query, "test_func");
}

#[test]
fn test_search_references_empty_results() {
    let (db_file, _conn) = create_test_db_with_references();

    let options = SearchOptions {
        db_path: db_file.path(),
        query: "nonexistent",
        path_filter: None,
        kind_filter: None,
        limit: 100,
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

    let (result, _partial) = search_references(options).expect("search_references should succeed");
    assert_eq!(
        result.results.len(),
        0,
        "Should find 0 references for nonexistent symbol"
    );
}

#[test]
fn test_search_references_prefix_match() {
    let (db_file, _conn) = create_test_db_with_references();

    let options = SearchOptions {
        db_path: db_file.path(),
        query: "test",
        path_filter: None,
        kind_filter: None,
        limit: 100,
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

    let (result, _partial) = search_references(options).expect("search_references should succeed");
    assert_eq!(
        result.results.len(),
        1,
        "Should find 1 reference with 'test' prefix"
    );
    assert_eq!(result.results[0].referenced_symbol, "test_func");
}

#[test]
fn test_search_references_regex_mode() {
    let (db_file, _conn) = create_test_db_with_references();

    let options = SearchOptions {
        db_path: db_file.path(),
        query: "test.*",
        path_filter: None,
        kind_filter: None,
        limit: 100,
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

    let (result, _partial) = search_references(options).expect("search_references should succeed");
    assert_eq!(
        result.results.len(),
        1,
        "Should find 1 reference matching regex 'test.*'"
    );
    assert_eq!(result.results[0].referenced_symbol, "test_func");
}

#[test]
fn test_search_references_regex_no_match() {
    let (db_file, _conn) = create_test_db_with_references();

    let options = SearchOptions {
        db_path: db_file.path(),
        query: "xyz.*",
        path_filter: None,
        kind_filter: None,
        limit: 100,
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

    let (result, _partial) = search_references(options).expect("search_references should succeed");
    assert_eq!(
        result.results.len(),
        0,
        "Should find 0 references matching regex 'xyz.*'"
    );
}

#[test]
fn test_search_references_score_exact_match() {
    let (db_file, _conn) = create_test_db_with_references();

    let options = SearchOptions {
        db_path: db_file.path(),
        query: "test_func",
        path_filter: None,
        kind_filter: None,
        limit: 100,
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

    let (result, _partial) = search_references(options).expect("search_references should succeed");
    assert_eq!(result.results.len(), 1);
    assert_eq!(
        result.results[0].score,
        Some(100),
        "Exact match should have score 100"
    );
}

#[test]
fn test_search_references_limit() {
    let (db_file, _conn) = create_test_db_with_references();

    let options = SearchOptions {
        db_path: db_file.path(),
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

    let (result, _partial) = search_references(options).expect("search_references should succeed");
    assert_eq!(
        result.results.len(),
        1,
        "Limit should restrict results to 1"
    );
}

#[test]
fn test_search_references_total_count() {
    let (db_file, _conn) = create_test_db_with_references();

    let options = SearchOptions {
        db_path: db_file.path(),
        query: "test_func",
        path_filter: None,
        kind_filter: None,
        limit: 100,
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

    let (result, _partial) = search_references(options).expect("search_references should succeed");
    assert_eq!(result.total_count, 1, "Total count should be 1");
}

#[test]
fn test_search_references_path_filter() {
    let (db_file, _conn) = create_test_db_with_references();

    let path_filter = PathBuf::from("/test/file.rs");
    let options = SearchOptions {
        db_path: db_file.path(),
        query: "test_func",
        path_filter: Some(&path_filter),
        kind_filter: None,
        limit: 100,
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

    let (result, _partial) = search_references(options).expect("search_references should succeed");
    assert_eq!(
        result.results.len(),
        1,
        "Should find 1 reference in /test/file.rs"
    );
    assert_eq!(result.results[0].span.file_path, "/test/file.rs");
}

#[test]
fn test_search_references_include_score() {
    let (db_file, _conn) = create_test_db_with_references();

    let options = SearchOptions {
        db_path: db_file.path(),
        query: "test_func",
        path_filter: None,
        kind_filter: None,
        limit: 100,
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

    let (result, _partial) = search_references(options).expect("search_references should succeed");
    assert_eq!(result.results.len(), 1);
    assert!(
        result.results[0].score.is_some(),
        "Score should be included when include_score=true"
    );
}

#[test]
fn test_search_references_ordering() {
    let (db_file, _conn) = create_test_db_with_references();

    let options = SearchOptions {
        db_path: db_file.path(),
        query: "test",
        path_filter: None,
        kind_filter: None,
        limit: 100,
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

    let (result, _partial) = search_references(options).expect("search_references should succeed");
    for i in 1..result.results.len() {
        let prev_score = result.results[i - 1].score.unwrap_or(0);
        let curr_score = result.results[i].score.unwrap_or(0);
        assert!(
            prev_score >= curr_score,
            "Results should be sorted by score descending"
        );
    }
}
