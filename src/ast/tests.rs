use super::*;

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
    assert!(json.contains("\"ast_id\":123"));
    assert!(json.contains("\"kind\":\"function_item\""));
    assert!(json.contains("\"parent_id\":122"));
    assert!(json.contains("\"byte_start\":100"));
    assert!(json.contains("\"byte_end\":200"));
    // Enriched fields should not appear in JSON when None (skip_serializing_if)
    assert!(!json.contains("depth"));
    assert!(!json.contains("parent_kind"));
    assert!(!json.contains("children_count_by_kind"));
    assert!(!json.contains("decision_points"));
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
    assert!(json.contains("\"parent_id\":null"));
}

#[test]
fn test_ast_context_enriched_serialization() {
    let mut children = HashMap::new();
    children.insert("let_declaration".to_string(), 3);
    children.insert("if_expression".to_string(), 2);

    let ctx = AstContext {
        ast_id: 42,
        kind: "function_item".to_string(),
        parent_id: None,
        byte_start: 1000,
        byte_end: 2000,
        depth: Some(0),
        parent_kind: None,
        children_count_by_kind: Some(children),
        decision_points: Some(2),
    };

    let json = serde_json::to_string(&ctx).unwrap();
    // Basic fields
    assert!(json.contains("\"ast_id\":42"));
    assert!(json.contains("\"kind\":\"function_item\""));
    // Enriched fields should appear when set
    assert!(json.contains("\"depth\":0"));
    assert!(json.contains("\"decision_points\":2"));
    assert!(json.contains("\"let_declaration\":3"));
    assert!(json.contains("\"if_expression\":2"));
    // parent_kind should not appear (None)
    assert!(!json.contains("parent_kind"));
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

#[test]
fn test_check_ast_table_exists_missing() {
    // Test with in-memory database (no tables)
    let conn = Connection::open_in_memory().unwrap();
    let result = check_ast_table_exists(&conn).unwrap();
    assert!(!result, "Should return false when table doesn't exist");
}

#[test]
fn test_check_ast_table_exists_present() {
    // Create database with ast_nodes table
    let conn = Connection::open_in_memory().unwrap();
    conn.execute(ast_nodes_table_schema(), []).unwrap();

    let result = check_ast_table_exists(&conn).unwrap();
    assert!(result, "Should return true when table exists");
}

#[test]
fn test_check_ast_table_exists_with_other_tables() {
    // Create database with other tables but not ast_nodes
    let conn = Connection::open_in_memory().unwrap();
    conn.execute("CREATE TABLE other_table (id INTEGER PRIMARY KEY)", [])
        .unwrap();

    let result = check_ast_table_exists(&conn).unwrap();
    assert!(!result, "Should return false when only other tables exist");
}

#[test]
fn test_calculate_decision_depth() {
    use super::*;

    let conn = Connection::open_in_memory().unwrap();
    conn.execute(ast_nodes_table_schema(), []).unwrap();

    // Create a tree structure with decision points:
    // id=1: mod_item (parent_id=NULL) -> decision depth 0 (not a decision point)
    // id=2: function_item (parent_id=1) -> decision depth 0
    // id=3: if_expression (parent_id=2) -> decision depth 1
    // id=4: loop_expression (parent_id=3) -> decision depth 2
    // id=5: let_declaration (parent_id=4) -> decision depth 2 (not a decision point)
    // id=6: match_expression (parent_id=5) -> decision depth 3
    conn.execute(
        "INSERT INTO ast_nodes (id, parent_id, kind, byte_start, byte_end) VALUES
        (1, NULL, 'mod_item', 0, 1000),
        (2, 1, 'function_item', 100, 900),
        (3, 2, 'if_expression', 150, 800),
        (4, 3, 'loop_expression', 200, 700),
        (5, 4, 'let_declaration', 250, 600),
        (6, 5, 'match_expression', 300, 500)",
        [],
    )
    .unwrap();

    // Test decision depth calculation
    assert_eq!(
        calculate_decision_depth(&conn, 1).unwrap().unwrap(),
        0,
        "mod_item at root should have decision depth 0"
    );
    assert_eq!(
        calculate_decision_depth(&conn, 2).unwrap().unwrap(),
        0,
        "function_item (child of mod) should have decision depth 0"
    );
    assert_eq!(
        calculate_decision_depth(&conn, 3).unwrap().unwrap(),
        1,
        "if_expression should have decision depth 1"
    );
    assert_eq!(
        calculate_decision_depth(&conn, 4).unwrap().unwrap(),
        2,
        "loop_expression (child of if) should have decision depth 2"
    );
    assert_eq!(
        calculate_decision_depth(&conn, 5).unwrap().unwrap(),
        2,
        "let_declaration (child of loop) should have decision depth 2"
    );
    assert_eq!(
        calculate_decision_depth(&conn, 6).unwrap().unwrap(),
        3,
        "match_expression (child of let) should have decision depth 3"
    );
}
