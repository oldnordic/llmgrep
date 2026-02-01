use llmgrep::query::{
    search_symbols, AstOptions, ContextOptions, FqnOptions, MetricsOptions, SearchOptions, SnippetOptions,
};
/// Unit tests for v1.1 features - internal logic testing
///
/// Tests for:
/// - Safe extraction function edge cases
/// - Public API with new v1.1 options
use llmgrep::safe_extraction::extract_symbol_content_safe;
use llmgrep::SortMode;
use rusqlite::{params, Connection};
use serde_json::json;

fn setup_db(path: &std::path::Path) -> Connection {
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
        CREATE TABLE code_chunks (
            id INTEGER PRIMARY KEY,
            file_path TEXT NOT NULL,
            byte_start INTEGER NOT NULL,
            byte_end INTEGER NOT NULL,
            content TEXT NOT NULL,
            content_hash TEXT NOT NULL,
            symbol_name TEXT,
            symbol_kind TEXT
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

fn insert_metrics(conn: &Connection, symbol_row_id: i64, symbol_name: &str, kind: &str, file_path: &str, fan_in: i64, fan_out: i64, complexity: i64) {
    conn.execute(
        "INSERT INTO symbol_metrics (symbol_id, symbol_name, kind, file_path, loc, estimated_loc, fan_in, fan_out, cyclomatic_complexity, last_updated)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![symbol_row_id, symbol_name, kind, file_path, 0i64, 0.0f64, fan_in, fan_out, complexity, 0i64],
    )
    .expect("insert metrics");
}

/// Test 1: Safe extraction function - valid bounds
#[test]
fn test_extract_symbol_content_safe_valid_bounds() {
    let content = b"Hello, world!";
    // Valid bounds within content
    let result = extract_symbol_content_safe(content, 0, 5);
    assert_eq!(result, Some("Hello".to_string()));

    let result = extract_symbol_content_safe(content, 7, 12);
    assert_eq!(result, Some("world".to_string()));
}

/// Test 1b: Safe extraction function - invalid bounds
#[test]
fn test_extract_symbol_content_safe_invalid_bounds() {
    let content = b"Hello";

    // Start > end
    let result = extract_symbol_content_safe(content, 5, 0);
    assert_eq!(result, None);

    // Start beyond content length
    let result = extract_symbol_content_safe(content, 10, 15);
    assert_eq!(result, None);

    // End beyond content length
    let result = extract_symbol_content_safe(content, 0, 100);
    assert_eq!(result, None);
}

/// Test 1c: Safe extraction function - multi-byte boundary
#[test]
fn test_extract_symbol_content_safe_multi_byte_boundary() {
    // UTF-8 string with multi-byte characters
    // "Hello" = 5 bytes, "世界" = 6 bytes (3 each), "!" = 1 byte
    let content = "Hello世界!".as_bytes();

    // Extract up to safe boundary (end of "Hello")
    let result = extract_symbol_content_safe(content, 0, 5);
    assert_eq!(result, Some("Hello".to_string()));

    // Extract full content
    let result = extract_symbol_content_safe(content, 0, content.len());
    assert_eq!(result, Some("Hello世界!".to_string()));

    // Extract from middle through multi-byte chars
    let result = extract_symbol_content_safe(content, 5, content.len());
    assert_eq!(result, Some("世界!".to_string()));

    // Try to cut through multi-byte character (should return None or safe boundary)
    // "世" starts at byte 5, "界" starts at byte 8
    // If we end at byte 7 (middle of "世"), should return None or truncated
    let result = extract_symbol_content_safe(content, 5, 7);
    // Implementation should handle this gracefully
    assert!(result.is_none() || result.as_ref().unwrap().len() <= 2);
}

/// Test 1d: Safe extraction - emoji handling
#[test]
fn test_extract_symbol_content_safe_emoji() {
    // Test with emoji (4-byte UTF-8 sequences)
    let content = "fn test() { // 🚀🔥⭐\n  true\n}";
    let content_bytes = content.as_bytes();

    // Extract including emoji
    let result = extract_symbol_content_safe(content_bytes, 0, content_bytes.len());
    assert_eq!(result, Some(content.to_string()));

    // Verify emoji is preserved
    let result = result.unwrap();
    assert!(result.contains("🚀"));
    assert!(result.contains("🔥"));
    assert!(result.contains("⭐"));
}

/// Test 2: Public API - metrics filtering
#[test]
fn test_api_metrics_filtering() {
    let temp_dir = tempfile::TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db(&db_path);

    let file_path = "src/test.rs";
    let file_id = insert_file(&conn, file_path);

    // Insert symbols with different metrics
    let low_name = "low_complexity";
    let low_id = insert_symbol(&conn, low_name, "Function", "fn", (0, 15));
    insert_define_edge(&conn, file_id, low_id);
    insert_metrics(&conn, low_id, low_name, "Function", file_path, 1, 1, 1);

    let high_name = "high_complexity";
    let high_id = insert_symbol(&conn, high_name, "Function", "fn", (20, 40));
    insert_define_edge(&conn, file_id, high_id);
    insert_metrics(&conn, high_id, high_name, "Function", file_path, 10, 5, 20);

    // Test min_complexity filter
    let options = SearchOptions {
        db_path: &db_path,
        query: "complexity", // matches both
        path_filter: None,
        kind_filter: None,
        language_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions::default(),
        include_score: true,
        sort_by: SortMode::Relevance,
        metrics: MetricsOptions {
            min_complexity: Some(10),
            max_complexity: None,
            min_fan_in: None,
            min_fan_out: None,
        },
        ast: AstOptions::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
    };

    let response = search_symbols(options).expect("search should succeed");
    assert_eq!(response.0.results.len(), 1);
    assert_eq!(response.0.results[0].name, high_name);
}

/// Test 3: Public API - metrics sorting
#[test]
fn test_api_metrics_sorting() {
    let temp_dir = tempfile::TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db(&db_path);

    let file_path = "src/test.rs";
    let file_id = insert_file(&conn, file_path);

    // Insert symbols with different fan-in values
    let low_name = "low_fan";
    let low_id = insert_symbol(&conn, low_name, "Function", "fn", (0, 10));
    insert_define_edge(&conn, file_id, low_id);
    insert_metrics(&conn, low_id, low_name, "Function", file_path, 1, 0, 1);

    let high_name = "high_fan";
    let high_id = insert_symbol(&conn, high_name, "Function", "fn", (20, 30));
    insert_define_edge(&conn, file_id, high_id);
    insert_metrics(&conn, high_id, high_name, "Function", file_path, 100, 0, 1);

    // Test sort by fan-in (highest first)
    let options = SearchOptions {
        db_path: &db_path,
        query: "fan", // matches both
        path_filter: None,
        kind_filter: None,
        language_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions::default(),
        include_score: true,
        sort_by: SortMode::FanIn,
        metrics: MetricsOptions::default(),
        ast: AstOptions::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
    };

    let response = search_symbols(options).expect("search should succeed");
    assert_eq!(response.0.results.len(), 2);
    // Should be sorted by fan-in descending
    assert_eq!(response.0.results[0].name, high_name);
    assert_eq!(response.0.results[1].name, low_name);
}

/// Test 4: Public API - FQN field population
#[test]
fn test_api_fqn_field_population() {
    let temp_dir = tempfile::TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db(&db_path);

    let file_path = "src/test.rs";
    let file_id = insert_file(&conn, file_path);

    let symbol_name = "test_func";
    let symbol_id = insert_symbol(&conn, symbol_name, "Function", "fn", (0, 10));
    insert_define_edge(&conn, file_id, symbol_id);

    // Request all FQN fields
    let options = SearchOptions {
        db_path: &db_path,
        query: symbol_name,
        path_filter: None,
        kind_filter: None,
        language_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions {
            fqn: true,
            canonical_fqn: true,
            display_fqn: true,
        },
        include_score: true,
        sort_by: SortMode::Relevance,
        metrics: MetricsOptions::default(),
        ast: AstOptions::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
    };

    let response = search_symbols(options).expect("search should succeed");
    assert_eq!(response.0.results.len(), 1);

    let result = &response.0.results[0];
    assert!(result.fqn.is_some(), "fqn should be populated");
    assert!(
        result.canonical_fqn.is_some(),
        "canonical_fqn should be populated"
    );
    assert!(
        result.display_fqn.is_some(),
        "display_fqn should be populated"
    );
}

/// Test 5: Public API - language filtering
#[test]
fn test_api_language_filtering() {
    let temp_dir = tempfile::TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db(&db_path);

    // Insert Rust file
    let rust_file = "src/test.rs";
    let rust_file_id = insert_file(&conn, rust_file);
    let rust_fn = "test_func";
    let rust_id = insert_symbol(&conn, rust_fn, "Function", "fn", (0, 10));
    insert_define_edge(&conn, rust_file_id, rust_id);

    // Insert Python file
    let python_file = "src/test.py";
    let python_file_id = insert_file(&conn, python_file);
    let python_fn = "test_func";
    let python_id = insert_symbol(&conn, python_fn, "Function", "fn", (0, 10));
    insert_define_edge(&conn, python_file_id, python_id);

    // Filter for Rust only
    let options = SearchOptions {
        db_path: &db_path,
        query: "test_func",
        path_filter: None,
        kind_filter: None,
        language_filter: Some("rust"),
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions::default(),
        include_score: true,
        sort_by: SortMode::Relevance,
        metrics: MetricsOptions::default(),
        ast: AstOptions::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
    };

    let response = search_symbols(options).expect("search should succeed");
    assert_eq!(response.0.results.len(), 1);
    assert_eq!(response.0.results[0].language.as_ref().unwrap(), "rust");
}

/// Test 6: Public API - position mode sorting
#[test]
fn test_api_position_mode_sorting() {
    let temp_dir = tempfile::TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db(&db_path);

    let file_path = "src/test.rs";
    let file_id = insert_file(&conn, file_path);

    // Insert symbols at different positions
    let first_name = "first_func";
    let first_id = insert_symbol(&conn, first_name, "Function", "fn", (0, 10));
    insert_define_edge(&conn, file_id, first_id);

    let second_name = "second_func";
    let second_id = insert_symbol(&conn, second_name, "Function", "fn", (20, 30));
    insert_define_edge(&conn, file_id, second_id);

    let third_name = "third_func";
    let third_id = insert_symbol(&conn, third_name, "Function", "fn", (40, 50));
    insert_define_edge(&conn, file_id, third_id);

    // Use position mode
    let options = SearchOptions {
        db_path: &db_path,
        query: "func", // matches all
        path_filter: None,
        kind_filter: None,
        language_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions::default(),
        include_score: false, // Position mode doesn't use scores
        sort_by: SortMode::Position,
        metrics: MetricsOptions::default(),
        ast: AstOptions::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
    };

    let response = search_symbols(options).expect("search should succeed");
    assert_eq!(response.0.results.len(), 3);
    // Should be ordered by position (byte_start)
    assert_eq!(response.0.results[0].name, first_name);
    assert_eq!(response.0.results[1].name, second_name);
    assert_eq!(response.0.results[2].name, third_name);
}

/// Test 7: Public API - symbol_id lookup
#[test]
fn test_api_symbol_id_lookup() {
    let temp_dir = tempfile::TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db(&db_path);

    let file_path = "src/test.rs";
    let file_id = insert_file(&conn, file_path);

    let symbol_name = "unique_func";
    let known_symbol_id = "abc123def456789abc123def456789ab";
    let data = json!({
        "symbol_id": known_symbol_id,
        "name": symbol_name,
        "kind": "Function",
        "kind_normalized": "fn",
        "fqn": format!("test::{}", symbol_name),
        "display_fqn": symbol_name,
        "canonical_fqn": format!("test::src/test.rs::Function {}", symbol_name),
        "byte_start": 0,
        "byte_end": 12,
        "start_line": 1,
        "start_col": 0,
        "end_line": 1,
        "end_col": 5
    })
    .to_string();
    conn.execute(
        "INSERT INTO graph_entities (kind, name, data) VALUES (?1, ?2, ?3)",
        params!["Symbol", symbol_name, data],
    )
    .expect("insert symbol");
    let symbol_row_id = conn.last_insert_rowid();
    insert_define_edge(&conn, file_id, symbol_row_id);

    // Insert metrics for the symbol - using row ID reference
    insert_metrics(&conn, symbol_row_id, symbol_name, "Function", file_path, 5, 3, 2);

    // Search by SymbolId
    let options = SearchOptions {
        db_path: &db_path,
        query: "", // empty query
        path_filter: None,
        kind_filter: None,
        language_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions::default(),
        include_score: true,
        sort_by: SortMode::Relevance,
        metrics: MetricsOptions::default(),
        ast: AstOptions::default(),
        symbol_id: Some(known_symbol_id),
        fqn_pattern: None,
        exact_fqn: None,
    };

    let response = search_symbols(options).expect("search should succeed");
    assert_eq!(response.0.results.len(), 1);
    assert_eq!(response.0.results[0].name, symbol_name);
    assert_eq!(
        response.0.results[0].symbol_id.as_ref().unwrap(),
        known_symbol_id
    );
}

/// Test 8: Public API - FQN pattern filtering
#[test]
fn test_api_fqn_pattern_filtering() {
    let temp_dir = tempfile::TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db(&db_path);

    let file_path = "src/test.rs";
    let file_id = insert_file(&conn, file_path);

    // Insert two symbols with different FQNs
    let module_a = "helper_a";
    let data_a = json!({
        "symbol_id": format!("{}-id", module_a),
        "name": module_a,
        "kind": "Function",
        "kind_normalized": "fn",
        "fqn": "crate::module_a::helper_a",
        "display_fqn": "module_a::helper_a",
        "canonical_fqn": "crate::src/module_a.rs::Function helper_a",
        "byte_start": 0,
        "byte_end": 10,
        "start_line": 1,
        "start_col": 0,
        "end_line": 1,
        "end_col": 5
    })
    .to_string();
    conn.execute(
        "INSERT INTO graph_entities (kind, name, data) VALUES (?1, ?2, ?3)",
        params!["Symbol", module_a, data_a],
    )
    .expect("insert symbol");
    let id_a = conn.last_insert_rowid();
    insert_define_edge(&conn, file_id, id_a);

    let module_b = "helper_b";
    let data_b = json!({
        "symbol_id": format!("{}-id-b", module_b),
        "name": module_b,
        "kind": "Function",
        "kind_normalized": "fn",
        "fqn": "crate::module_b::helper_b",
        "display_fqn": "module_b::helper_b",
        "canonical_fqn": "crate::src/module_b.rs::Function helper_b",
        "byte_start": 20,
        "byte_end": 30,
        "start_line": 2,
        "start_col": 0,
        "end_line": 2,
        "end_col": 5
    })
    .to_string();
    conn.execute(
        "INSERT INTO graph_entities (kind, name, data) VALUES (?1, ?2, ?3)",
        params!["Symbol", module_b, data_b],
    )
    .expect("insert symbol");
    let id_b = conn.last_insert_rowid();
    insert_define_edge(&conn, file_id, id_b);

    // Filter by FQN pattern
    let options = SearchOptions {
        db_path: &db_path,
        query: "helper", // matches both
        path_filter: None,
        kind_filter: None,
        language_filter: None,
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
        include_score: true,
        sort_by: SortMode::Relevance,
        metrics: MetricsOptions::default(),
        ast: AstOptions::default(),
        symbol_id: None,
        fqn_pattern: Some("%module_a%"), // LIKE pattern
        exact_fqn: None,
    };

    let response = search_symbols(options).expect("search should succeed");
    assert_eq!(response.0.results.len(), 1);
    assert_eq!(response.0.results[0].name, module_a);
}
