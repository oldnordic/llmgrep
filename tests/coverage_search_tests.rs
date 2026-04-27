use llmgrep::query::{
    search_symbols, AstOptions, ContextOptions, CoverageFilter, DepthOptions, FqnOptions,
    MetricsOptions, SearchOptions, SnippetOptions,
};
use llmgrep::AlgorithmOptions;
use rusqlite::{params, Connection};
use serde_json::json;
use std::path::PathBuf;

fn setup_db_with_coverage(path: &std::path::Path) -> Connection {
    let conn = Connection::open(path).expect("open db");
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
        );
        CREATE TABLE cfg_blocks (
            id INTEGER PRIMARY KEY,
            function_id INTEGER NOT NULL,
            kind TEXT NOT NULL,
            coord_x INTEGER,
            coord_y INTEGER,
            coord_z INTEGER,
            start_line INTEGER,
            end_line INTEGER,
            FOREIGN KEY (function_id) REFERENCES graph_entities(id) ON DELETE CASCADE
        );
        CREATE TABLE cfg_block_coverage (
            block_id INTEGER PRIMARY KEY,
            hit_count INTEGER NOT NULL DEFAULT 0,
            source_kind TEXT NOT NULL,
            source_revision TEXT,
            ingested_at INTEGER NOT NULL,
            FOREIGN KEY (block_id) REFERENCES cfg_blocks(id) ON DELETE CASCADE
        );
        CREATE TABLE cfg_edges (
            id INTEGER PRIMARY KEY,
            function_id INTEGER NOT NULL,
            source_idx INTEGER NOT NULL,
            target_idx INTEGER NOT NULL,
            edge_type TEXT NOT NULL,
            FOREIGN KEY (function_id) REFERENCES graph_entities(id) ON DELETE CASCADE
        );
        CREATE TABLE cfg_edge_coverage (
            edge_id INTEGER PRIMARY KEY,
            hit_count INTEGER NOT NULL DEFAULT 0,
            source_kind TEXT NOT NULL,
            source_revision TEXT,
            ingested_at INTEGER NOT NULL,
            FOREIGN KEY (edge_id) REFERENCES cfg_edges(id) ON DELETE CASCADE
        );
        CREATE TABLE cfg_coverage_meta (
            source_kind TEXT PRIMARY KEY,
            source_revision TEXT,
            ingested_at INTEGER,
            total_blocks INTEGER,
            total_edges INTEGER
        );",
    )
    .expect("create tables");
    conn
}

fn insert_file(conn: &Connection, path: &str) -> i64 {
    let data = json!({
        "path": path,
        "hash": "sha256:deadbeef",
        "last_indexed_at": 0,
        "last_modified": 0
    })
    .to_string();
    conn.execute(
        "INSERT INTO graph_entities (kind, name, data) VALUES (?1, ?2, ?3)",
        params!["File", path, data],
    )
    .expect("insert file");
    conn.last_insert_rowid()
}

fn insert_symbol(
    conn: &Connection,
    name: &str,
    kind: &str,
    kind_normalized: &str,
    span: (u64, u64),
) -> i64 {
    let data = json!({
        "symbol_id": format!("{}-id", name),
        "name": name,
        "kind": kind,
        "kind_normalized": kind_normalized,
        "fqn": format!("test::{}", name),
        "display_fqn": name,
        "canonical_fqn": format!("test::src/lib.rs::{} {}", kind, name),
        "byte_start": span.0,
        "byte_end": span.1,
        "start_line": 1,
        "start_col": 0,
        "end_line": 1,
        "end_col": 5
    })
    .to_string();
    conn.execute(
        "INSERT INTO graph_entities (kind, name, data) VALUES (?1, ?2, ?3)",
        params!["Symbol", name, data],
    )
    .expect("insert symbol");
    conn.last_insert_rowid()
}

fn insert_define_edge(conn: &Connection, file_id: i64, symbol_id: i64) {
    conn.execute(
        "INSERT INTO graph_edges (from_id, to_id, edge_type) VALUES (?1, ?2, ?3)",
        params![file_id, symbol_id, "DEFINES"],
    )
    .expect("insert edge");
}

fn insert_cfg_block(conn: &Connection, function_id: i64, block_id: i64) {
    conn.execute(
        "INSERT INTO cfg_blocks (id, function_id, kind, coord_x, coord_y, coord_z, start_line, end_line)
         VALUES (?1, ?2, 'basic_block', 0, 0, 0, 1, 2)",
        params![block_id, function_id],
    )
    .expect("insert cfg block");
}

fn insert_coverage(conn: &Connection, block_id: i64, hit_count: i64) {
    conn.execute(
        "INSERT INTO cfg_block_coverage (block_id, hit_count, source_kind, source_revision, ingested_at)
         VALUES (?1, ?2, 'test', 'abc', 0)",
        params![block_id, hit_count],
    )
    .expect("insert coverage");
}

fn insert_cfg_edge(conn: &Connection, function_id: i64, edge_id: i64) {
    conn.execute(
        "INSERT INTO cfg_edges (id, function_id, source_idx, target_idx, edge_type)
         VALUES (?1, ?2, 0, 1, 'direct')",
        params![edge_id, function_id],
    )
    .expect("insert cfg edge");
}

fn insert_edge_coverage(conn: &Connection, edge_id: i64, hit_count: i64) {
    conn.execute(
        "INSERT INTO cfg_edge_coverage (edge_id, hit_count, source_kind, source_revision, ingested_at)
         VALUES (?1, ?2, 'test', 'abc', 0)",
        params![edge_id, hit_count],
    )
    .expect("insert edge coverage");
}

fn make_options(db_path: &'_ PathBuf) -> SearchOptions<'_> {
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
            lines: 0,
            max_lines: 20,
        },
        snippet: SnippetOptions {
            include: false,
            max_bytes: 0,
        },
        fqn: FqnOptions {
            fqn: false,
            canonical_fqn: false,
            display_fqn: false,
        },
        include_score: true,
        sort_by: llmgrep::SortMode::default(),
        metrics: MetricsOptions::default(),
        ast: AstOptions::default(),
        depth: DepthOptions::default(),
        algorithm: AlgorithmOptions::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
        coverage_filter: None,
    }
}

#[test]
fn test_search_uncovered_returns_zero_coverage_functions() {
    let temp_dir = tempfile::TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db_with_coverage(&db_path);

    let file_id = insert_file(&conn, "src/lib.rs");

    // Function with 2 blocks, 0 covered
    let fn1_id = insert_symbol(&conn, "uncovered_fn", "Function", "fn", (0, 10));
    insert_define_edge(&conn, file_id, fn1_id);
    insert_cfg_block(&conn, fn1_id, 1);
    insert_cfg_block(&conn, fn1_id, 2);
    insert_coverage(&conn, 1, 0);
    insert_coverage(&conn, 2, 0);

    // Function with 2 blocks, 1 covered
    let fn2_id = insert_symbol(&conn, "partially_covered_fn", "Function", "fn", (20, 30));
    insert_define_edge(&conn, file_id, fn2_id);
    insert_cfg_block(&conn, fn2_id, 3);
    insert_cfg_block(&conn, fn2_id, 4);
    insert_coverage(&conn, 3, 1);
    insert_coverage(&conn, 4, 0);

    let mut options = make_options(&db_path);
    options.query = "fn";
    options.coverage_filter = Some(CoverageFilter::Uncovered);

    let (response, _partial, _paths_bounded) = search_symbols(options).expect("search");

    assert_eq!(
        response.results.len(),
        1,
        "Should return exactly 1 uncovered function"
    );
    assert_eq!(response.results[0].name, "uncovered_fn");
    assert!(response.results[0].coverage.is_some());
    let cov = response.results[0].coverage.as_ref().unwrap();
    assert_eq!(cov.covered_blocks, 0);
    assert_eq!(cov.total_blocks, 2);
}

#[test]
fn test_search_covered_returns_nonzero_coverage_functions() {
    let temp_dir = tempfile::TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db_with_coverage(&db_path);

    let file_id = insert_file(&conn, "src/lib.rs");

    // Function with 2 blocks, 0 covered
    let fn1_id = insert_symbol(&conn, "uncovered_fn", "Function", "fn", (0, 10));
    insert_define_edge(&conn, file_id, fn1_id);
    insert_cfg_block(&conn, fn1_id, 1);
    insert_cfg_block(&conn, fn1_id, 2);
    insert_coverage(&conn, 1, 0);
    insert_coverage(&conn, 2, 0);

    // Function with 2 blocks, 1 covered
    let fn2_id = insert_symbol(&conn, "partially_covered_fn", "Function", "fn", (20, 30));
    insert_define_edge(&conn, file_id, fn2_id);
    insert_cfg_block(&conn, fn2_id, 3);
    insert_cfg_block(&conn, fn2_id, 4);
    insert_coverage(&conn, 3, 5);
    insert_coverage(&conn, 4, 0);

    let mut options = make_options(&db_path);
    options.query = "fn";
    options.coverage_filter = Some(CoverageFilter::Covered);

    let (response, _partial, _paths_bounded) = search_symbols(options).expect("search");

    assert_eq!(
        response.results.len(),
        1,
        "Should return exactly 1 covered function"
    );
    assert_eq!(response.results[0].name, "partially_covered_fn");
    assert!(response.results[0].coverage.is_some());
    let cov = response.results[0].coverage.as_ref().unwrap();
    assert_eq!(cov.covered_blocks, 1);
    assert_eq!(cov.total_blocks, 2);
}

#[test]
fn test_search_no_coverage_tables_filter_is_noop() {
    // Use the standard setup_db (without coverage tables) from search_tests.rs
    let temp_dir = tempfile::TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = Connection::open(&db_path).expect("open db");
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
    )
    .expect("create tables");

    let file_id = insert_file(&conn, "src/lib.rs");
    let fn1_id = insert_symbol(&conn, "some_fn", "Function", "fn", (0, 10));
    insert_define_edge(&conn, file_id, fn1_id);

    let mut options = make_options(&db_path);
    options.query = "some";
    options.coverage_filter = Some(CoverageFilter::Uncovered);

    let (response, _partial, _paths_bounded) = search_symbols(options).expect("search");

    // Without coverage tables, the filter is a no-op and all matching symbols are returned
    assert_eq!(response.results.len(), 1);
    assert_eq!(response.results[0].name, "some_fn");
    assert!(response.results[0].coverage.is_none());
}

#[test]
fn test_search_coverage_json_output() {
    let temp_dir = tempfile::TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db_with_coverage(&db_path);

    let file_id = insert_file(&conn, "src/lib.rs");
    let fn1_id = insert_symbol(&conn, "covered_fn", "Function", "fn", (0, 10));
    insert_define_edge(&conn, file_id, fn1_id);
    insert_cfg_block(&conn, fn1_id, 1);
    insert_cfg_block(&conn, fn1_id, 2);
    insert_coverage(&conn, 1, 5);
    insert_coverage(&conn, 2, 3);
    // Add edge coverage data
    insert_cfg_edge(&conn, fn1_id, 1);
    insert_cfg_edge(&conn, fn1_id, 2);
    insert_cfg_edge(&conn, fn1_id, 3);
    insert_edge_coverage(&conn, 1, 1);
    insert_edge_coverage(&conn, 2, 0);
    insert_edge_coverage(&conn, 3, 1);

    let mut options = make_options(&db_path);
    options.query = "covered";

    let (response, _partial, _paths_bounded) = search_symbols(options).expect("search");

    assert_eq!(response.results.len(), 1);
    let coverage = response.results[0]
        .coverage
        .as_ref()
        .expect("coverage should be present");
    assert_eq!(coverage.total_blocks, 2);
    assert_eq!(coverage.covered_blocks, 2);
    assert_eq!(coverage.block_percentage, 100.0);
    assert_eq!(coverage.total_edges, 3);
    assert_eq!(coverage.covered_edges, 2);
    assert!((coverage.edge_percentage - 66.66666666666667).abs() < 0.001);
}
