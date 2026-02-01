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
        depth: None,
        parent_kind: None,
        children_count_by_kind: None,
        decision_points: None,
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
        depth: None,
        parent_kind: None,
        children_count_by_kind: None,
        decision_points: None,
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
            with_ast_context: false,
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
            with_ast_context: false,
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
            with_ast_context: false,
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
            with_ast_context: false,
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

// Test: Calculate AST depth for nested nodes
#[test]
fn test_calculate_ast_depth() {
    use llmgrep::ast::calculate_ast_depth;

    let temp_dir = TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = Connection::open(&db_path).expect("open db");

    // Create ast_nodes table
    conn.execute(ast_nodes_table_schema(), []).expect("create ast_nodes");

    // Create a tree structure:
    //   id=1: root (parent_id=NULL) -> depth 0
    //   id=2: child of 1 -> depth 1
    //   id=3: child of 2 -> depth 2
    conn.execute(
        "INSERT INTO ast_nodes (id, parent_id, kind, byte_start, byte_end) VALUES
        (1, NULL, 'mod_item', 0, 1000),
        (2, 1, 'function_item', 100, 500),
        (3, 2, 'block', 150, 450)",
        [],
    )
    .expect("insert nodes");

    // Test depth calculation
    assert_eq!(calculate_ast_depth(&conn, 1).unwrap().unwrap(), 0, "Root should have depth 0");
    assert_eq!(calculate_ast_depth(&conn, 2).unwrap().unwrap(), 1, "Child should have depth 1");
    assert_eq!(calculate_ast_depth(&conn, 3).unwrap().unwrap(), 2, "Grandchild should have depth 2");
}

// Test: Get parent kind for AST nodes
#[test]
fn test_get_parent_kind() {
    use llmgrep::ast::get_parent_kind;

    let temp_dir = TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = Connection::open(&db_path).expect("open db");

    conn.execute(ast_nodes_table_schema(), []).expect("create ast_nodes");

    conn.execute(
        "INSERT INTO ast_nodes (id, parent_id, kind, byte_start, byte_end) VALUES
        (1, NULL, 'mod_item', 0, 1000),
        (2, 1, 'function_item', 100, 500)",
        [],
    )
    .expect("insert nodes");

    // Test parent kind lookup
    assert_eq!(
        get_parent_kind(&conn, Some(1)).unwrap().unwrap(),
        "mod_item",
        "Parent kind should be mod_item"
    );
    assert_eq!(
        get_parent_kind(&conn, None).unwrap(),
        None,
        "None parent_id should return None"
    );
    assert_eq!(
        get_parent_kind(&conn, Some(999)).unwrap(),
        None,
        "Non-existent parent should return None"
    );
}

// Test: Count children by kind
#[test]
fn test_count_children_by_kind() {
    use llmgrep::ast::count_children_by_kind;

    let temp_dir = TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = Connection::open(&db_path).expect("open db");

    conn.execute(ast_nodes_table_schema(), []).expect("create ast_nodes");

    // Create a node with multiple child types
    conn.execute(
        "INSERT INTO ast_nodes (id, parent_id, kind, byte_start, byte_end) VALUES
        (1, NULL, 'function_item', 0, 1000),
        (2, 1, 'let_declaration', 100, 150),
        (3, 1, 'let_declaration', 150, 200),
        (4, 1, 'let_declaration', 200, 250),
        (5, 1, 'if_expression', 250, 400),
        (6, 1, 'call_expression', 300, 350),
        (7, 1, 'call_expression', 350, 380)",
        [],
    )
    .expect("insert nodes");

    let counts = count_children_by_kind(&conn, 1).unwrap();
    assert_eq!(counts.get("let_declaration"), Some(&3), "Should have 3 let_declaration");
    assert_eq!(counts.get("if_expression"), Some(&1), "Should have 1 if_expression");
    assert_eq!(counts.get("call_expression"), Some(&2), "Should have 2 call_expression");
}

// Test: Count decision points
#[test]
fn test_count_decision_points() {
    use llmgrep::ast::count_decision_points;

    let temp_dir = TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = Connection::open(&db_path).expect("open db");

    conn.execute(ast_nodes_table_schema(), []).expect("create ast_nodes");

    // Create a node with decision points (2-7 are decision points, 8-9 are not)
    conn.execute(
        "INSERT INTO ast_nodes (id, parent_id, kind, byte_start, byte_end) VALUES
        (1, NULL, 'function_item', 0, 1000),
        (2, 1, 'if_expression', 100, 200),
        (3, 1, 'match_expression', 200, 300),
        (4, 1, 'while_expression', 300, 400),
        (5, 1, 'for_expression', 400, 500),
        (6, 1, 'loop_expression', 500, 600),
        (7, 1, 'conditional_expression', 600, 650),
        (8, 1, 'let_declaration', 650, 700),
        (9, 1, 'call_expression', 700, 750)",
        [],
    )
    .expect("insert nodes");

    let decision_points = count_decision_points(&conn, 1).unwrap();
    assert_eq!(decision_points, 6, "Should count 6 decision points");
}

// Test: Enriched AST context in search results
#[test]
fn test_with_ast_context_flag() {
    let temp_dir = TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db_with_ast(&db_path);

    // Insert file
    let file_id = 1i64;
    insert_file(&conn, file_id, "src/test.rs");

    // Create AST nodes with nested structure
    conn.execute(
        "INSERT INTO ast_nodes (id, parent_id, kind, byte_start, byte_end) VALUES
        (1, NULL, 'mod_item', 0, 1000),
        (2, 1, 'function_item', 100, 500),
        (3, 2, 'let_declaration', 150, 200),
        (4, 2, 'if_expression', 200, 350),
        (5, 2, 'call_expression', 360, 400)",
        [],
    )
    .expect("insert ast nodes");

    // Insert symbol and edge
    insert_symbol(&conn, 100, "my_function", "Function", file_id);
    insert_define_edge(&conn, file_id, 100);
    conn.execute(
        "UPDATE graph_entities SET data = json_object(
            'byte_start', 150,
            'byte_end', 400,
            'start_line', 10,
            'start_col', 0,
            'end_line', 25,
            'end_col', 1,
            'kind', 'Function',
            'name', 'my_function'
        ) WHERE id = 100",
        [],
    )
    .expect("update symbol data");

    // Update symbol_metrics to match symbol_id
    conn.execute(
        "INSERT INTO symbol_metrics (symbol_id, fan_in, fan_out, cyclomatic_complexity)
        VALUES (100, 5, 3, 2)",
        [],
    )
    .expect("insert metrics");

    // Search with --with-ast-context flag
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
            ast_kind: None,
            with_ast_context: true, // Enable enriched context
        },
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
        language_filter: None,
    };

    let (response, _partial) = search_symbols(options).expect("search should succeed");
    assert_eq!(response.results.len(), 1, "Should find the function");

    let ast_ctx = response.results[0]
        .ast_context
        .as_ref()
        .expect("Should have ast_context");

    // Check basic fields
    assert_eq!(ast_ctx.kind, "function_item");

    // Check enriched fields (should be populated with --with-ast-context)
    assert_eq!(ast_ctx.depth, Some(1), "Depth should be populated");
    assert_eq!(
        ast_ctx.parent_kind.as_ref().map(|s| s.as_str()),
        Some("mod_item"),
        "Parent kind should be populated"
    );
    assert!(
        ast_ctx.children_count_by_kind.is_some(),
        "Children count should be populated"
    );
    assert_eq!(
        ast_ctx.decision_points,
        Some(1), // One if_expression
        "Decision points should be counted"
    );
}

// Test: Backward compatibility - ast_context without flag
#[test]
fn test_ast_context_without_flag() {
    let temp_dir = TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db_with_ast(&db_path);

    // Insert file
    let file_id = 1i64;
    insert_file(&conn, file_id, "src/test.rs");

    // Create AST nodes - note: ID must match symbol ID (100) for the LEFT JOIN to work
    conn.execute(
        "INSERT INTO ast_nodes (id, parent_id, kind, byte_start, byte_end) VALUES
        (100, NULL, 'function_item', 100, 500)",
        [],
    )
    .expect("insert ast nodes");

    // Insert symbol and edge
    insert_symbol(&conn, 100, "my_function", "Function", file_id);
    insert_define_edge(&conn, file_id, 100);
    conn.execute(
        "UPDATE graph_entities SET data = json_object(
            'byte_start', 150,
            'byte_end', 400,
            'start_line', 10,
            'start_col', 0,
            'end_line', 25,
            'end_col', 1,
            'kind', 'Function',
            'name', 'my_function'
        ) WHERE id = 100",
        [],
    )
    .expect("update symbol data");

    // Update symbol_metrics
    conn.execute(
        "INSERT INTO symbol_metrics (symbol_id, fan_in, fan_out, cyclomatic_complexity)
        VALUES (100, 5, 3, 2)",
        [],
    )
    .expect("insert metrics");

    // Search WITHOUT --with-ast-context flag
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
            ast_kind: None,
            with_ast_context: false, // NOT enabled
        },
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
        language_filter: None,
    };

    let (response, _partial) = search_symbols(options).expect("search should succeed");
    assert_eq!(response.results.len(), 1, "Should find the function");

    let ast_ctx = response.results[0]
        .ast_context
        .as_ref()
        .expect("Should have basic ast_context from LEFT JOIN");

    // Basic fields should be present
    assert_eq!(ast_ctx.kind, "function_item");

    // Enriched fields should NOT be populated without flag
    assert_eq!(ast_ctx.depth, None, "Depth should not be populated");
    assert_eq!(ast_ctx.parent_kind, None, "Parent kind should not be populated");
    assert_eq!(
        ast_ctx.children_count_by_kind, None,
        "Children count should not be populated"
    );
    assert_eq!(
        ast_ctx.decision_points, None,
        "Decision points should not be populated"
    );
}

// Test: Sorting by AstComplexity (same as Complexity)
#[test]
fn test_sort_by_ast_complexity() {
    let temp_dir = TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db_with_ast(&db_path);

    // Insert file
    let file_id = 1i64;
    insert_file(&conn, file_id, "src/test.rs");

    // Insert symbols with different complexities
    insert_symbol(&conn, 100, "simple_func", "Function", file_id);
    insert_define_edge(&conn, file_id, 100);
    conn.execute(
        "UPDATE graph_entities SET data = json_object(
            'byte_start', 100, 'byte_end', 150,
            'start_line', 10, 'start_col', 0, 'end_line', 12, 'end_col', 1,
            'kind', 'Function', 'name', 'simple_func'
        ) WHERE id = 100",
        [],
    )
    .expect("update symbol 1");

    insert_symbol(&conn, 200, "complex_func", "Function", file_id);
    insert_define_edge(&conn, file_id, 200);
    conn.execute(
        "UPDATE graph_entities SET data = json_object(
            'byte_start', 200, 'byte_end', 300,
            'start_line', 20, 'start_col', 0, 'end_line', 25, 'end_col', 1,
            'kind', 'Function', 'name', 'complex_func'
        ) WHERE id = 200",
        [],
    )
    .expect("update symbol 2");

    // Insert metrics with different complexity values
    conn.execute(
        "INSERT INTO symbol_metrics (symbol_id, fan_in, fan_out, cyclomatic_complexity)
        VALUES (100, 1, 0, 1)",
        [],
    )
    .expect("insert metrics 1");

    conn.execute(
        "INSERT INTO symbol_metrics (symbol_id, fan_in, fan_out, cyclomatic_complexity)
        VALUES (200, 2, 1, 10)",
        [],
    )
    .expect("insert metrics 2");

    // Search with AstComplexity sort
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
        sort_by: llmgrep::SortMode::AstComplexity, // New sort mode
        metrics: MetricsOptions::default(),
        ast: AstOptions::default(),
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
        language_filter: None,
    };

    let (response, _partial) = search_symbols(options).expect("search should succeed");
    assert_eq!(response.results.len(), 2, "Should find both functions");

    // Should be sorted by complexity descending (complex_func first)
    assert_eq!(
        response.results[0].name,
        "complex_func",
        "Highest complexity should come first"
    );
    assert_eq!(
        response.results[1].name,
        "simple_func",
        "Lower complexity should come second"
    );
}
