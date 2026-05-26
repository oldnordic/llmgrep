use super::*;
use rusqlite::Connection;
use std::path::PathBuf;

#[test]
fn test_symbol_id_lookup_returns_single_result() {
    let (_db_file, _conn) = create_test_db();
    let db_path = _db_file.path();

    let options = SearchOptions {
        db_path,
        query: "unused",
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
        symbol_id: Some("sym1"),
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
        "Should find exactly 1 result by symbol_id"
    );
    assert_eq!(response.results[0].name, "test_func");
    assert_eq!(response.results[0].symbol_id.as_deref(), Some("sym1"));
}

#[test]
fn test_fqn_pattern_filter() {
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
        fqn_pattern: Some("/test/file.rs%"),
        exact_fqn: None,
        language_filter: None,
        coverage_filter: None,
    };

    let (response, _partial, _) = search_symbols(options).expect("search_symbols should succeed");
    assert!(
        !response.results.is_empty(),
        "Should find symbols matching FQN pattern"
    );
}

#[test]
fn test_exact_fqn_filter() {
    let (_db_file, _conn) = create_test_db();
    let db_path = _db_file.path();

    let options = SearchOptions {
        db_path,
        query: "",
        path_filter: None,
        kind_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions {
            fqn: false,
            canonical_fqn: true,
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
        exact_fqn: Some("/test/file.rs::test_func"),
        language_filter: None,
        coverage_filter: None,
    };

    let (response, _partial, _) = search_symbols(options).expect("search_symbols should succeed");
    assert_eq!(
        response.results.len(),
        1,
        "Should find exactly 1 result by exact FQN"
    );
    assert_eq!(response.results[0].name, "test_func");
    assert_eq!(
        response.results[0].canonical_fqn.as_deref(),
        Some("/test/file.rs::test_func")
    );
}

#[test]
fn test_symbol_id_included_in_json_output() {
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
        fqn: FqnOptions {
            fqn: false,
            canonical_fqn: true,
            display_fqn: true,
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

    let (response, _partial, _) = search_symbols(options).expect("search_symbols should succeed");
    for result in &response.results {
        assert!(
            result.symbol_id.is_some(),
            "symbol_id should be present in results"
        );
        assert!(
            result.canonical_fqn.is_some(),
            "canonical_fqn should be present when requested"
        );
        assert!(
            result.display_fqn.is_some(),
            "display_fqn should be present when requested"
        );
    }
}

#[test]
fn test_ambiguity_detection_with_duplicate_names() {
    let db_file = tempfile::NamedTempFile::new().expect("failed to create temp file");
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
        "CREATE TABLE graph_edges (
            id INTEGER PRIMARY KEY,
            from_id INTEGER NOT NULL,
            to_id INTEGER NOT NULL,
            edge_type TEXT NOT NULL
        )",
        [],
    )
    .expect("failed to execute SQL");
    conn.execute(
        "CREATE TABLE symbol_metrics (
            symbol_id TEXT PRIMARY KEY,
            fan_in INTEGER,
            fan_out INTEGER,
            cyclomatic_complexity INTEGER,
            loc INTEGER
        )",
        [],
    )
    .expect("failed to execute SQL");

    conn.execute(
        "INSERT INTO graph_entities (id, kind, data) VALUES (1, 'File', '{\"path\":\"/test/a.rs\"}')",
        [],
    ).expect("failed to execute SQL");
    conn.execute(
        "INSERT INTO graph_entities (id, kind, data) VALUES (2, 'File', '{\"path\":\"/test/b.rs\"}')",
        [],
    ).expect("failed to execute SQL");

    conn.execute(
        "INSERT INTO graph_entities (id, kind, data) VALUES
            (10, 'Symbol', '{\"name\":\"parse\",\"kind\":\"Function\",\"display_fqn\":\"parse\",\"fqn\":\"a::parse\",\"canonical_fqn\":\"/test/a.rs::parse\",\"symbol_id\":\"parse_a\",\"byte_start\":100,\"byte_end\":200,\"start_line\":5,\"start_col\":0,\"end_line\":10,\"end_col\":1}'),
            (11, 'Symbol', '{\"name\":\"parse\",\"kind\":\"Function\",\"display_fqn\":\"parse\",\"fqn\":\"b::parse\",\"canonical_fqn\":\"/test/b.rs::parse\",\"symbol_id\":\"parse_b\",\"byte_start\":300,\"byte_end\":400,\"start_line\":15,\"start_col\":0,\"end_line\":20,\"end_col\":1}')",
        [],
    ).expect("failed to execute SQL");

    conn.execute(
        "INSERT INTO graph_edges (from_id, to_id, edge_type) VALUES (1, 10, 'DEFINES'), (2, 11, 'DEFINES')",
        [],
    ).expect("failed to execute SQL");

    let db_path = db_file.path();

    let options = SearchOptions {
        db_path,
        query: "parse",
        path_filter: None,
        kind_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions {
            fqn: false,
            canonical_fqn: true,
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

    let (response, _partial, _) = search_symbols(options).expect("search_symbols should succeed");
    assert_eq!(
        response.results.len(),
        2,
        "Should find both 'parse' symbols"
    );
    let fqns: Vec<_> = response
        .results
        .iter()
        .filter_map(|r| r.canonical_fqn.as_ref())
        .collect();
    assert_eq!(fqns.len(), 2, "Should have 2 different FQNs");
}

#[test]
fn test_symbol_id_bypasses_ambiguity() {
    let db_file = tempfile::NamedTempFile::new().expect("failed to create temp file");
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
        "CREATE TABLE graph_edges (
            id INTEGER PRIMARY KEY,
            from_id INTEGER NOT NULL,
            to_id INTEGER NOT NULL,
            edge_type TEXT NOT NULL
        )",
        [],
    )
    .expect("failed to execute SQL");
    conn.execute(
        "CREATE TABLE symbol_metrics (
            symbol_id TEXT PRIMARY KEY,
            fan_in INTEGER,
            fan_out INTEGER,
            cyclomatic_complexity INTEGER,
            loc INTEGER
        )",
        [],
    )
    .expect("failed to execute SQL");

    conn.execute(
        "INSERT INTO graph_entities (id, kind, data) VALUES (1, 'File', '{\"path\":\"/test/a.rs\"}')",
        [],
    ).expect("failed to execute SQL");

    conn.execute(
        "INSERT INTO graph_entities (id, kind, data) VALUES
            (10, 'Symbol', '{\"name\":\"parse\",\"kind\":\"Function\",\"display_fqn\":\"parse\",\"fqn\":\"a::parse\",\"canonical_fqn\":\"/test/a.rs::parse\",\"symbol_id\":\"target_parse\",\"byte_start\":100,\"byte_end\":200,\"start_line\":5,\"start_col\":0,\"end_line\":10,\"end_col\":1}'),
            (11, 'Symbol', '{\"name\":\"parse\",\"kind\":\"Function\",\"display_fqn\":\"parse\",\"fqn\":\"b::parse\",\"canonical_fqn\":\"/test/b.rs::parse\",\"symbol_id\":\"other_parse\",\"byte_start\":300,\"byte_end\":400,\"start_line\":15,\"start_col\":0,\"end_line\":20,\"end_col\":1}')",
        [],
    ).expect("failed to execute SQL");

    conn.execute(
        "INSERT INTO graph_edges (from_id, to_id, edge_type) VALUES (1, 10, 'DEFINES')",
        [],
    )
    .expect("failed to execute SQL");

    let db_path = db_file.path();

    let options = SearchOptions {
        db_path,
        query: "ignored",
        path_filter: None,
        kind_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions {
            fqn: false,
            canonical_fqn: true,
            display_fqn: false,
        },
        include_score: false,
        sort_by: SortMode::default(),
        metrics: MetricsOptions::default(),
        ast: AstOptions::default(),
        depth: DepthOptions::default(),
        algorithm: AlgorithmOptions::default(),
        symbol_id: Some("target_parse"),
        fqn_pattern: None,
        exact_fqn: None,
        language_filter: None,
        coverage_filter: None,
    };

    let (response, _partial, _) = search_symbols(options).expect("search_symbols should succeed");
    assert_eq!(
        response.results.len(),
        1,
        "Should find exactly 1 result by symbol_id"
    );
    assert_eq!(
        response.results[0].symbol_id.as_deref(),
        Some("target_parse")
    );
    assert_eq!(
        response.results[0].canonical_fqn.as_deref(),
        Some("/test/a.rs::parse")
    );
}

#[test]
fn test_infer_language_from_extension() {
    assert_eq!(infer_language("src/main.rs"), Some("Rust"));
    assert_eq!(infer_language("lib/app.py"), Some("Python"));
    assert_eq!(infer_language("component.js"), Some("JavaScript"));
    assert_eq!(infer_language("module.ts"), Some("TypeScript"));
    assert_eq!(infer_language("header.h"), Some("C"));
    assert_eq!(infer_language("impl.cpp"), Some("C++"));
    assert_eq!(infer_language("Main.java"), Some("Java"));
    assert_eq!(infer_language("main.go"), Some("Go"));

    assert_eq!(infer_language("App.jsx"), Some("JavaScript"));
    assert_eq!(infer_language("App.tsx"), Some("TypeScript"));

    assert_eq!(infer_language("file.xyz"), None);
    assert_eq!(infer_language("README"), None);
    assert_eq!(infer_language("no_extension"), None);
}

#[test]
fn test_normalize_kind_label() {
    assert_eq!(normalize_kind_label("Function"), "function");
    assert_eq!(normalize_kind_label("STRUCT"), "struct");
    assert_eq!(normalize_kind_label("Method"), "method");
    assert_eq!(normalize_kind_label("Class"), "class");
    assert_eq!(normalize_kind_label("enum"), "enum");
}

#[test]
fn test_build_search_query_with_language_filter() {
    let (sql, params, _strategy) = build_search_query(
        "test",
        None,
        None,
        Some("rust"),
        false,
        false,
        100,
        MetricsOptions::default(),
        SortMode::default(),
        None,
        None,
        None,
        false,
        &[],
        None,
        None,
        None,
        None,
        None,
        false,
        None,
        false,
    );

    assert!(sql.contains("f.file_path LIKE ? ESCAPE '\\'"));
    assert_eq!(params.len(), 5);
}

#[test]
fn test_build_search_query_with_unknown_language() {
    let (_sql, params, _) = build_search_query(
        "test",
        None,
        None,
        Some("unknown_language"),
        false,
        false,
        100,
        MetricsOptions::default(),
        SortMode::default(),
        None,
        None,
        None,
        false,
        &[],
        None,
        None,
        None,
        None,
        None,
        false,
        None,
        false,
    );

    assert_eq!(params.len(), 4);
}

#[test]
fn test_build_search_query_combined_language_and_kind() {
    let path = PathBuf::from("/src/module");
    let (sql, params, _strategy) = build_search_query(
        "test",
        Some(&path),
        Some("Function"),
        Some("python"),
        false,
        false,
        100,
        MetricsOptions::default(),
        SortMode::default(),
        None,
        None,
        None,
        false,
        &[],
        None,
        None,
        None,
        None,
        None,
        false,
        None,
        false,
    );

    assert!(sql.contains("f.file_path LIKE ? ESCAPE '\\'"));
    assert!(sql.contains("s.kind_normalized = ? OR s.kind = ?"));
    assert_eq!(params.len(), 8);
    assert_eq!(count_params(&sql), 8);
}

#[test]
fn test_build_search_query_with_cpp_language() {
    let (sql, params, _strategy) = build_search_query(
        "test",
        None,
        None,
        Some("cpp"),
        false,
        false,
        100,
        MetricsOptions::default(),
        SortMode::default(),
        None,
        None,
        None,
        false,
        &[],
        None,
        None,
        None,
        None,
        None,
        false,
        None,
        false,
    );

    assert!(sql.contains("f.file_path LIKE ? ESCAPE '\\'"));
    assert_eq!(params.len(), 5);
}

fn count_params(sql: &str) -> usize {
    sql.matches('?').count()
}
