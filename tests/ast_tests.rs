//! AST module tests
//!
//! Tests for AST filtering and backward compatibility.

use llmgrep::ast::{check_ast_table_exists, AstContext, ast_nodes_table_schema};
use llmgrep::query::{
    search_symbols, AstOptions, ContextOptions, FqnOptions, MetricsOptions, SearchOptions,
    SnippetOptions,
};
use rusqlite::{params, Connection};
use std::path::PathBuf;
use tempfile::TempDir;

fn setup_db_with_ast(path: &std::path::Path) -> Connection {
    let conn = Connection::open(path).expect("open db");

    // Create graph_entities table
    conn.execute_batch(
        "CREATE TABLE graph_entities (
            id INTEGER PRIMARY KEY,
            kind TEXT NOT NULL,
            name TEXT,
            data TEXT
        );
        CREATE TABLE graph_edges (
            id INTEGER PRIMARY KEY,
            from_id INTEGER,
            to_id INTEGER,
            edge_type TEXT
        );
        CREATE TABLE symbol_metrics (
            symbol_id INTEGER PRIMARY KEY,
            fan_in INTEGER,
            fan_out INTEGER,
            cyclomatic_complexity INTEGER
        );",
    )
    .expect("create base tables");

    // Create ast_nodes table
    conn.execute(ast_nodes_table_schema(), [])
        .expect("create ast_nodes table");

    conn
}

fn insert_symbol(conn: &Connection, id: i64, name: &str, kind: &str, file_id: i64) {
    conn.execute(
        "INSERT INTO graph_entities (id, kind, name, data) VALUES (?1, 'Symbol', ?2, ?3)",
        params![
            id,
            name,
            format!(
                r#"{{"name":"{}","kind":"{}","symbol_id":"{:040x}","byte_start":0,"byte_end":100,"start_line":1,"start_col":0,"end_line":1,"end_col":100}}"#,
                name,
                kind,
                id
            )
        ],
    )
    .expect("insert symbol");
}

fn insert_file(conn: &Connection, id: i64, path: &str) {
    conn.execute(
        "INSERT INTO graph_entities (id, kind, data) VALUES (?1, 'File', ?2)",
        params![id, format!(r#"{{"path":"{}"}}"#, path)],
    )
    .expect("insert file");
}

fn insert_define_edge(conn: &Connection, from_id: i64, to_id: i64) {
    conn.execute(
        "INSERT INTO graph_edges (from_id, to_id, edge_type) VALUES (?1, ?2, 'DEFINES')",
        params![from_id, to_id],
    )
    .expect("insert edge");
}

fn insert_ast_node(
    conn: &Connection,
    id: i64,
    kind: &str,
    parent_id: Option<i64>,
    byte_start: u64,
    byte_end: u64,
) {
    conn.execute(
        "INSERT INTO ast_nodes (id, kind, parent_id, byte_start, byte_end) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, kind, parent_id, byte_start, byte_end],
    )
    .expect("insert ast node");
}

#[test]
fn test_check_ast_table_exists() {
    let temp_dir = TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");

    // Test with ast_nodes table
    let conn = setup_db_with_ast(&db_path);
    assert!(
        check_ast_table_exists(&conn).unwrap(),
        "Should return true when ast_nodes table exists"
    );
    drop(conn);

    // Test without ast_nodes table
    let conn2 = Connection::open(&db_path).expect("open db");
    conn2
        .execute("DROP TABLE ast_nodes", [])
        .expect("drop ast_nodes");
    assert!(
        !check_ast_table_exists(&conn2).unwrap(),
        "Should return false when ast_nodes table doesn't exist"
    );
}

#[test]
fn test_ast_context_serialization() {
    let ctx = AstContext {
        ast_id: 123,
        kind: "function_item".to_string(),
        parent_id: Some(122),
        byte_start: 100,
        byte_end: 200,
    };

    let json = serde_json::to_string(&ctx).unwrap();
    assert!(json.contains(r#""ast_id":123"#));
    assert!(json.contains(r#""kind":"function_item""#));
    assert!(json.contains(r#""parent_id":122"#));
    assert!(json.contains(r#""byte_start":100"#));
    assert!(json.contains(r#""byte_end":200"#));
}

#[test]
fn test_ast_context_without_parent() {
    let ctx = AstContext {
        ast_id: 1,
        kind: "mod_item".to_string(),
        parent_id: None,
        byte_start: 0,
        byte_end: 50,
    };

    let json = serde_json::to_string(&ctx).unwrap();
    assert!(json.contains(r#""parent_id":null"#));
}

#[test]
fn test_ast_kind_filter() {
    let temp_dir = TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db_with_ast(&db_path);

    // Insert file
    insert_file(&conn, 1, "src/lib.rs");

    // Insert symbols
    insert_symbol(&conn, 10, "my_function", "Function", 1);
    insert_symbol(&conn, 11, "my_block", "Block", 1);
    insert_symbol(&conn, 12, "my_call", "Call", 1);
    insert_define_edge(&conn, 1, 10);
    insert_define_edge(&conn, 1, 11);
    insert_define_edge(&conn, 1, 12);

    // Insert AST nodes with different kinds
    insert_ast_node(&conn, 10, "function_item", None, 0, 100);
    insert_ast_node(&conn, 11, "block", Some(10), 10, 90);
    insert_ast_node(&conn, 12, "call_expression", Some(11), 20, 80);

    // Search for function_item only
    let options = SearchOptions {
        db_path: &db_path,
        query: "my_",
        path_filter: None,
        kind_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions::default(),
        include_score: true,
        sort_by: llmgrep::SortMode::default(),
        metrics: MetricsOptions::default(),
        ast: AstOptions {
            ast_kind: Some("function_item"),
        },
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
        language_filter: None,
    };

    let (response, _partial) = search_symbols(options).expect("search should succeed");
    assert_eq!(
        response.results.len(),
        1,
        "Should only return function_item kind"
    );
    assert_eq!(response.results[0].name, "my_function");
    assert_eq!(
        response.results[0].ast_context.as_ref().unwrap().kind,
        "function_item"
    );
}

#[test]
fn test_backward_compat_no_ast_table() {
    let temp_dir = TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = Connection::open(&db_path).expect("open db");

    // Create basic schema without ast_nodes table (but with symbol_metrics for compatibility)
    conn.execute_batch(
        "CREATE TABLE graph_entities (
            id INTEGER PRIMARY KEY,
            kind TEXT NOT NULL,
            name TEXT,
            data TEXT
        );
        CREATE TABLE graph_edges (
            id INTEGER PRIMARY KEY,
            from_id INTEGER,
            to_id INTEGER,
            edge_type TEXT
        );
        CREATE TABLE symbol_metrics (
            symbol_id INTEGER PRIMARY KEY,
            fan_in INTEGER,
            fan_out INTEGER,
            cyclomatic_complexity INTEGER
        );",
    )
    .expect("create base tables");

    // Insert file and symbol
    insert_file(&conn, 1, "src/lib.rs");
    insert_symbol(&conn, 10, "my_function", "Function", 1);
    insert_define_edge(&conn, 1, 10);

    // Verify table doesn't exist
    assert!(
        !check_ast_table_exists(&conn).unwrap(),
        "ast_nodes table should not exist"
    );

    // Search with ast_kind filter (should be gracefully ignored)
    let options = SearchOptions {
        db_path: &db_path,
        query: "my_function",
        path_filter: None,
        kind_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions::default(),
        include_score: true,
        sort_by: llmgrep::SortMode::default(),
        metrics: MetricsOptions::default(),
        ast: AstOptions {
            ast_kind: Some("function_item"),
        },
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
        language_filter: None,
    };

    let (response, _partial) = search_symbols(options).expect("search should succeed");
    assert_eq!(response.results.len(), 1);
    assert_eq!(response.results[0].name, "my_function");
    // ast_context should be None when table doesn't exist
    assert!(
        response.results[0].ast_context.is_none(),
        "ast_context should be None when ast_nodes table doesn't exist"
    );
}

#[test]
fn test_ast_context_population() {
    let temp_dir = TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db_with_ast(&db_path);

    // Insert file and symbol
    insert_file(&conn, 1, "src/lib.rs");
    insert_symbol(&conn, 10, "parent_function", "Function", 1);
    insert_define_edge(&conn, 1, 10);

    // Insert AST node with parent
    insert_ast_node(&conn, 10, "function_item", Some(5), 100, 200);

    // Search
    let options = SearchOptions {
        db_path: &db_path,
        query: "parent_function",
        path_filter: None,
        kind_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions::default(),
        include_score: true,
        sort_by: llmgrep::SortMode::default(),
        metrics: MetricsOptions::default(),
        ast: AstOptions::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
        language_filter: None,
    };

    let (response, _partial) = search_symbols(options).expect("search should succeed");
    assert_eq!(response.results.len(), 1);

    let ast_ctx = response.results[0]
        .ast_context
        .as_ref()
        .expect("ast_context should be present");

    assert_eq!(ast_ctx.ast_id, 10);
    assert_eq!(ast_ctx.kind, "function_item");
    assert_eq!(ast_ctx.parent_id, Some(5));
    assert_eq!(ast_ctx.byte_start, 100);
    assert_eq!(ast_ctx.byte_end, 200);
}

#[test]
fn test_multiple_ast_kinds() {
    let temp_dir = TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db_with_ast(&db_path);

    // Insert file
    insert_file(&conn, 1, "src/lib.rs");

    // Insert symbols with different AST kinds
    insert_symbol(&conn, 10, "func_one", "Function", 1);
    insert_symbol(&conn, 11, "func_two", "Function", 1);
    insert_define_edge(&conn, 1, 10);
    insert_define_edge(&conn, 1, 11);

    insert_ast_node(&conn, 10, "function_item", None, 0, 100);
    insert_ast_node(&conn, 11, "function_item", None, 100, 200);

    // Test filtering by call_expression (should return none since we only have functions)
    let options = SearchOptions {
        db_path: &db_path,
        query: "func",
        path_filter: None,
        kind_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions::default(),
        include_score: true,
        sort_by: llmgrep::SortMode::default(),
        metrics: MetricsOptions::default(),
        ast: AstOptions {
            ast_kind: Some("call_expression"),
        },
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
        language_filter: None,
    };

    let (response, _partial) = search_symbols(options).expect("search should succeed");
    assert_eq!(
        response.results.len(),
        0,
        "Should return no results for call_expression kind"
    );

    // Test filtering by function_item (should return both)
    let options = SearchOptions {
        db_path: &db_path,
        query: "func",
        path_filter: None,
        kind_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions::default(),
        include_score: true,
        sort_by: llmgrep::SortMode::default(),
        metrics: MetricsOptions::default(),
        ast: AstOptions {
            ast_kind: Some("function_item"),
        },
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
        language_filter: None,
    };

    let (response, _partial) = search_symbols(options).expect("search should succeed");
    assert_eq!(
        response.results.len(),
        2,
        "Should return both function_item results"
    );
}

#[test]
fn test_ast_nodes_table_schema() {
    let schema = ast_nodes_table_schema();
    assert!(schema.contains("CREATE TABLE ast_nodes"));
    assert!(schema.contains("id INTEGER PRIMARY KEY"));
    assert!(schema.contains("parent_id"));
    assert!(schema.contains("kind TEXT NOT NULL"));
    assert!(schema.contains("byte_start INTEGER NOT NULL"));
    assert!(schema.contains("byte_end INTEGER NOT NULL"));
}
