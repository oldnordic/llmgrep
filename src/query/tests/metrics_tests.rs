use super::*;
use rusqlite::Connection;

fn create_test_db_with_metrics() -> (tempfile::NamedTempFile, Connection) {
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
        )",
        [],
    )
    .expect("failed to execute SQL");

    conn.execute(
        "INSERT INTO graph_entities (id, kind, data) VALUES (1, 'File', '{\"path\":\"/test/file.rs\"}')",
        [],
    ).expect("failed to execute SQL");

    conn.execute(
        "INSERT INTO graph_entities (id, kind, data) VALUES
            (10, 'Symbol', '{\"name\":\"low_complexity\",\"kind\":\"Function\",\"kind_normalized\":\"function\",\"display_fqn\":\"low_complexity\",\"fqn\":\"module::low_complexity\",\"symbol_id\":\"sym1\",\"byte_start\":100,\"byte_end\":200,\"start_line\":5,\"start_col\":0,\"end_line\":10,\"end_col\":1}'),
            (11, 'Symbol', '{\"name\":\"med_complexity\",\"kind\":\"Function\",\"kind_normalized\":\"function\",\"display_fqn\":\"med_complexity\",\"fqn\":\"module::med_complexity\",\"symbol_id\":\"sym2\",\"byte_start\":300,\"byte_end\":400,\"start_line\":15,\"start_col\":0,\"end_line\":20,\"end_col\":1}'),
            (12, 'Symbol', '{\"name\":\"high_complexity\",\"kind\":\"Function\",\"kind_normalized\":\"function\",\"display_fqn\":\"high_complexity\",\"fqn\":\"module::high_complexity\",\"symbol_id\":\"sym3\",\"byte_start\":500,\"byte_end\":600,\"start_line\":25,\"start_col\":0,\"end_line\":30,\"end_col\":1}')",
        [],
    ).expect("failed to execute SQL");

    conn.execute(
        "INSERT INTO graph_edges (from_id, to_id, edge_type) VALUES (1, 10, 'DEFINES'), (1, 11, 'DEFINES'), (1, 12, 'DEFINES')",
        [],
    ).expect("failed to execute SQL");

    conn.execute(
        "INSERT INTO symbol_metrics (symbol_id, symbol_name, kind, file_path, loc, estimated_loc, fan_in, fan_out, cyclomatic_complexity, last_updated) VALUES
            (10, 'low_complexity', 'Function', '/test/file.rs', 50, 0.0, 10, 2, 5, 0),
            (11, 'med_complexity', 'Function', '/test/file.rs', 100, 0.0, 5, 8, 15, 0),
            (12, 'high_complexity', 'Function', '/test/file.rs', 150, 0.0, 2, 15, 25, 0)",
        [],
    ).expect("failed to execute SQL");

    (db_file, conn)
}

#[test]
fn test_metrics_filter_by_min_complexity() {
    let (_db_file, _conn) = create_test_db_with_metrics();
    let db_path = _db_file.path();

    let options = SearchOptions {
        db_path,
        query: "complexity",
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
        metrics: MetricsOptions {
            min_complexity: Some(10),
            max_complexity: None,
            min_fan_in: None,
            min_fan_out: None,
        },
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
        2,
        "Should find 2 results with complexity >= 10"
    );

    let names: Vec<&str> = response.results.iter().map(|r| r.name.as_str()).collect();
    assert!(
        names.contains(&"med_complexity"),
        "Should contain med_complexity"
    );
    assert!(
        names.contains(&"high_complexity"),
        "Should contain high_complexity"
    );
    assert!(
        !names.contains(&"low_complexity"),
        "Should not contain low_complexity"
    );
}

#[test]
fn test_metrics_filter_by_max_complexity() {
    let (_db_file, _conn) = create_test_db_with_metrics();
    let db_path = _db_file.path();

    let options = SearchOptions {
        db_path,
        query: "complexity",
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
        metrics: MetricsOptions {
            min_complexity: None,
            max_complexity: Some(10),
            min_fan_in: None,
            min_fan_out: None,
        },
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
        "Should find 1 result with complexity <= 10"
    );
    assert_eq!(response.results[0].name, "low_complexity");
}

#[test]
fn test_metrics_filter_combined_min_max_complexity() {
    let (_db_file, _conn) = create_test_db_with_metrics();
    let db_path = _db_file.path();

    let options = SearchOptions {
        db_path,
        query: "complexity",
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
        metrics: MetricsOptions {
            min_complexity: Some(10),
            max_complexity: Some(20),
            min_fan_in: None,
            min_fan_out: None,
        },
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
        "Should find 1 result with complexity in range [10, 20]"
    );
    assert_eq!(response.results[0].name, "med_complexity");
}

#[test]
fn test_metrics_filter_by_min_fan_in() {
    let (_db_file, _conn) = create_test_db_with_metrics();
    let db_path = _db_file.path();

    let options = SearchOptions {
        db_path,
        query: "complexity",
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
        metrics: MetricsOptions {
            min_complexity: None,
            max_complexity: None,
            min_fan_in: Some(8),
            min_fan_out: None,
        },
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
        "Should find 1 result with fan_in >= 8"
    );
    assert_eq!(response.results[0].name, "low_complexity");
    assert_eq!(
        response.results[0].fan_in,
        Some(10),
        "fan_in should be populated"
    );
}

#[test]
fn test_metrics_filter_by_min_fan_out() {
    let (_db_file, _conn) = create_test_db_with_metrics();
    let db_path = _db_file.path();

    let options = SearchOptions {
        db_path,
        query: "complexity",
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
        metrics: MetricsOptions {
            min_complexity: None,
            max_complexity: None,
            min_fan_in: None,
            min_fan_out: Some(10),
        },
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
        "Should find 1 result with fan_out >= 10"
    );
    assert_eq!(response.results[0].name, "high_complexity");
    assert_eq!(
        response.results[0].fan_out,
        Some(15),
        "fan_out should be populated"
    );
}

#[test]
fn test_metrics_sort_by_fan_in() {
    let (_db_file, _conn) = create_test_db_with_metrics();
    let db_path = _db_file.path();

    let options = SearchOptions {
        db_path,
        query: "complexity",
        path_filter: None,
        kind_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions::default(),
        include_score: false,
        sort_by: SortMode::FanIn,
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
    assert_eq!(response.results.len(), 3, "Should find all 3 results");

    assert_eq!(
        response.results[0].name, "low_complexity",
        "First should have highest fan_in"
    );
    assert_eq!(response.results[0].fan_in, Some(10));
    assert_eq!(
        response.results[1].name, "med_complexity",
        "Second should have medium fan_in"
    );
    assert_eq!(response.results[1].fan_in, Some(5));
    assert_eq!(
        response.results[2].name, "high_complexity",
        "Third should have lowest fan_in"
    );
    assert_eq!(response.results[2].fan_in, Some(2));
}

#[test]
fn test_metrics_sort_by_fan_out() {
    let (_db_file, _conn) = create_test_db_with_metrics();
    let db_path = _db_file.path();

    let options = SearchOptions {
        db_path,
        query: "complexity",
        path_filter: None,
        kind_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions::default(),
        include_score: false,
        sort_by: SortMode::FanOut,
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
    assert_eq!(response.results.len(), 3, "Should find all 3 results");

    assert_eq!(
        response.results[0].name, "high_complexity",
        "First should have highest fan_out"
    );
    assert_eq!(response.results[0].fan_out, Some(15));
    assert_eq!(
        response.results[1].name, "med_complexity",
        "Second should have medium fan_out"
    );
    assert_eq!(response.results[1].fan_out, Some(8));
    assert_eq!(
        response.results[2].name, "low_complexity",
        "Third should have lowest fan_out"
    );
    assert_eq!(response.results[2].fan_out, Some(2));
}

#[test]
fn test_metrics_sort_by_complexity() {
    let (_db_file, _conn) = create_test_db_with_metrics();
    let db_path = _db_file.path();

    let options = SearchOptions {
        db_path,
        query: "complexity",
        path_filter: None,
        kind_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions::default(),
        include_score: false,
        sort_by: SortMode::Complexity,
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
    assert_eq!(response.results.len(), 3, "Should find all 3 results");

    assert_eq!(
        response.results[0].name, "high_complexity",
        "First should have highest complexity"
    );
    assert_eq!(response.results[0].cyclomatic_complexity, Some(25));
    assert_eq!(
        response.results[1].name, "med_complexity",
        "Second should have medium complexity"
    );
    assert_eq!(response.results[1].cyclomatic_complexity, Some(15));
    assert_eq!(
        response.results[2].name, "low_complexity",
        "Third should have lowest complexity"
    );
    assert_eq!(response.results[2].cyclomatic_complexity, Some(5));
}

#[test]
fn test_metrics_fields_populated() {
    let (_db_file, _conn) = create_test_db_with_metrics();
    let db_path = _db_file.path();

    let options = SearchOptions {
        db_path,
        query: "low_complexity",
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
    assert_eq!(response.results.len(), 1);

    let result = &response.results[0];
    assert_eq!(result.name, "low_complexity");
    assert_eq!(result.fan_in, Some(10), "fan_in should be populated");
    assert_eq!(result.fan_out, Some(2), "fan_out should be populated");
    assert_eq!(
        result.cyclomatic_complexity,
        Some(5),
        "cyclomatic_complexity should be populated"
    );
    assert_eq!(
        result.complexity_score, None,
        "complexity_score is not available in symbol_metrics"
    );
}

#[test]
fn test_metrics_null_handling() {
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
    conn.execute(
        "CREATE TABLE symbol_metrics (
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
        )",
        [],
    )
    .expect("failed to execute SQL");

    conn.execute(
        "INSERT INTO graph_entities (id, kind, data) VALUES (1, 'File', '{\"path\":\"/test/file.rs\"}')",
        [],
    ).expect("failed to execute SQL");

    conn.execute(
        "INSERT INTO graph_entities (id, kind, data) VALUES
            (10, 'Symbol', '{\"name\":\"with_metrics\",\"kind\":\"Function\",\"kind_normalized\":\"function\",\"display_fqn\":\"with_metrics\",\"fqn\":\"module::with_metrics\",\"symbol_id\":\"sym1\",\"byte_start\":100,\"byte_end\":200,\"start_line\":5,\"start_col\":0,\"end_line\":10,\"end_col\":1}'),
            (11, 'Symbol', '{\"name\":\"no_metrics_1\",\"kind\":\"Function\",\"kind_normalized\":\"function\",\"display_fqn\":\"no_metrics_1\",\"fqn\":\"module::no_metrics_1\",\"symbol_id\":\"sym2\",\"byte_start\":300,\"byte_end\":400,\"start_line\":15,\"start_col\":0,\"end_line\":20,\"end_col\":1}'),
            (12, 'Symbol', '{\"name\":\"no_metrics_2\",\"kind\":\"Function\",\"kind_normalized\":\"function\",\"display_fqn\":\"no_metrics_2\",\"fqn\":\"module::no_metrics_2\",\"symbol_id\":\"sym3\",\"byte_start\":500,\"byte_end\":600,\"start_line\":25,\"start_col\":0,\"end_line\":30,\"end_col\":1}')",
        [],
    ).expect("failed to execute SQL");

    conn.execute(
        "INSERT INTO graph_edges (from_id, to_id, edge_type) VALUES (1, 10, 'DEFINES'), (1, 11, 'DEFINES'), (1, 12, 'DEFINES')",
        [],
    ).expect("failed to execute SQL");

    conn.execute(
        "INSERT INTO symbol_metrics (symbol_id, symbol_name, kind, file_path, loc, estimated_loc, fan_in, fan_out, cyclomatic_complexity, last_updated) VALUES
            (10, 'with_metrics', 'Function', '/test/file.rs', 50, 0.0, 10, 2, 5, 0)",
        [],
    ).expect("failed to execute SQL");

    let db_path = db_file.path();

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
        fqn: FqnOptions::default(),
        include_score: false,
        sort_by: SortMode::FanIn,
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
    assert_eq!(response.results.len(), 3, "Should find all 3 symbols");

    let with_metrics = response
        .results
        .iter()
        .find(|r| r.name == "with_metrics")
        .expect("result should be found");
    assert_eq!(
        with_metrics.fan_in,
        Some(10),
        "Symbol with metrics should have fan_in"
    );

    let no_metrics_1 = response
        .results
        .iter()
        .find(|r| r.name == "no_metrics_1")
        .expect("result should be found");
    assert_eq!(
        no_metrics_1.fan_in, None,
        "Symbol without metrics should have None for fan_in"
    );

    let no_metrics_2 = response
        .results
        .iter()
        .find(|r| r.name == "no_metrics_2")
        .expect("result should be found");
    assert_eq!(
        no_metrics_2.fan_in, None,
        "Symbol without metrics should have None for fan_in"
    );

    let options_filter = SearchOptions {
        db_path,
        query: "",
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
        metrics: MetricsOptions {
            min_fan_in: Some(5),
            ..Default::default()
        },
        ast: AstOptions::default(),
        depth: DepthOptions::default(),
        algorithm: AlgorithmOptions::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
        language_filter: None,
        coverage_filter: None,
    };

    let (response_filter, _, _) =
        search_symbols(options_filter).expect("search_symbols with filter should succeed");
    assert_eq!(
        response_filter.results.len(),
        1,
        "Should find only 1 symbol with fan_in >= 5"
    );
    assert_eq!(response_filter.results[0].name, "with_metrics");
}
