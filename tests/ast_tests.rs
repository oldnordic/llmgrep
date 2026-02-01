//! AST module tests
//!
//! Tests for AST filtering and backward compatibility.

use llmgrep::ast::{check_ast_table_exists, AstContext, ast_nodes_table_schema};
use llmgrep::query::{
    search_symbols, AstOptions, ContextOptions, DepthOptions, FqnOptions, MetricsOptions,
    SearchOptions, SnippetOptions,
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
            ast_kinds: vec!["function_item".to_string()],
            with_ast_context: false,
            _phantom: std::marker::PhantomData,
        },
        depth: DepthOptions::default(),
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
            ast_kinds: vec!["function_item".to_string()],
            with_ast_context: false,
            _phantom: std::marker::PhantomData,
        },
        depth: DepthOptions::default(),
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
        depth: DepthOptions::default(),
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
            ast_kinds: vec!["call_expression".to_string()],
            with_ast_context: false,
            _phantom: std::marker::PhantomData,
        },
        depth: DepthOptions::default(),
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
            ast_kinds: vec!["function_item".to_string()],
            with_ast_context: false,
            _phantom: std::marker::PhantomData,
        },
        depth: DepthOptions::default(),
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
            ast_kinds: vec![],
            with_ast_context: true, // Enable enriched context
            _phantom: std::marker::PhantomData,
        },
        depth: DepthOptions::default(),
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
            ast_kinds: vec![],
            with_ast_context: false, // NOT enabled
            _phantom: std::marker::PhantomData,
        },
        depth: DepthOptions::default(),
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
        depth: DepthOptions::default(),
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

// ============================================================================
// Phase 9 Tests: Depth and Structural Search
// ============================================================================

// Test 1: test_calculate_decision_depth (already exists in src/ast.rs)

// Test 2: test_min_depth_filter
#[test]
fn test_min_depth_filter() {
    let temp_dir = TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db_with_ast(&db_path);

    // Insert file
    let file_id = 1i64;
    insert_file(&conn, file_id, "src/test.rs");

    // Create AST nodes at different decision depths
    conn.execute(
        "INSERT INTO ast_nodes (id, parent_id, kind, byte_start, byte_end) VALUES
        (100, NULL, 'function_item', 100, 500),  -- depth 0 (root level function)
        (101, 100, 'let_declaration', 150, 200),
        (102, 100, 'if_expression', 250, 350),  -- depth 1 (inside function)
        (103, 102, 'loop_expression', 260, 340), -- depth 2 (inside if)
        (104, 100, 'let_declaration', 400, 450), -- depth 0 (sibling of if)
        (105, 100, 'match_expression', 460, 490), -- depth 1 (inside function)
        (106, 105, 'if_expression', 470, 480), -- depth 2 (inside match)
        (107, 106, 'loop_expression', 472, 478), -- depth 3 (inside if, inside match)
        (108, 100, 'let_declaration', 500, 520)  -- depth 0 (sibling)",
        [],
    )
    .expect("insert ast nodes");

    // Insert symbols matching the AST nodes
    for ast_id in [100, 102, 103, 105, 106, 107] {
        insert_symbol(&conn, ast_id, &format!("symbol_{}", ast_id), "Function", file_id);
        insert_define_edge(&conn, file_id, ast_id);
    }

    // Search with min_depth=2 should return only symbols at depth 2 or 3
    let options = SearchOptions {
        db_path: &db_path,
        query: "symbol_",
        path_filter: None,
        kind_filter: None,
        limit: 100,
        use_regex: false,
        candidates: 100,
        context: ContextOptions::default(),
        snippet: SnippetOptions::default(),
        fqn: FqnOptions::default(),
        include_score: true,
        sort_by: llmgrep::SortMode::default(),
        metrics: MetricsOptions::default(),
        ast: AstOptions::default(),
        depth: DepthOptions {
            min_depth: Some(2),
            max_depth: None,
            inside: None,
            contains: None,
        },
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
        language_filter: None,
    };

    let (response, _partial) = search_symbols(options).expect("search should succeed");

    // Should find symbols at depth >= 2
    // symbol_103 (loop inside if) = depth 2
    // symbol_107 (if inside match) = depth 2  
    // symbol_108 (loop inside if inside match) = depth 3
    assert!(
        response.results.len() >= 2,
        "Should find at least 2 symbols with depth >= 2"
    );
}

// Test 3: test_max_depth_filter
#[test]
fn test_max_depth_filter() {
    let temp_dir = TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db_with_ast(&db_path);

    // Insert file
    let file_id = 1i64;
    insert_file(&conn, file_id, "src/test.rs");

    // Create AST nodes at different depths
    conn.execute(
        "INSERT INTO ast_nodes (id, parent_id, kind, byte_start, byte_end) VALUES
        (100, NULL, 'function_item', 100, 600),
        (101, 100, 'if_expression', 150, 250),   -- depth 1
        (102, 101, 'loop_expression', 160, 240), -- depth 2
        (103, 102, 'match_expression', 170, 230), -- depth 3
        (104, 103, 'if_expression', 180, 220),   -- depth 4
        (105, 100, 'let_declaration', 260, 300)",
        [],
    )
    .expect("insert ast nodes");

    // Insert symbols - use "test" prefix so query matches both
    insert_symbol(&conn, 100, "test_func_depth0", "Function", file_id);
    insert_define_edge(&conn, file_id, 100);
    insert_symbol(&conn, 101, "test_if_depth1", "Function", file_id);
    insert_define_edge(&conn, file_id, 101);
    insert_symbol(&conn, 102, "test_loop_depth2", "Function", file_id);
    insert_define_edge(&conn, file_id, 102);

    // Search with max_depth=1 should return only symbols at depth <= 1
    let options = SearchOptions {
        db_path: &db_path,
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
        sort_by: llmgrep::SortMode::default(),
        metrics: MetricsOptions::default(),
        ast: AstOptions::default(),
        depth: DepthOptions {
            min_depth: None,
            max_depth: Some(1),
            inside: None,
            contains: None,
        },
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
        language_filter: None,
    };

    let (response, _partial) = search_symbols(options).expect("search should succeed");

    // Should find test_func_depth0 (depth 0) and test_if_depth1 (depth 1)
    // But NOT test_loop_depth2 (depth 2)
    assert!(
        response.results.len() >= 2,
        "Should find at least 2 symbols with depth <= 1"
    );
    let names: Vec<&str> = response.results.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"test_func_depth0"), "Should include depth 0 symbol");
    assert!(names.contains(&"test_if_depth1"), "Should include depth 1 symbol");
}

// Test 4: test_min_max_depth_range
#[test]
fn test_min_max_depth_range() {
    let temp_dir = TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db_with_ast(&db_path);

    // Insert file
    let file_id = 1i64;
    insert_file(&conn, file_id, "src/test.rs");

    // Create AST nodes at depths 0-4
    conn.execute(
        "INSERT INTO ast_nodes (id, parent_id, kind, byte_start, byte_end) VALUES
        (100, NULL, 'function_item', 100, 800),
        (101, 100, 'if_expression', 150, 250),   -- depth 1
        (102, 101, 'loop_expression', 160, 240), -- depth 2
        (103, 102, 'match_expression', 170, 230), -- depth 3
        (104, 103, 'if_expression', 180, 220),   -- depth 4
        (105, 100, 'let_declaration', 260, 300)",
        [],
    )
    .expect("insert ast nodes");

    // Insert symbols for each depth level
    for ast_id in [100, 101, 102, 103, 104] {
        insert_symbol(&conn, ast_id, &format!("depth{}", ast_id - 100), "Function", file_id);
        insert_define_edge(&conn, file_id, ast_id);
    }

    // Search with min_depth=1, max_depth=2
    let options = SearchOptions {
        db_path: &db_path,
        query: "depth",
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
        depth: DepthOptions {
            min_depth: Some(1),
            max_depth: Some(2),
            inside: None,
            contains: None,
        },
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
        language_filter: None,
    };

    let (response, _partial) = search_symbols(options).expect("search should succeed");

    // Should find depth1 (depth 1) and depth2 (depth 2)
    // But NOT depth0 (depth 0) or depth3/depth4 (depth 3+)
    assert!(
        response.results.len() >= 2,
        "Should find symbols in range [1, 2]"
    );
}

// Test 5: test_inside_function_item
#[test]
fn test_inside_function_item() {
    let temp_dir = TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db_with_ast(&db_path);

    // Insert file
    let file_id = 1i64;
    insert_file(&conn, file_id, "src/test.rs");

    // Create AST structure: function -> block -> closure_expression
    conn.execute(
        "INSERT INTO ast_nodes (id, parent_id, kind, byte_start, byte_end) VALUES
        (100, NULL, 'function_item', 100, 600),
        (101, 100, 'block', 150, 550),
        (102, 101, 'closure_expression', 200, 250),
        (103, 101, 'let_declaration', 260, 300),
        (104, NULL, 'function_item', 700, 900),  -- Different function, no closure
        (105, 104, 'closure_expression', 750, 850),  -- Closure in different function
        (106, 101, 'call_expression', 310, 350)",
        [],
    )
    .expect("insert ast nodes");

    // Insert symbols
    insert_symbol(&conn, 102, "closure_inside_func", "Function", file_id);
    insert_define_edge(&conn, file_id, 102);
    insert_symbol(&conn, 103, "let_inside_func", "Function", file_id);
    insert_define_edge(&conn, file_id, 103);
    insert_symbol(&conn, 105, "closure_other", "Function", file_id);
    insert_define_edge(&conn, file_id, 105);

    // Search for closures inside function_item
    let options = SearchOptions {
        db_path: &db_path,
        query: "closure",
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
            ast_kinds: vec!["closure_expression".to_string()],
            with_ast_context: false,
            _phantom: std::marker::PhantomData,
        },
        depth: DepthOptions {
            min_depth: None,
            max_depth: None,
            inside: Some("function_item"),
            contains: None,
        },
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
        language_filter: None,
    };

    let (response, _partial) = search_symbols(options).expect("search should succeed");

    // Should find closure_inside_func (inside function with id 100)
    // And closure_other (inside function with id 104)
    // But NOT the let_declaration which is also inside function_item
    assert!(
        response.results.len() >= 2,
        "Should find at least 2 closure_expression nodes inside function_item"
    );

    let names: Vec<&str> = response.results.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"closure_inside_func"), "Should find closure in first function");
    assert!(names.contains(&"closure_other"), "Should find closure in other function");
}

// Test 6: test_inside_block
#[test]
fn test_inside_block() {
    let temp_dir = TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db_with_ast(&db_path);

    // Insert file
    let file_id = 1i64;
    insert_file(&conn, file_id, "src/test.rs");

    // Create AST structure: function -> block -> let_declaration
    conn.execute(
        "INSERT INTO ast_nodes (id, parent_id, kind, byte_start, byte_end) VALUES
        (100, NULL, 'function_item', 100, 500),
        (101, 100, 'block', 150, 450),
        (102, 101, 'let_declaration', 200, 250),
        (103, 100, 'let_declaration', 460, 480),  -- Let at function level (not in block)
        (104, 101, 'call_expression', 300, 350)",
        [],
    )
    .expect("insert ast nodes");

    // Insert symbols
    insert_symbol(&conn, 102, "let_in_block", "Function", file_id);
    insert_define_edge(&conn, file_id, 102);
    insert_symbol(&conn, 103, "let_at_func_level", "Function", file_id);
    insert_define_edge(&conn, file_id, 103);

    // Search for let_declarations inside block
    let options = SearchOptions {
        db_path: &db_path,
        query: "let",
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
            ast_kinds: vec!["let_declaration".to_string()],
            with_ast_context: false,
            _phantom: std::marker::PhantomData,
        },
        depth: DepthOptions {
            min_depth: None,
            max_depth: None,
            inside: Some("block"),
            contains: None,
        },
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
        language_filter: None,
    };

    let (response, _partial) = search_symbols(options).expect("search should succeed");

    // Should find let_in_block (inside block)
    // Should NOT find let_at_func_level (direct child of function, not block)
    assert!(
        response.results.len() >= 1,
        "Should find let_declaration inside block"
    );

    let names: Vec<&str> = response.results.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"let_in_block"), "Should include let_in_block");
}

// Test 7: test_contains_if_expression
#[test]
fn test_contains_if_expression() {
    let temp_dir = TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db_with_ast(&db_path);

    // Insert file
    let file_id = 1i64;
    insert_file(&conn, file_id, "src/test.rs");

    // Create AST: function with if_expression child
    conn.execute(
        "INSERT INTO ast_nodes (id, parent_id, kind, byte_start, byte_end) VALUES
        (100, NULL, 'function_item', 100, 400),
        (101, 100, 'if_expression', 150, 300),
        (102, 101, 'let_declaration', 160, 200),
        (200, NULL, 'function_item', 500, 700), -- Function without if
        (201, 200, 'let_declaration', 510, 550),
        (300, NULL, 'function_item', 800, 1000), -- Function with multiple if expressions
        (301, 300, 'if_expression', 850, 950),
        (302, 300, 'if_expression', 960, 990)",
        [],
    )
    .expect("insert ast nodes");

    // Insert symbols
    insert_symbol(&conn, 100, "func_with_if", "Function", file_id);
    insert_define_edge(&conn, file_id, 100);
    insert_symbol(&conn, 200, "func_plain", "Function", file_id);
    insert_define_edge(&conn, file_id, 200);
    insert_symbol(&conn, 300, "func_with_multiple_ifs", "Function", file_id);
    insert_define_edge(&conn, file_id, 300);

    // Search for functions containing if_expression
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
            ast_kinds: vec!["function_item".to_string()],
            with_ast_context: false,
            _phantom: std::marker::PhantomData,
        },
        depth: DepthOptions {
            min_depth: None,
            max_depth: None,
            inside: None,
            contains: Some("if_expression"),
        },
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
        language_filter: None,
    };

    let (response, _partial) = search_symbols(options).expect("search should succeed");

    // Should find func_with_if and func_with_multiple_ifs
    // Should NOT find func_plain (no if_expression children)
    assert!(
        response.results.len() >= 2,
        "Should find functions containing if_expression"
    );

    let names: Vec<&str> = response.results.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"func_with_if"), "Should include func_with_if");
    assert!(names.contains(&"func_with_multiple_ifs"), "Should include func_with_multiple_ifs");
}

// Test 8: test_contains_multiple_children
#[test]
fn test_contains_multiple_children() {
    let temp_dir = TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let _conn = setup_db_with_ast(&db_path);

    // Insert file
    let file_id = 1i64;
    insert_file(&_conn, file_id, "src/test.rs");

    // Create AST: function with multiple call_expression children
    _conn.execute(
        "INSERT INTO ast_nodes (id, parent_id, kind, byte_start, byte_end) VALUES
        (100, NULL, 'function_item', 100, 800),
        (101, 100, 'call_expression', 150, 200),
        (102, 100, 'call_expression', 250, 300),
        (103, 100, 'call_expression', 350, 400),
        (104, 100, 'let_declaration', 450, 500),
        (200, NULL, 'function_item', 900, 1200), -- Function with single call
        (201, 200, 'call_expression', 950, 1000),
        (300, NULL, 'function_item', 1300, 1500), -- Function with no calls
        (301, 300, 'let_declaration', 1350, 1400)",
        [],
    )
    .expect("insert ast nodes");

    // Insert symbols
    insert_symbol(&_conn, 100, "func_many_calls", "Function", file_id);
    insert_define_edge(&_conn, file_id, 100);
    insert_symbol(&_conn, 200, "func_one_call", "Function", file_id);
    insert_define_edge(&_conn, file_id, 200);
    insert_symbol(&_conn, 300, "func_no_calls", "Function", file_id);
    insert_define_edge(&_conn, file_id, 300);

    // Search for functions containing call_expression
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
            ast_kinds: vec!["function_item".to_string()],
            with_ast_context: false,
            _phantom: std::marker::PhantomData,
        },
        depth: DepthOptions {
            min_depth: None,
            max_depth: None,
            inside: None,
            contains: Some("call_expression"),
        },
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
        language_filter: None,
    };

    let (response, _partial) = search_symbols(options).expect("search should succeed");

    // Should find func_many_calls and func_one_call
    // Should NOT find func_no_calls
    assert!(
        response.results.len() >= 2,
        "Should find functions with call_expression children"
    );

    let names: Vec<&str> = response.results.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"func_many_calls"), "Should include func_many_calls");
    assert!(names.contains(&"func_one_call"), "Should include func_one_call");
}

// Test 9: test_combined_depth_and_inside
#[test]
fn test_combined_depth_and_inside() {
    let temp_dir = TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db_with_ast(&db_path);

    // Insert file
    let file_id = 1i64;
    insert_file(&conn, file_id, "src/test.rs");

    // Create nested structure with varying depths
    conn.execute(
        "INSERT INTO ast_nodes (id, parent_id, kind, byte_start, byte_end) VALUES
        (100, NULL, 'function_item', 100, 1000),  -- func_outer, depth 0
        (101, 100, 'if_expression', 150, 800),     -- depth 1
        (102, 101, 'block', 160, 700),          -- depth 1
        (103, 102, 'closure_expression', 200, 300), -- depth 1, inside block
        (104, 101, 'if_expression', 350, 600),     -- depth 2, inside outer if
        (105, 104, 'match_expression', 400, 500),    -- depth 3, inside nested if
        (106, 102, 'let_declaration', 310, 320),     -- depth 1, inside block
        (107, 100, 'let_declaration', 820, 850),     -- depth 0, at function level
        (108, NULL, 'function_item', 1100, 1500), -- func_other, depth 0
        (109, 108, 'if_expression', 1200, 1400),    -- depth 1
        (110, 108, 'let_declaration', 1250, 1300),    -- depth 1,
        (111, 109, 'closure_expression', 1300, 1350),   -- depth 2, inside if
        (112, 109, 'block', 1210, 1390),            -- depth 1
        (113, 112, 'let_declaration', 1310, 1320),   -- depth 1, inside block
        (114, 112, 'closure_expression', 1330, 1340),   -- depth 1, inside block inside if
        (115, NULL, 'block', 2000, 2500),            -- orphan block, depth 0
        (116, 115, 'if_expression', 2100, 2200),       -- depth 1, inside orphan block
        (117, 115, 'let_declaration', 2300, 2350)",
        [],
    )
    .expect("insert ast nodes");

    // Insert symbols for closures at various depths
    for ast_id in [103, 111, 114, 116] {
        insert_symbol(&conn, ast_id, &format!("closure_{}", ast_id), "Function", file_id);
        insert_define_edge(&conn, file_id, ast_id);
    }

    // Search for closures at depth >= 1 inside function_item
    let options = SearchOptions {
        db_path: &db_path,
        query: "closure",
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
            ast_kinds: vec!["closure_expression".to_string()],
            with_ast_context: false,
            _phantom: std::marker::PhantomData,
        },
        depth: DepthOptions {
            min_depth: Some(1),  // At depth 1 or deeper
            max_depth: None,
            inside: Some("function_item"), // Inside function_item
            contains: None,
        },
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
        language_filter: None,
    };

    let (response, _partial) = search_symbols(options).expect("search should succeed");

    // Should find:
    // - closure_103 (depth 1, inside block inside func_outer)
    // - closure_111 (depth 2, inside if inside func_other)
    // Should NOT find:
    // - closure_114 (depth 1, inside block inside if, but block is NOT inside function_item directly)
    // - closure_116 (depth 1, inside orphan block, no function parent)
    assert!(
        response.results.len() >= 2,
        "Should find closures at depth >= 1 inside function_item"
    );

    let names: Vec<&str> = response.results.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"closure_103"), "Should include closure_103");
}

// Test 10: test_backward_compat_no_depth_filter
#[test]
fn test_backward_compat_no_depth_filter() {
    let temp_dir = TempDir::new().expect("tempdir");
    let db_path = temp_dir.path().join("test.db");
    let conn = setup_db_with_ast(&db_path);

    // Insert file
    let file_id = 1i64;
    insert_file(&conn, file_id, "src/test.rs");

    // Create AST nodes
    conn.execute(
        "INSERT INTO ast_nodes (id, parent_id, kind, byte_start, byte_end) VALUES
        (100, NULL, 'function_item', 100, 500),
        (101, 100, 'let_declaration', 150, 200),
        (102, 100, 'if_expression', 250, 350)",
        [],
    )
    .expect("insert ast nodes");

    // Insert symbols
    insert_symbol(&conn, 100, "my_function", "Function", file_id);
    insert_define_edge(&conn, file_id, 100);

    // Search without depth flags - should work as before
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
        ast: AstOptions::default(),
        depth: DepthOptions::default(), // No depth filtering
        symbol_id: None,
        fqn_pattern: None,
        exact_fqn: None,
        language_filter: None,
    };

    let (response, _partial) = search_symbols(options).expect("search should succeed");

    assert_eq!(response.results.len(), 1);
    assert_eq!(response.results[0].name, "my_function");
    // ast_context should be present but depth should NOT be populated
    let ast_ctx = response.results[0]
        .ast_context
        .as_ref()
        .expect("ast_context should be present");
    assert_eq!(ast_ctx.depth, None, "Depth should not be populated without depth filtering or --with-ast-context");
}
