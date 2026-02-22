use super::*;
use crate::algorithm::AlgorithmOptions;
use crate::error::LlmError;
use crate::SortMode;
use regex::Regex;
use rusqlite::Connection;
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;

#[test]
fn test_score_match_empty_query() {
    // Empty query matches everything via starts_with (every string starts with "")
    // This is the current behavior - every name starts with empty string
    let score = score_match("", "any_name", "any_display_fqn", "any_fqn", None);
    assert_eq!(score, 80, "Empty query matches via name.starts_with('')");
}

#[test]
fn test_score_match_exact_name() {
    let score = score_match("foo", "foo", "", "", None);
    assert_eq!(score, 100, "Exact name match should return score 100");
}

#[test]
fn test_score_match_exact_display_fqn() {
    let score = score_match("foo", "", "foo", "", None);
    assert_eq!(score, 95, "Exact display_fqn match should return score 95");
}

#[test]
fn test_score_match_exact_fqn() {
    let score = score_match("foo", "", "", "foo", None);
    assert_eq!(score, 90, "Exact fqn match should return score 90");
}

#[test]
fn test_score_match_name_prefix() {
    let score = score_match("foo", "foobar", "", "", None);
    assert_eq!(score, 80, "Name prefix match should return score 80");
}

#[test]
fn test_score_match_display_fqn_prefix() {
    let score = score_match("foo", "", "foobar", "", None);
    assert_eq!(score, 70, "Display_fqn prefix match should return score 70");
}

#[test]
fn test_score_match_name_contains() {
    let score = score_match("foo", "barfoobar", "", "", None);
    assert_eq!(score, 60, "Name contains match should return score 60");
}

#[test]
fn test_score_match_display_fqn_contains() {
    let score = score_match("foo", "", "barfoobar", "", None);
    assert_eq!(
        score, 50,
        "Display_fqn contains match should return score 50"
    );
}

#[test]
fn test_score_match_fqn_contains() {
    let score = score_match("foo", "", "", "barfoobar", None);
    assert_eq!(score, 40, "Fqn contains match should return score 40");
}

#[test]
fn test_score_match_tie_handling() {
    // Same query against equivalent names should produce equal scores
    let score1 = score_match("test", "test_value", "", "", None);
    let score2 = score_match("test", "test_another", "", "", None);
    assert_eq!(
        score1, score2,
        "Equivalent matches should produce equal scores"
    );
}

#[test]
fn test_score_match_regex_name() {
    let regex = Regex::new("foo.*").ok();
    let score = score_match("foo.*", "foobar", "", "", regex.as_ref());
    assert_eq!(score, 70, "Regex match on name should return score 70");
}

#[test]
fn test_score_match_regex_display_fqn() {
    let regex = Regex::new("foo.*").ok();
    let score = score_match("foo.*", "", "foobar", "", regex.as_ref());
    assert_eq!(
        score, 60,
        "Regex match on display_fqn should return score 60"
    );
}

#[test]
fn test_score_match_regex_fqn() {
    let regex = Regex::new("foo.*").ok();
    let score = score_match("foo.*", "", "", "foobar", regex.as_ref());
    assert_eq!(score, 50, "Regex match on fqn should return score 50");
}

#[test]
fn test_score_match_boundary_max() {
    // Exact name match should cap at 100
    let score = score_match("test", "test", "test", "test", None);
    assert_eq!(score, 100, "Score should never exceed 100");
}

#[test]
fn test_score_match_no_match() {
    let score = score_match("xyz", "abc", "def", "ghi", None);
    assert_eq!(score, 0, "No match should return score 0");
}

#[test]
fn test_score_match_regex_no_match() {
    let regex = Regex::new("xyz.*").ok();
    let score = score_match("xyz.*", "abc", "def", "ghi", regex.as_ref());
    assert_eq!(score, 0, "Regex no match should return score 0");
}

#[test]
fn test_score_match_priority_exact_over_prefix() {
    // Exact match should take priority over prefix match
    let score = score_match("foo", "foo", "foobar", "", None);
    assert_eq!(
        score, 100,
        "Exact name match should take priority over prefix"
    );
}

#[test]
fn test_score_match_priority_prefix_over_contains() {
    // Prefix match should take priority over contains match
    let score = score_match("foo", "foobar", "barfoobar", "", None);
    assert_eq!(score, 80, "Prefix match should take priority over contains");
}

#[test]
fn test_score_match_multiple_matches_highest_score() {
    // When multiple matches exist, highest score should be returned
    let score = score_match("foo", "foo", "foobar", "barfoobar", None);
    assert_eq!(score, 100, "Should return highest score from all matches");
}

#[test]
fn test_score_match_case_sensitive() {
    // Matching should be case-sensitive
    let score1 = score_match("foo", "foo", "", "", None);
    let score2 = score_match("foo", "Foo", "", "", None);
    assert_eq!(score1, 100, "Exact case match should return 100");
    // "Foo" doesn't start with or contain "foo" (case-sensitive)
    assert_eq!(score2, 0, "Different case should not match");
}

#[test]
fn test_score_match_empty_name_field() {
    // Empty fields should be handled correctly
    let score = score_match("foo", "", "", "", None);
    assert_eq!(
        score, 0,
        "All empty fields with non-empty query should return 0"
    );
}

// Helper to count parameter placeholders in SQL
fn count_params(sql: &str) -> usize {
    sql.matches('?').count()
}

#[test]
fn test_build_search_query_basic() {
    let (sql, params, _strategy) = build_search_query(
        "test",
        None,
        None,
        None,
        false,
        false,
        100,
        MetricsOptions::default(),
        SortMode::default(),
        None,
        None,
        None,
        false, // has_ast_table
        &[],   // ast_kinds
        None,  // min_depth
        None,  // max_depth
        None,  // inside_kind
        None,  // contains_kind
        None,  // symbol_set_filter
    );

    // Should have LIKE clauses for name, display_fqn, fqn
    assert!(sql.contains("s.name LIKE ? ESCAPE '\\'"));
    assert!(sql.contains("s.display_fqn LIKE ? ESCAPE '\\'"));
    assert!(sql.contains("s.fqn LIKE ? ESCAPE '\\'"));

    // Should have LIMIT clause
    assert!(sql.contains("LIMIT ?"));

    // Should have 3 LIKE params + 1 LIMIT param
    assert_eq!(params.len(), 4);
    assert_eq!(count_params(&sql), 4);
}

#[test]
fn test_build_search_query_with_kind_filter() {
    let (sql, params, _strategy) = build_search_query(
        "test",
        None,
        Some("Function"),
        None,
        false,
        false,
        100,
        MetricsOptions::default(),
        SortMode::default(),
        None,
        None,
        None,
        false, // has_ast_table
        &[],   // ast_kinds
        None,  // min_depth
        None,  // max_depth
        None,  // inside_kind
        None,  // contains_kind
        None,  // symbol_set_filter
    );

    // Should add kind filter
    assert!(sql.contains("s.kind_normalized = ? OR s.kind = ?"));

    // Should have 3 LIKE params + 2 kind params + 1 LIMIT param
    assert_eq!(params.len(), 6);
    assert_eq!(count_params(&sql), 6);
}

#[test]
fn test_build_search_query_with_path_filter() {
    let path = PathBuf::from("/src/module");
    let (sql, params, _strategy) = build_search_query(
        "test",
        Some(&path),
        None,
        None,
        false,
        false,
        100,
        MetricsOptions::default(),
        SortMode::default(),
        None,
        None,
        None,
        false, // has_ast_table
        &[],   // ast_kinds
        None,  // min_depth
        None,  // max_depth
        None,  // inside_kind
        None,  // contains_kind
        None,  // symbol_set_filter
    );

    // Should add file path filter
    assert!(sql.contains("f.file_path LIKE ? ESCAPE '\\'"));

    // Should have 3 LIKE params + 1 path param + 1 LIMIT param
    assert_eq!(params.len(), 5);
    assert_eq!(count_params(&sql), 5);
}

#[test]
fn test_build_search_query_regex_mode() {
    let (sql, params, _strategy) = build_search_query(
        "test.*",
        None,
        None,
        None,
        true,
        false,
        100,
        MetricsOptions::default(),
        SortMode::default(),
        None,
        None,
        None,
        false, // has_ast_table
        &[],   // ast_kinds
        None,  // min_depth
        None,  // max_depth
        None,  // inside_kind
        None,  // contains_kind
        None,  // symbol_set_filter
    );

    // Should NOT have LIKE clauses in regex mode
    assert!(!sql.contains("LIKE ? ESCAPE '\\'"));

    // Should have LIMIT clause
    assert!(sql.contains("LIMIT ?"));

    // Should only have LIMIT param (no LIKE params)
    assert_eq!(params.len(), 1);
    assert_eq!(count_params(&sql), 1);
}

#[test]
fn test_build_search_query_count_only() {
    let (sql, params, _strategy) = build_search_query(
        "test",
        None,
        None,
        None,
        false,
        true,
        0,
        MetricsOptions::default(),
        SortMode::default(),
        None,
        None,
        None,
        false, // has_ast_table
        &[],   // ast_kinds
        None,  // min_depth
        None,  // max_depth
        None,  // inside_kind
        None,  // contains_kind
        None,  // symbol_set_filter
    );

    // Should start with COUNT
    assert!(sql.starts_with("SELECT COUNT(*)"));

    // Should NOT have LIMIT clause
    assert!(!sql.contains("LIMIT"));

    // Should have 3 LIKE params (no LIMIT param)
    assert_eq!(params.len(), 3);
    assert_eq!(count_params(&sql), 3);
}

#[test]
fn test_build_search_query_regular_query() {
    let (sql, params, _strategy) = build_search_query(
        "test",
        None,
        None,
        None,
        false,
        false,
        100,
        MetricsOptions::default(),
        SortMode::default(),
        None,
        None,
        None,
        false, // has_ast_table
        &[],   // ast_kinds
        None,  // min_depth
        None,  // max_depth
        None,  // inside_kind
        None,  // contains_kind
        None,  // symbol_set_filter
    );

    // Should have ORDER BY
    assert!(sql.contains("ORDER BY"));

    // Should have LIMIT clause
    assert!(sql.contains("LIMIT ?"));

    // Should have params
    assert!(!params.is_empty());
}

#[test]
fn test_build_search_query_with_metrics_fan_in_sort() {
    let (sql, params, _strategy) = build_search_query(
        "test",
        None,
        None,
        None,
        false,
        false,
        100,
        MetricsOptions::default(),
        SortMode::FanIn,
        None,
        None,
        None,
        false, // has_ast_table
        &[],   // ast_kinds
        None,  // min_depth
        None,  // max_depth
        None,  // inside_kind
        None,  // contains_kind
        None,  // symbol_set_filter
    );

    // Should ORDER BY fan_in DESC
    assert!(sql.contains("COALESCE(sm.fan_in, 0) DESC"));

    // Should have basic params
    assert!(!params.is_empty());
}

#[test]
fn test_build_search_query_with_metrics_fan_out_sort() {
    let (sql, params, _strategy) = build_search_query(
        "test",
        None,
        None,
        None,
        false,
        false,
        100,
        MetricsOptions::default(),
        SortMode::FanOut,
        None,
        None,
        None,
        false, // has_ast_table
        &[],   // ast_kinds
        None,  // min_depth
        None,  // max_depth
        None,  // inside_kind
        None,  // contains_kind
        None,  // symbol_set_filter
    );

    // Should ORDER BY fan_out DESC
    assert!(sql.contains("COALESCE(sm.fan_out, 0) DESC"));

    // Should have basic params
    assert!(!params.is_empty());
}

#[test]
fn test_build_search_query_with_metrics_complexity_sort() {
    let (sql, params, _strategy) = build_search_query(
        "test",
        None,
        None,
        None,
        false,
        false,
        100,
        MetricsOptions::default(),
        SortMode::Complexity,
        None,
        None,
        None,
        false, // has_ast_table
        &[],   // ast_kinds
        None,  // min_depth
        None,  // max_depth
        None,  // inside_kind
        None,  // contains_kind
        None,  // symbol_set_filter
    );

    // Should ORDER BY cyclomatic_complexity DESC
    assert!(sql.contains("COALESCE(sm.cyclomatic_complexity, 0) DESC"));

    // Should have basic params
    assert!(!params.is_empty());
}

#[test]
fn test_build_search_query_with_min_complexity_filter() {
    let metrics = MetricsOptions {
        min_complexity: Some(5),
        ..Default::default()
    };
    let (sql, params, _strategy) = build_search_query(
        "test",
        None,
        None,
        None,
        false,
        false,
        100,
        metrics,
        SortMode::default(),
        None,
        None,
        None,
        false, // has_ast_table
        &[],   // ast_kinds
        None,  // min_depth
        None,  // max_depth
        None,  // inside_kind
        None,  // contains_kind
        None,  // symbol_set_filter
    );

    // Should filter by min_complexity
    assert!(sql.contains("sm.cyclomatic_complexity >= ?"));

    // Should have 3 LIKE params + 1 filter param + 1 LIMIT param
    assert_eq!(params.len(), 5);
    assert_eq!(count_params(&sql), 5);
}

#[test]
fn test_build_search_query_with_max_complexity_filter() {
    let metrics = MetricsOptions {
        max_complexity: Some(20),
        ..Default::default()
    };
    let (sql, params, _strategy) = build_search_query(
        "test",
        None,
        None,
        None,
        false,
        false,
        100,
        metrics,
        SortMode::default(),
        None,
        None,
        None,
        false, // has_ast_table
        &[],   // ast_kinds
        None,  // min_depth
        None,  // max_depth
        None,  // inside_kind
        None,  // contains_kind
        None,  // symbol_set_filter
    );

    // Should filter by max_complexity
    assert!(sql.contains("sm.cyclomatic_complexity <= ?"));

    // Should have 3 LIKE params + 1 filter param + 1 LIMIT param
    assert_eq!(params.len(), 5);
    assert_eq!(count_params(&sql), 5);
}

#[test]
fn test_build_search_query_with_min_fan_in_filter() {
    let metrics = MetricsOptions {
        min_fan_in: Some(10),
        ..Default::default()
    };
    let (sql, params, _strategy) = build_search_query(
        "test",
        None,
        None,
        None,
        false,
        false,
        100,
        metrics,
        SortMode::default(),
        None,
        None,
        None,
        false, // has_ast_table
        &[],   // ast_kinds
        None,  // min_depth
        None,  // max_depth
        None,  // inside_kind
        None,  // contains_kind
        None,  // symbol_set_filter
    );

    // Should filter by min_fan_in
    assert!(sql.contains("sm.fan_in >= ?"));

    // Should have 3 LIKE params + 1 filter param + 1 LIMIT param
    assert_eq!(params.len(), 5);
    assert_eq!(count_params(&sql), 5);
}

#[test]
fn test_build_search_query_with_metrics_join() {
    let (sql, _, _) = build_search_query(
        "test",
        None,
        None,
        None,
        false,
        false,
        100,
        MetricsOptions::default(),
        SortMode::default(),
        None,
        None,
        None,
        false, // has_ast_table
        &[],   // ast_kinds
        None,  // min_depth
        None,  // max_depth
        None,  // inside_kind
        None,  // contains_kind
        None,  // symbol_set_filter
    );

    // Should LEFT JOIN symbol_metrics
    assert!(sql.contains("LEFT JOIN symbol_metrics sm"));

    // Should select metrics columns
    assert!(sql.contains("sm.fan_in, sm.fan_out, sm.cyclomatic_complexity"));
}

#[test]
fn test_build_search_query_combined_filters() {
    let metrics = MetricsOptions {
        min_complexity: Some(5),
        max_complexity: Some(20),
        min_fan_in: Some(10),
        ..Default::default()
    };
    let (sql, params, _strategy) = build_search_query(
        "test",
        None,
        None,
        None,
        false,
        false,
        100,
        metrics,
        SortMode::default(),
        None,
        None,
        None,
        false, // has_ast_table
        &[],   // ast_kinds
        None,  // min_depth
        None,  // max_depth
        None,  // inside_kind
        None,  // contains_kind
        None,  // symbol_set_filter
    );

    // Should have all filter clauses
    assert!(sql.contains("sm.cyclomatic_complexity >= ?"));
    assert!(sql.contains("sm.cyclomatic_complexity <= ?"));
    assert!(sql.contains("sm.fan_in >= ?"));

    // Should have 3 LIKE params + 3 filter params + 1 LIMIT param
    assert_eq!(params.len(), 7);
    assert_eq!(count_params(&sql), 7);
}

#[test]
fn test_build_reference_query_basic() {
    let (sql, params) = build_reference_query("test", None, false, false, 100);

    // Should have kind filter
    assert!(sql.contains("r.kind = 'Reference'"));

    // Should join with graph_edges
    assert!(sql.contains("LEFT JOIN graph_edges e"));

    // Should have LIKE clause
    assert!(sql.contains("r.name LIKE ? ESCAPE '\\'"));

    // Should have LIMIT clause
    assert!(sql.contains("LIMIT ?"));

    // Should have 1 LIKE param + 1 LIMIT param
    assert_eq!(params.len(), 2);
    assert_eq!(count_params(&sql), 2);
}

#[test]
fn test_build_reference_query_with_path_filter() {
    let path = PathBuf::from("/src/module");
    let (sql, params) = build_reference_query("test", Some(&path), false, false, 100);

    // Should add file path filter
    assert!(sql.contains("json_extract(r.data, '$.file') LIKE ? ESCAPE '\\'"));

    // Should have 1 LIKE param + 1 path param + 1 LIMIT param
    assert_eq!(params.len(), 3);
    assert_eq!(count_params(&sql), 3);
}

#[test]
fn test_build_reference_query_count_only() {
    let (sql, params) = build_reference_query("test", None, false, true, 0);

    // Should start with COUNT
    assert!(sql.starts_with("SELECT COUNT(*)"));

    // Should NOT have LIMIT clause
    assert!(!sql.contains("LIMIT"));

    // Should have 1 LIKE param (no LIMIT param)
    assert_eq!(params.len(), 1);
    assert_eq!(count_params(&sql), 1);
}

#[test]
fn test_build_call_query_basic() {
    let (sql, params) = build_call_query("test", None, false, false, 100);

    // Should have kind filter
    assert!(sql.contains("c.kind = 'Call'"));

    // Should have json_extract for caller/callee
    assert!(sql.contains("json_extract(c.data, '$.caller')"));
    assert!(sql.contains("json_extract(c.data, '$.callee')"));

    // Should have LIKE clauses for caller/callee
    assert!(sql.contains("json_extract(c.data, '$.caller') LIKE ? ESCAPE '\\'"));
    assert!(sql.contains("json_extract(c.data, '$.callee') LIKE ? ESCAPE '\\'"));

    // Should have LIMIT clause
    assert!(sql.contains("LIMIT ?"));

    // Should have 2 LIKE params + 1 LIMIT param
    assert_eq!(params.len(), 3);
    assert_eq!(count_params(&sql), 3);
}

#[test]
fn test_build_call_query_with_path_filter() {
    let path = PathBuf::from("/src/module");
    let (sql, params) = build_call_query("test", Some(&path), false, false, 100);

    // Should add file path filter
    assert!(sql.contains("json_extract(c.data, '$.file') LIKE ? ESCAPE '\\'"));

    // Should have 2 LIKE params + 1 path param + 1 LIMIT param
    assert_eq!(params.len(), 4);
    assert_eq!(count_params(&sql), 4);
}

#[test]
fn test_build_call_query_count_only() {
    let (sql, params) = build_call_query("test", None, false, true, 0);

    // Should start with COUNT
    assert!(sql.starts_with("SELECT COUNT(*)"));

    // Should NOT have LIMIT clause
    assert!(!sql.contains("LIMIT"));

    // Should have 2 LIKE params (no LIMIT param)
    assert_eq!(params.len(), 2);
    assert_eq!(count_params(&sql), 2);
}

#[test]
fn test_like_pattern_percent_escaping() {
    let result = like_pattern("test%value");
    assert_eq!(result, "%test\\%value%");
}

#[test]
fn test_like_pattern_underscore_escaping() {
    let result = like_pattern("test_value");
    assert_eq!(result, "%test\\_value%");
}

#[test]
fn test_like_pattern_backslash_escaping() {
    let result = like_pattern("test\\value");
    assert_eq!(result, "%test\\\\value%");
}

#[test]
fn test_like_pattern_multiple_special_chars() {
    let result = like_pattern("test%value_\\more");
    assert_eq!(result, "%test\\%value\\_\\\\more%");
}

#[test]
fn test_like_pattern_empty_string() {
    let result = like_pattern("");
    assert_eq!(result, "%%");
}

#[test]
fn test_like_prefix_path() {
    let path = PathBuf::from("/src/path");
    let result = like_prefix(&path);
    assert_eq!(result, "/src/path%");
}

#[test]
fn test_like_prefix_with_percent() {
    let path = PathBuf::from("/src/path%test");
    let result = like_prefix(&path);
    // Should escape the % in the path
    assert_eq!(result, "/src/path\\%test%");
}

#[test]
fn test_like_prefix_with_underscore() {
    let path = PathBuf::from("/src/path_test");
    let result = like_prefix(&path);
    // Should escape the _ in the path
    assert_eq!(result, "/src/path\\_test%");
}

#[test]
fn test_like_prefix_with_backslash() {
    let path = PathBuf::from("C:\\src\\path");
    let result = like_prefix(&path);
    // Should escape backslashes
    assert_eq!(result, "C:\\\\src\\\\path%");
}

#[test]
fn test_build_search_query_combined_filters_path_kind() {
    let path = PathBuf::from("/src/module");
    let (sql, params, _strategy) = build_search_query(
        "test",
        Some(&path),
        Some("Function"),
        None,
        false,
        false,
        100,
        MetricsOptions::default(),
        SortMode::default(),
        None,
        None,
        None,
        false, // has_ast_table
        &[],   // ast_kinds
        None,  // min_depth
        None,  // max_depth
        None,  // inside_kind
        None,  // contains_kind
        None,  // symbol_set_filter
    );

    // Should have all filters
    assert!(sql.contains("s.name LIKE ? ESCAPE '\\'"));
    assert!(sql.contains("f.file_path LIKE ? ESCAPE '\\'"));
    assert!(sql.contains("s.kind_normalized = ? OR s.kind = ?"));

    // Should have 3 LIKE params + 1 path param + 2 kind params + 1 LIMIT param
    assert_eq!(params.len(), 7);
    assert_eq!(count_params(&sql), 7);
}

#[test]
fn test_build_reference_query_regex_mode() {
    let (sql, params) = build_reference_query("test.*", None, true, false, 100);

    // Should NOT have LIKE clauses in regex mode
    assert!(!sql.contains("LIKE ? ESCAPE '\\'"));

    // Should have LIMIT clause
    assert!(sql.contains("LIMIT ?"));

    // Should only have LIMIT param (no LIKE params)
    assert_eq!(params.len(), 1);
    assert_eq!(count_params(&sql), 1);
}

#[test]
fn test_build_call_query_regex_mode() {
    let (sql, params) = build_call_query("test.*", None, true, false, 100);

    // Should NOT have LIKE clauses in regex mode
    assert!(!sql.contains("LIKE ? ESCAPE '\\'"));

    // Should have LIMIT clause
    assert!(sql.contains("LIMIT ?"));

    // Should only have LIMIT param (no LIKE params)
    assert_eq!(params.len(), 1);
    assert_eq!(count_params(&sql), 1);
}

// Helper to create a test database with sample data for search_symbols tests
fn create_test_db() -> (tempfile::NamedTempFile, Connection) {
    let db_file = tempfile::NamedTempFile::new().expect("failed to create temp file for test database");
    let conn = Connection::open(db_file.path()).expect("failed to open test database connection");

    // Create schema
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
    .expect("failed to create graph_edges table");
    // Create symbol_metrics table (required for LEFT JOIN in queries)
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
    .expect("failed to create symbol_metrics table");

    // Insert test File entity
    conn.execute(
        "INSERT INTO graph_entities (id, kind, data) VALUES (1, 'File', '{\"path\":\"/test/file.rs\"}')",
        [],
    ).expect("failed to insert test File entity");

    // Insert test Symbol entities
    conn.execute(
        "INSERT INTO graph_entities (id, kind, data) VALUES
            (10, 'Symbol', '{\"name\":\"test_func\",\"kind\":\"Function\",\"kind_normalized\":\"function\",\"display_fqn\":\"test_func\",\"fqn\":\"module::test_func\",\"canonical_fqn\":\"/test/file.rs::test_func\",\"symbol_id\":\"sym1\",\"byte_start\":100,\"byte_end\":200,\"start_line\":5,\"start_col\":0,\"end_line\":10,\"end_col\":1}'),
            (11, 'Symbol', '{\"name\":\"TestStruct\",\"kind\":\"Struct\",\"kind_normalized\":\"struct\",\"display_fqn\":\"TestStruct\",\"fqn\":\"module::TestStruct\",\"canonical_fqn\":\"/test/file.rs::TestStruct\",\"symbol_id\":\"sym2\",\"byte_start\":300,\"byte_end\":400,\"start_line\":15,\"start_col\":0,\"end_line\":20,\"end_col\":1}'),
            (12, 'Symbol', '{\"name\":\"helper\",\"kind\":\"Function\",\"kind_normalized\":\"function\",\"display_fqn\":\"helper\",\"fqn\":\"module::helper\",\"canonical_fqn\":\"/test/file.rs::helper\",\"symbol_id\":\"sym3\",\"byte_start\":500,\"byte_end\":600,\"start_line\":25,\"start_col\":0,\"end_line\":30,\"end_col\":1}')",
        [],
    ).expect("failed to insert test Symbol entities");

    // Insert DEFINES edges from File to Symbols
    conn.execute(
        "INSERT INTO graph_edges (from_id, to_id, edge_type) VALUES (1, 10, 'DEFINES'), (1, 11, 'DEFINES'), (1, 12, 'DEFINES')",
        [],
    ).expect("failed to insert test DEFINES edges");

    (db_file, conn)
}

// Public API tests for search_symbols()
mod pub_api_tests_symbols {
    use super::*;

    #[test]
    fn test_search_symbols_basic() {
        let (_db_file, _conn) = create_test_db();
        let db_path = _db_file.path();

        let options = SearchOptions {
            db_path,
            query: "test_func",
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
        };

        let (response, partial, _) = search_symbols(options).expect("search_symbols should succeed");
        assert!(!partial, "Should not be partial");
        assert_eq!(response.results.len(), 1, "Should find 1 result");
        assert_eq!(
            response.results[0].name, "test_func",
            "Should match test_func"
        );
    }

    #[test]
    fn test_search_symbols_empty_results() {
        let (_db_file, _conn) = create_test_db();
        let db_path = _db_file.path();

        let options = SearchOptions {
            db_path,
            query: "nonexistent",
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
        };

        let (response, partial, _) = search_symbols(options).expect("search_symbols should succeed");
        assert!(!partial, "Should not be partial");
        assert_eq!(response.results.len(), 0, "Should find 0 results");
    }

    #[test]
    fn test_search_symbols_prefix_match() {
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
            fqn_pattern: None,
            exact_fqn: None,
            language_filter: None,
        };

        let (response, partial, _) = search_symbols(options).expect("search_symbols should succeed");
        assert!(!partial, "Should not be partial");
        assert_eq!(response.results.len(), 2, "Should find 2 results");

        let names: Vec<&str> = response.results.iter().map(|r| r.name.as_str()).collect();
        assert!(names.contains(&"test_func"), "Should contain test_func");
        assert!(names.contains(&"TestStruct"), "Should contain TestStruct");
    }

    #[test]
    fn test_search_symbols_contains_match() {
        let (_db_file, _conn) = create_test_db();
        let db_path = _db_file.path();

        let options = SearchOptions {
            db_path,
            query: "helper",
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
        };

        let (response, partial, _) = search_symbols(options).expect("search_symbols should succeed");
        assert!(!partial, "Should not be partial");
        assert_eq!(response.results.len(), 1, "Should find 1 result");
        assert_eq!(response.results[0].name, "helper", "Should match helper");
    }

    #[test]
    fn test_search_symbols_kind_filter() {
        let (_db_file, _conn) = create_test_db();
        let db_path = _db_file.path();

        let options = SearchOptions {
            db_path,
            query: "test",
            path_filter: None,
            kind_filter: Some("Function"),
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
        };

        let (response, partial, _) = search_symbols(options).expect("search_symbols should succeed");
        assert!(!partial, "Should not be partial");
        assert_eq!(response.results.len(), 1, "Should find 1 Function result");
        assert_eq!(response.results[0].name, "test_func", "Should be test_func");
        assert_eq!(
            response.results[0].kind, "Function",
            "Should be Function kind"
        );
    }

    #[test]
    fn test_search_symbols_limit() {
        let (_db_file, _conn) = create_test_db();
        let db_path = _db_file.path();

        let options = SearchOptions {
            db_path,
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
        };

        let (response, partial, _) = search_symbols(options).expect("search_symbols should succeed");
        assert!(!partial, "Should not be partial");
        assert_eq!(
            response.results.len(),
            1,
            "Should return at most 1 result due to limit"
        );
    }

    #[test]
    fn test_search_symbols_regex_mode() {
        let (_db_file, _conn) = create_test_db();
        let db_path = _db_file.path();

        let options = SearchOptions {
            db_path,
            query: "test.*",
            path_filter: None,
            kind_filter: None,
            limit: 10,
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
        };

        let (response, partial, _) = search_symbols(options).expect("search_symbols should succeed");
        assert!(!partial, "Should not be partial");
        assert_eq!(
            response.results.len(),
            1,
            "Should find 1 result matching regex"
        );
        assert_eq!(response.results[0].name, "test_func", "Should be test_func");
    }

    #[test]
    fn test_search_symbols_regex_no_match() {
        let (_db_file, _conn) = create_test_db();
        let db_path = _db_file.path();

        let options = SearchOptions {
            db_path,
            query: "xyz.*",
            path_filter: None,
            kind_filter: None,
            limit: 10,
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
        };

        let (response, partial, _) = search_symbols(options).expect("search_symbols should succeed");
        assert!(!partial, "Should not be partial");
        assert_eq!(response.results.len(), 0, "Should find 0 results");
    }

    #[test]
    fn test_search_symbols_score_exact_match() {
        let (_db_file, _conn) = create_test_db();
        let db_path = _db_file.path();

        let options = SearchOptions {
            db_path,
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
        };

        let (response, partial, _) = search_symbols(options).expect("search_symbols should succeed");
        assert!(!partial, "Should not be partial");
        assert_eq!(response.results.len(), 1, "Should find 1 result");
        assert_eq!(
            response.results[0].score,
            Some(100),
            "Exact match should have score 100"
        );
    }

    #[test]
    fn test_search_symbols_score_prefix_match() {
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
        };

        let (response, partial, _) = search_symbols(options).expect("search_symbols should succeed");
        assert!(!partial, "Should not be partial");
        assert_eq!(response.results.len(), 2, "Should find 2 results");

        let test_func = response
            .results
            .iter()
            .find(|r| r.name == "test_func")
            .expect("test_func should be in results");
        assert_eq!(
            test_func.score,
            Some(80),
            "test_func should have prefix score 80"
        );

        let test_struct = response
            .results
            .iter()
            .find(|r| r.name == "TestStruct")
            .expect("TestStruct should be in results");
        assert_eq!(
            test_struct.score,
            Some(0),
            "TestStruct should have score 0 (case mismatch)"
        );
    }

    #[test]
    fn test_search_symbols_partial_result() {
        let (_db_file, _conn) = create_test_db();
        let db_path = _db_file.path();

        let options = SearchOptions {
            db_path,
            query: "test",
            path_filter: None,
            kind_filter: None,
            limit: 10,
            use_regex: false,
            candidates: 1,
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
        };

        let (response, partial, _) = search_symbols(options).expect("search_symbols should succeed");
        assert!(partial, "Should be partial since candidates < total count");
        assert_eq!(response.results.len(), 1, "Should return at most 1 result");
    }

    #[test]
    fn test_search_symbols_total_count() {
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
            fqn_pattern: None,
            exact_fqn: None,
            language_filter: None,
        };

        let (response, partial, _) = search_symbols(options).expect("search_symbols should succeed");
        assert!(!partial, "Should not be partial");
        assert_eq!(response.total_count, 2, "Total count should be 2");
    }

    #[test]
    fn test_search_symbols_ordering() {
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
        };

        let (response, partial, _) = search_symbols(options).expect("search_symbols should succeed");
        assert!(!partial, "Should not be partial");
        assert_eq!(response.results.len(), 2, "Should find 2 results");

        assert_eq!(
            response.results[0].name, "test_func",
            "test_func should be first (higher score)"
        );
        assert_eq!(
            response.results[0].score,
            Some(80),
            "test_func should have prefix score 80"
        );
        assert_eq!(
            response.results[1].name, "TestStruct",
            "TestStruct should be second"
        );
        assert_eq!(
            response.results[1].score,
            Some(0),
            "TestStruct should have score 0"
        );
    }

    #[test]
    fn test_search_symbols_include_score() {
        let (_db_file, _conn) = create_test_db();
        let db_path = _db_file.path();

        let options = SearchOptions {
            db_path,
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
        };

        let (response, partial, _) = search_symbols(options).expect("search_symbols should succeed");
        assert!(!partial, "Should not be partial");
        assert_eq!(response.results.len(), 1, "Should find 1 result");
        assert!(
            response.results[0].score.is_some(),
            "Score should be included"
        );
        assert_eq!(response.results[0].score, Some(100), "Score should be 100");
    }

    #[test]
    fn test_search_symbols_with_fqn() {
        let (_db_file, _conn) = create_test_db();
        let db_path = _db_file.path();

        let options = SearchOptions {
            db_path,
            query: "test_func",
            path_filter: None,
            kind_filter: None,
            limit: 10,
            use_regex: false,
            candidates: 100,
            context: ContextOptions::default(),
            snippet: SnippetOptions::default(),
            fqn: FqnOptions {
                fqn: true,
                canonical_fqn: false,
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
        };

        let (response, partial, _) = search_symbols(options).expect("search_symbols should succeed");
        assert!(!partial, "Should not be partial");
        assert_eq!(response.results.len(), 1, "Should find 1 result");
        assert_eq!(
            response.results[0].fqn,
            Some("module::test_func".to_string()),
            "FQN should be included"
        );
        assert!(
            response.results[0].display_fqn.is_none(),
            "display_fqn should not be included"
        );
    }
}
// Public API tests for search_calls()
mod pub_api_tests {
    use super::*;
    use rusqlite::Connection;
    use tempfile::NamedTempFile;

    fn create_test_db_with_calls() -> (NamedTempFile, Connection) {
        let db_file = NamedTempFile::new().expect("failed to create temp file");
        let conn = Connection::open(db_file.path()).expect("failed to open database");

        // Create schema
        conn.execute(
            "CREATE TABLE graph_entities (
                id INTEGER PRIMARY KEY,
                kind TEXT NOT NULL,
                data TEXT NOT NULL
            )",
            [],
        )
        .expect("failed to create graph_entities table");

        // Insert test Call entities
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
        };

        let (response, _partial) = search_calls(options).expect("search_calls should succeed");

        // Should find 2 calls where callee is "test_func"
        assert_eq!(response.results.len(), 2);
        assert_eq!(response.total_count, 2);
        assert_eq!(response.query, "test_func");

        // Both results should have callee "test_func"
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
        };

        let (response, _partial) = search_calls(options).expect("search_calls should succeed");

        // Should find 2 calls where caller is "main"
        assert_eq!(response.results.len(), 2);
        assert_eq!(response.total_count, 2);

        // Both results should have caller "main"
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
        };

        let (response, _partial) = search_calls(options).expect("search_calls should succeed");

        // Should find 0 results
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
        };

        let (response, _partial) = search_calls(options).expect("search_calls should succeed");

        // Should find 2 calls matching "test.*" pattern (callee is "test_func")
        assert_eq!(response.results.len(), 2);
        assert_eq!(response.total_count, 2);

        // Both should match because callee is "test_func" which matches "test.*"
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
        };

        let (response, _partial) = search_calls(options).expect("search_calls should succeed");

        // Should find 0 results - nothing matches "xyz.*"
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
        };

        let (response, _partial) = search_calls(options).expect("search_calls should succeed");

        // Should find results with scores
        assert!(!response.results.is_empty());

        // All results should have scores
        for result in &response.results {
            assert!(result.score.is_some());
            // Exact match on callee should give score 100
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
        };

        let (response, _partial) = search_calls(options).expect("search_calls should succeed");

        // Should find results with scores
        assert!(!response.results.is_empty());

        // All results should have scores
        for result in &response.results {
            assert!(result.score.is_some());
            // Exact match on caller should give score 100
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
        };

        let (response, _partial) = search_calls(options).expect("search_calls should succeed");

        // Should return only 1 result due to limit
        assert_eq!(response.results.len(), 1);
        // But total_count should reflect all matches
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
        };

        let (response, _partial) = search_calls(options).expect("search_calls should succeed");

        // total_count should accurately reflect all matching results
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
        };

        let (response, _partial) = search_calls(options).expect("search_calls should succeed");

        // Should only find calls in /test/file.rs
        assert_eq!(response.results.len(), 1);
        assert_eq!(response.total_count, 1);

        // Result should be from the filtered path
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
        };

        let (response, _partial) = search_calls(options).expect("search_calls should succeed");

        // All results should include scores when include_score is true
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
        };

        let (response, _partial) = search_calls(options).expect("search_calls should succeed");

        // Results should be sorted by score (all same here), then by start_line
        if response.results.len() > 1 {
            for i in 1..response.results.len() {
                let prev = &response.results[i - 1];
                let curr = &response.results[i];
                // Scores should be non-increasing
                assert!(prev.score.expect("score should be Some") >= curr.score.expect("score should be Some"));
                // Within same score, sorted by start_line
                if prev.score == curr.score {
                    assert!(prev.span.start_line <= curr.span.start_line);
                }
            }
        }
    }

    // Helper function to create a test database with reference data
    fn create_test_db_with_references() -> (NamedTempFile, Connection) {
        let db_file = NamedTempFile::new().expect("failed to create temp file");
        let conn = Connection::open(db_file.path()).expect("failed to open database");

        // Create schema
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

        // Insert test Symbol entity
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

        // Insert test Reference entities
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

        // Insert REFERENCES edge
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
        };

        let (result, _partial) = search_references(options).expect("search_references should succeed");
        // Verify that results are sorted by score (descending)
        for i in 1..result.results.len() {
            let prev_score = result.results[i - 1].score.unwrap_or(0);
            let curr_score = result.results[i].score.unwrap_or(0);
            assert!(
                prev_score >= curr_score,
                "Results should be sorted by score descending"
            );
        }
    }
}

#[test]
fn test_load_file_returns_none_on_missing_file() {
    let mut cache = HashMap::new();
    let result = load_file("/nonexistent/path/to/file.rs", &mut cache);
    assert!(result.is_none());
    assert!(!cache.contains_key("/nonexistent/path/to/file.rs"));
}

#[test]
fn test_load_file_caches_successful_reads() {
    use std::io::Write;
    let temp_dir = std::env::temp_dir();
    let temp_file = temp_dir.join("llmgrep_test_load_file.txt");
    let mut file = std::fs::File::create(&temp_file).expect("failed to create temp file");
    file.write_all(b"line1\nline2\nline3").expect("failed to execute SQL");

    let mut cache = HashMap::new();
    let path_str = temp_file.to_str().expect("failed to convert path to string");

    let result1 = load_file(path_str, &mut cache);
    assert!(result1.is_some());
    assert_eq!(result1.expect("result should be Some").lines.len(), 3);

    let result2 = load_file(path_str, &mut cache);
    assert!(result2.is_some());
    assert_eq!(cache.len(), 1);

    std::fs::remove_file(&temp_file).ok();
}

#[test]
fn test_search_symbols_corrupted_database() {
    use std::io::Write;
    let temp_dir = std::env::temp_dir();
    let fake_db = temp_dir.join("llmgrep_test_corrupt.db");
    {
        let mut file = std::fs::File::create(&fake_db).expect("search_symbols should handle corrupted database");
        file.write_all(b"This is not a SQLite database").expect("failed to execute SQL");
    }

    let result = search_symbols(SearchOptions {
        db_path: &fake_db,
        query: "test",
        path_filter: None,
        kind_filter: None,
        limit: 10,
        use_regex: false,
        candidates: 50,
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
    });

    match result {
        Err(LlmError::DatabaseCorrupted { .. }) => {}
        Err(other) => panic!("Expected DatabaseCorrupted error, got: {:?}", other),
        Ok(_) => panic!("Expected error for corrupted database"),
    }

    std::fs::remove_file(&fake_db).ok();
}

// Chunk retrieval tests
mod chunk_tests {
    use super::*;
    use rusqlite::Connection;
    use tempfile::NamedTempFile;

    /// Create a test database with code_chunks table for chunk tests
    fn create_test_db_with_chunks() -> (NamedTempFile, Connection) {
        let db_file = NamedTempFile::new().expect("failed to create temp file");
        let conn = Connection::open(db_file.path()).expect("failed to open database");

        // Create code_chunks table
        conn.execute(
            "CREATE TABLE code_chunks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_path TEXT NOT NULL,
                byte_start INTEGER NOT NULL,
                byte_end INTEGER NOT NULL,
                content TEXT NOT NULL,
                content_hash TEXT NOT NULL,
                symbol_name TEXT,
                symbol_kind TEXT,
                created_at INTEGER NOT NULL
            )",
            [],
        )
        .expect("failed to execute SQL");

        // Insert test chunks
        // SHA-256 hash of "fn test_func() { }"
        let hash1 = "a0d2da8d1f8b1a8c8f8b1a8c8f8b1a8c8f8b1a8c8f8b1a8c8f8b1a8c8f8b1a8c";
        conn.execute(
            "INSERT INTO code_chunks (file_path, byte_start, byte_end, content, content_hash, symbol_name, symbol_kind, created_at) VALUES
                ('/test/file.rs', 100, 200, 'fn test_func() { }', ?, 'test_func', 'Function', 1700000000),
                ('/test/file.rs', 300, 400, 'struct TestStruct { }', 'b1e3eb9e2f9c2b9d9f9c2b9d9f9c2b9d9f9c2b9d9f9c2b9d9f9c2b9d9f9c2b9d', 'TestStruct', 'Struct', 1700000001),
                ('/test/other.rs', 500, 600, 'fn helper() { }', 'c2f4fc0f3g0d3c0e0g0d3c0e0g0d3c0e0g0d3c0e0g0d3c0e0g0d3c0e0g0d3c0e', 'helper', 'Function', 1700000002)",
            [hash1],
        ).expect("failed to execute SQL");

        (db_file, conn)
    }

    #[test]
    fn test_search_chunks_by_symbol_name() {
        let (_db_file, conn) = create_test_db_with_chunks();

        // Query for test_func symbol
        let chunks = search_chunks_by_symbol_name(&conn, "test_func").expect("failed to search chunks by symbol name");
        assert_eq!(chunks.len(), 1, "Should find 1 chunk for test_func");

        let chunk = &chunks[0];
        assert_eq!(chunk.file_path, "/test/file.rs");
        assert_eq!(chunk.byte_start, 100);
        assert_eq!(chunk.byte_end, 200);
        assert_eq!(chunk.content, "fn test_func() { }");
        assert_eq!(chunk.symbol_name, Some("test_func".to_string()));
        assert_eq!(chunk.symbol_kind, Some("Function".to_string()));
    }

    #[test]
    fn test_search_chunks_by_symbol_name_not_found() {
        let (_db_file, conn) = create_test_db_with_chunks();

        // Query for non-existent symbol
        let chunks = search_chunks_by_symbol_name(&conn, "nonexistent").expect("failed to search chunks by symbol name");
        assert_eq!(
            chunks.len(),
            0,
            "Should find 0 chunks for non-existent symbol"
        );
    }

    #[test]
    fn test_search_chunks_by_span() {
        let (_db_file, conn) = create_test_db_with_chunks();

        // Query for exact span
        let chunk = search_chunks_by_span(&conn, "/test/file.rs", 100, 200).expect("failed to search chunks by span");
        assert!(chunk.is_some(), "Should find chunk for exact span");

        let chunk = chunk.expect("chunk should be Some");
        assert_eq!(chunk.file_path, "/test/file.rs");
        assert_eq!(chunk.byte_start, 100);
        assert_eq!(chunk.byte_end, 200);
        assert_eq!(chunk.content, "fn test_func() { }");
        assert_eq!(chunk.symbol_name, Some("test_func".to_string()));
        assert_eq!(chunk.symbol_kind, Some("Function".to_string()));
    }

    #[test]
    fn test_search_chunks_by_span_not_found() {
        let (_db_file, conn) = create_test_db_with_chunks();

        // Query for non-existent span
        let chunk = search_chunks_by_span(&conn, "/test/file.rs", 999, 1000).expect("failed to search chunks by span");
        assert!(chunk.is_none(), "Should return None for non-existent span");

        // Query for non-existent file
        let chunk = search_chunks_by_span(&conn, "/test/nonexistent.rs", 100, 200).expect("failed to search chunks by span");
        assert!(chunk.is_none(), "Should return None for non-existent file");
    }

    #[test]
    fn test_search_chunks_by_span_wrong_byte_range() {
        let (_db_file, conn) = create_test_db_with_chunks();

        // Query with wrong byte_start
        let chunk = search_chunks_by_span(&conn, "/test/file.rs", 101, 200).expect("failed to search chunks by span");
        assert!(
            chunk.is_none(),
            "Should return None when byte_start doesn't match"
        );

        // Query with wrong byte_end
        let chunk = search_chunks_by_span(&conn, "/test/file.rs", 100, 201).expect("failed to search chunks by span");
        assert!(
            chunk.is_none(),
            "Should return None when byte_end doesn't match"
        );
    }

    #[test]
    fn test_content_hash_format() {
        let (_db_file, conn) = create_test_db_with_chunks();

        let chunks = search_chunks_by_symbol_name(&conn, "test_func").expect("failed to search chunks by symbol name");
        assert_eq!(chunks.len(), 1);

        let hash = &chunks[0].content_hash;
        assert_eq!(hash.len(), 64, "SHA-256 hash should be 64 hex characters");
        assert!(
            hash.chars().all(|c| c.is_ascii_hexdigit()),
            "Hash should contain only hex characters"
        );
    }

    #[test]
    fn test_symbol_kind_retrieval() {
        let (_db_file, conn) = create_test_db_with_chunks();

        // Test Function kind
        let chunks = search_chunks_by_symbol_name(&conn, "test_func").expect("failed to search chunks by symbol name");
        assert_eq!(chunks[0].symbol_kind, Some("Function".to_string()));

        // Test Struct kind
        let chunks = search_chunks_by_symbol_name(&conn, "TestStruct").expect("failed to search chunks by symbol name");
        assert_eq!(chunks[0].symbol_kind, Some("Struct".to_string()));
    }

    #[test]
    fn test_multiple_chunks_same_symbol() {
        let db_file = NamedTempFile::new().expect("failed to create temp file");
        let conn = Connection::open(db_file.path()).expect("failed to open database");

        // Create code_chunks table
        conn.execute(
            "CREATE TABLE code_chunks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_path TEXT NOT NULL,
                byte_start INTEGER NOT NULL,
                byte_end INTEGER NOT NULL,
                content TEXT NOT NULL,
                content_hash TEXT NOT NULL,
                symbol_name TEXT,
                symbol_kind TEXT,
                created_at INTEGER NOT NULL
            )",
            [],
        )
        .expect("failed to execute SQL");

        // Insert multiple chunks for the same symbol (e.g., different parts)
        conn.execute(
            "INSERT INTO code_chunks (file_path, byte_start, byte_end, content, content_hash, symbol_name, symbol_kind, created_at) VALUES
                ('/test/file.rs', 100, 150, 'part1', 'hash1', 'my_symbol', 'Function', 1700000000),
                ('/test/file.rs', 150, 200, 'part2', 'hash2', 'my_symbol', 'Function', 1700000001)",
            [],
        ).expect("failed to execute SQL");

        // Query should return all chunks for the symbol
        let chunks = search_chunks_by_symbol_name(&conn, "my_symbol").expect("failed to search chunks by symbol name");
        assert_eq!(chunks.len(), 2, "Should find 2 chunks for my_symbol");
    }
}

// Metrics filtering and sorting tests
mod metrics_tests {
    use super::*;

    fn create_test_db_with_metrics() -> (tempfile::NamedTempFile, Connection) {
        let db_file = tempfile::NamedTempFile::new().expect("failed to create temp file");
        let conn = Connection::open(db_file.path()).expect("failed to open database");

        // Create schema
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

        // Insert test File entity
        conn.execute(
            "INSERT INTO graph_entities (id, kind, data) VALUES (1, 'File', '{\"path\":\"/test/file.rs\"}')",
            [],
        ).expect("failed to execute SQL");

        // Insert test Symbol entities with varying metrics
        // sym1: complexity=5, fan_in=10, fan_out=2
        // sym2: complexity=15, fan_in=5, fan_out=8
        // sym3: complexity=25, fan_in=2, fan_out=15
        conn.execute(
            "INSERT INTO graph_entities (id, kind, data) VALUES
                (10, 'Symbol', '{\"name\":\"low_complexity\",\"kind\":\"Function\",\"kind_normalized\":\"function\",\"display_fqn\":\"low_complexity\",\"fqn\":\"module::low_complexity\",\"symbol_id\":\"sym1\",\"byte_start\":100,\"byte_end\":200,\"start_line\":5,\"start_col\":0,\"end_line\":10,\"end_col\":1}'),
                (11, 'Symbol', '{\"name\":\"med_complexity\",\"kind\":\"Function\",\"kind_normalized\":\"function\",\"display_fqn\":\"med_complexity\",\"fqn\":\"module::med_complexity\",\"symbol_id\":\"sym2\",\"byte_start\":300,\"byte_end\":400,\"start_line\":15,\"start_col\":0,\"end_line\":20,\"end_col\":1}'),
                (12, 'Symbol', '{\"name\":\"high_complexity\",\"kind\":\"Function\",\"kind_normalized\":\"function\",\"display_fqn\":\"high_complexity\",\"fqn\":\"module::high_complexity\",\"symbol_id\":\"sym3\",\"byte_start\":500,\"byte_end\":600,\"start_line\":25,\"start_col\":0,\"end_line\":30,\"end_col\":1}')",
            [],
        ).expect("failed to execute SQL");

        // Insert DEFINES edges from File to Symbols
        conn.execute(
            "INSERT INTO graph_edges (from_id, to_id, edge_type) VALUES (1, 10, 'DEFINES'), (1, 11, 'DEFINES'), (1, 12, 'DEFINES')",
            [],
        ).expect("failed to execute SQL");

        // Insert metrics - symbol_id now references graph_entities.id (INTEGER)
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
        };

        let (response, partial, _) = search_symbols(options).expect("search_symbols should succeed");
        assert!(!partial, "Should not be partial");
        // Should find med_complexity (15) and high_complexity (25), but not low_complexity (5)
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
        };

        let (response, partial, _) = search_symbols(options).expect("search_symbols should succeed");
        assert!(!partial, "Should not be partial");
        // Should find only low_complexity (5), not med (15) or high (25)
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
        };

        let (response, partial, _) = search_symbols(options).expect("search_symbols should succeed");
        assert!(!partial, "Should not be partial");
        // Should find only med_complexity (15), not low (5) or high (25)
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
        };

        let (response, partial, _) = search_symbols(options).expect("search_symbols should succeed");
        assert!(!partial, "Should not be partial");
        // Should find only low_complexity (fan_in=10)
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
        };

        let (response, partial, _) = search_symbols(options).expect("search_symbols should succeed");
        assert!(!partial, "Should not be partial");
        // Should find only high_complexity (fan_out=15)
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
        };

        let (response, partial, _) = search_symbols(options).expect("search_symbols should succeed");
        assert!(!partial, "Should not be partial");
        assert_eq!(response.results.len(), 3, "Should find all 3 results");

        // Should be sorted by fan_in DESC: low_complexity (10), med_complexity (5), high_complexity (2)
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
        };

        let (response, partial, _) = search_symbols(options).expect("search_symbols should succeed");
        assert!(!partial, "Should not be partial");
        assert_eq!(response.results.len(), 3, "Should find all 3 results");

        // Should be sorted by fan_out DESC: high_complexity (15), med_complexity (8), low_complexity (2)
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
        };

        let (response, partial, _) = search_symbols(options).expect("search_symbols should succeed");
        assert!(!partial, "Should not be partial");
        assert_eq!(response.results.len(), 3, "Should find all 3 results");

        // Should be sorted by cyclomatic_complexity DESC: high_complexity (25), med_complexity (15), low_complexity (5)
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
        };

        let (response, partial, _) = search_symbols(options).expect("search_symbols should succeed");
        assert!(!partial, "Should not be partial");
        assert_eq!(response.results.len(), 1);

        let result = &response.results[0];
        assert_eq!(result.name, "low_complexity");
        // Verify metrics fields are populated
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
        // Create a DB where some symbols have metrics and some don't
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

        // Insert 3 symbols: only sym1 has metrics
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

        // Only sym1 has metrics - symbol_id now references graph_entities.id (INTEGER)
        conn.execute(
            "INSERT INTO symbol_metrics (symbol_id, symbol_name, kind, file_path, loc, estimated_loc, fan_in, fan_out, cyclomatic_complexity, last_updated) VALUES
                (10, 'with_metrics', 'Function', '/test/file.rs', 50, 0.0, 10, 2, 5, 0)",
            [],
        ).expect("failed to execute SQL");

        let db_path = db_file.path();

        // Test without filter: all symbols should appear
        let options = SearchOptions {
            db_path,
            query: "", // Empty query matches all
            path_filter: None,
            kind_filter: None,
            limit: 10,
            use_regex: false,
            candidates: 100,
            context: ContextOptions::default(),
            snippet: SnippetOptions::default(),
            fqn: FqnOptions::default(),
            include_score: false,
            sort_by: SortMode::FanIn, // Sort by fan_in
            metrics: MetricsOptions::default(),
            ast: AstOptions::default(),
            depth: DepthOptions::default(),
            algorithm: AlgorithmOptions::default(),
            symbol_id: None,
            fqn_pattern: None,
            exact_fqn: None,
            language_filter: None,
        };

        let (response, _partial, _) = search_symbols(options).expect("search_symbols should succeed");
        assert_eq!(response.results.len(), 3, "Should find all 3 symbols");

        // Symbols without metrics should have None for metrics fields
        // and appear last in sorted results (COALESCE to 0)
        let with_metrics = response
            .results
            .iter()
            .find(|r| r.name == "with_metrics").expect("result should be found");
        assert_eq!(
            with_metrics.fan_in,
            Some(10),
            "Symbol with metrics should have fan_in"
        );

        let no_metrics_1 = response
            .results
            .iter()
            .find(|r| r.name == "no_metrics_1").expect("result should be found");
        assert_eq!(
            no_metrics_1.fan_in, None,
            "Symbol without metrics should have None for fan_in"
        );

        let no_metrics_2 = response
            .results
            .iter()
            .find(|r| r.name == "no_metrics_2").expect("result should be found");
        assert_eq!(
            no_metrics_2.fan_in, None,
            "Symbol without metrics should have None for fan_in"
        );

        // With filter: only symbols with metrics matching filter should appear
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
        };

        let (response_filter, _, _) = search_symbols(options_filter).expect("search_symbols with filter should succeed");
        assert_eq!(
            response_filter.results.len(),
            1,
            "Should find only 1 symbol with fan_in >= 5"
        );
        assert_eq!(response_filter.results[0].name, "with_metrics");
    }
}

// Tests for SymbolId lookup and ambiguity detection
mod symbol_id_tests {
    use super::*;

    #[test]
    fn test_symbol_id_lookup_returns_single_result() {
        let (_db_file, _conn) = create_test_db();
        let db_path = _db_file.path();

        // Lookup by exact symbol_id
        let options = SearchOptions {
            db_path,
            query: "unused", // Query is ignored when symbol_id is provided
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

        // Filter by FQN pattern
        let options = SearchOptions {
            db_path,
            query: "test", // Query still applies
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
        };

        let (response, _partial, _) = search_symbols(options).expect("search_symbols should succeed");
        // All test symbols are in /test/file.rs
        assert!(
            !response.results.is_empty(),
            "Should find symbols matching FQN pattern"
        );
    }

    #[test]
    fn test_exact_fqn_filter() {
        let (_db_file, _conn) = create_test_db();
        let db_path = _db_file.path();

        // Filter by exact FQN
        let options = SearchOptions {
            db_path,
            query: "", // Empty query with exact_fqn should work
            path_filter: None,
            kind_filter: None,
            limit: 10,
            use_regex: false,
            candidates: 100,
            context: ContextOptions::default(),
            snippet: SnippetOptions::default(),
            fqn: FqnOptions {
                fqn: false,
                canonical_fqn: true, // Enable to see canonical_fqn in results
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
        };

        let (response, _partial, _) = search_symbols(options).expect("search_symbols should succeed");
        // All test symbols have symbol_id
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
        // Create a database with duplicate symbol names
        let db_file = tempfile::NamedTempFile::new().expect("failed to create temp file");
        let conn = Connection::open(db_file.path()).expect("failed to open database");

        // Create schema
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

        // Insert file
        conn.execute(
            "INSERT INTO graph_entities (id, kind, data) VALUES (1, 'File', '{\"path\":\"/test/a.rs\"}')",
            [],
        ).expect("failed to execute SQL");
        conn.execute(
            "INSERT INTO graph_entities (id, kind, data) VALUES (2, 'File', '{\"path\":\"/test/b.rs\"}')",
            [],
        ).expect("failed to execute SQL");

        // Insert two symbols with same name "parse" in different modules
        conn.execute(
            "INSERT INTO graph_entities (id, kind, data) VALUES
                (10, 'Symbol', '{\"name\":\"parse\",\"kind\":\"Function\",\"display_fqn\":\"parse\",\"fqn\":\"a::parse\",\"canonical_fqn\":\"/test/a.rs::parse\",\"symbol_id\":\"parse_a\",\"byte_start\":100,\"byte_end\":200,\"start_line\":5,\"start_col\":0,\"end_line\":10,\"end_col\":1}'),
                (11, 'Symbol', '{\"name\":\"parse\",\"kind\":\"Function\",\"display_fqn\":\"parse\",\"fqn\":\"b::parse\",\"canonical_fqn\":\"/test/b.rs::parse\",\"symbol_id\":\"parse_b\",\"byte_start\":300,\"byte_end\":400,\"start_line\":15,\"start_col\":0,\"end_line\":20,\"end_col\":1}')",
            [],
        ).expect("failed to execute SQL");

        // Insert edges
        conn.execute(
            "INSERT INTO graph_edges (from_id, to_id, edge_type) VALUES (1, 10, 'DEFINES'), (2, 11, 'DEFINES')",
            [],
        ).expect("failed to execute SQL");

        let db_path = db_file.path();

        // Query for "parse" - should trigger ambiguity warning
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
                canonical_fqn: true, // Enable to see canonical_fqn in results
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
        };

        // Capture stderr to check for warning
        let (response, _partial, _) = search_symbols(options).expect("search_symbols should succeed");
        // Should find both symbols
        assert_eq!(
            response.results.len(),
            2,
            "Should find both 'parse' symbols"
        );
        // Both should have different canonical_fqns
        let fqns: Vec<_> = response
            .results
            .iter()
            .filter_map(|r| r.canonical_fqn.as_ref())
            .collect();
        assert_eq!(fqns.len(), 2, "Should have 2 different FQNs");
    }

    #[test]
    fn test_symbol_id_bypasses_ambiguity() {
        // Create a database with duplicate symbol names
        let db_file = tempfile::NamedTempFile::new().expect("failed to create temp file");
        let conn = Connection::open(db_file.path()).expect("failed to open database");

        // Create schema
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

        // Insert file
        conn.execute(
            "INSERT INTO graph_entities (id, kind, data) VALUES (1, 'File', '{\"path\":\"/test/a.rs\"}')",
            [],
        ).expect("failed to execute SQL");

        // Insert two symbols with same name "parse"
        conn.execute(
            "INSERT INTO graph_entities (id, kind, data) VALUES
                (10, 'Symbol', '{\"name\":\"parse\",\"kind\":\"Function\",\"display_fqn\":\"parse\",\"fqn\":\"a::parse\",\"canonical_fqn\":\"/test/a.rs::parse\",\"symbol_id\":\"target_parse\",\"byte_start\":100,\"byte_end\":200,\"start_line\":5,\"start_col\":0,\"end_line\":10,\"end_col\":1}'),
                (11, 'Symbol', '{\"name\":\"parse\",\"kind\":\"Function\",\"display_fqn\":\"parse\",\"fqn\":\"b::parse\",\"canonical_fqn\":\"/test/b.rs::parse\",\"symbol_id\":\"other_parse\",\"byte_start\":300,\"byte_end\":400,\"start_line\":15,\"start_col\":0,\"end_line\":20,\"end_col\":1}')",
            [],
        ).expect("failed to execute SQL");

        // Insert edges
        conn.execute(
            "INSERT INTO graph_edges (from_id, to_id, edge_type) VALUES (1, 10, 'DEFINES')",
            [],
        )
        .expect("failed to execute SQL");

        let db_path = db_file.path();

        // Use symbol_id to get exact match - no ambiguity
        let options = SearchOptions {
            db_path,
            query: "ignored", // Query is ignored when symbol_id is provided
            path_filter: None,
            kind_filter: None,
            limit: 10,
            use_regex: false,
            candidates: 100,
            context: ContextOptions::default(),
            snippet: SnippetOptions::default(),
            fqn: FqnOptions {
                fqn: false,
                canonical_fqn: true, // Enable to see canonical_fqn in results
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
        // Test common language extensions
        assert_eq!(infer_language("src/main.rs"), Some("Rust"));
        assert_eq!(infer_language("lib/app.py"), Some("Python"));
        assert_eq!(infer_language("component.js"), Some("JavaScript"));
        assert_eq!(infer_language("module.ts"), Some("TypeScript"));
        assert_eq!(infer_language("header.h"), Some("C"));
        assert_eq!(infer_language("impl.cpp"), Some("C++"));
        assert_eq!(infer_language("Main.java"), Some("Java"));
        assert_eq!(infer_language("main.go"), Some("Go"));

        // Test JSX/TSX variants
        assert_eq!(infer_language("App.jsx"), Some("JavaScript"));
        assert_eq!(infer_language("App.tsx"), Some("TypeScript"));

        // Test unknown extensions
        assert_eq!(infer_language("file.xyz"), None);
        assert_eq!(infer_language("README"), None);
        assert_eq!(infer_language("no_extension"), None);
    }

    #[test]
    fn test_normalize_kind_label() {
        // Test that kind normalization lowercases the kind
        assert_eq!(normalize_kind_label("Function"), "function");
        assert_eq!(normalize_kind_label("STRUCT"), "struct");
        assert_eq!(normalize_kind_label("Method"), "method");
        assert_eq!(normalize_kind_label("Class"), "class");
        assert_eq!(normalize_kind_label("enum"), "enum");
    }

    #[test]
    fn test_build_search_query_with_language_filter() {
        // Test that language filter adds file extension filter
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
            false, // has_ast_table
            &[],   // ast_kinds
            None,  // min_depth
            None,  // max_depth
            None,  // inside_kind
            None,  // contains_kind
        None,  // symbol_set_filter
    );

        // Should filter by .rs extension
        assert!(sql.contains("f.file_path LIKE ? ESCAPE '\\'"));

        // Should have 5 params: 3 LIKE params + 1 language file param + 1 LIMIT
        assert_eq!(params.len(), 5);
    }

    #[test]
    fn test_build_search_query_with_unknown_language() {
        // Test that unknown language doesn't add filter
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
            false, // has_ast_table
            &[],   // ast_kinds
            None,  // min_depth
            None,  // max_depth
            None,  // inside_kind
            None,  // contains_kind
        None,  // symbol_set_filter
    );

        // Should NOT add language filter for unknown language
        // Should have 4 params: 3 LIKE + 1 LIMIT (no extra language param)
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
            false, // has_ast_table
            &[],   // ast_kinds
            None,  // min_depth
            None,  // max_depth
            None,  // inside_kind
            None,  // contains_kind
        None,  // symbol_set_filter
    );

        // Should have both path, kind, and language filters
        assert!(sql.contains("f.file_path LIKE ? ESCAPE '\\'"));
        assert!(sql.contains("s.kind_normalized = ? OR s.kind = ?"));

        // Should have 8 params: 3 LIKE + 1 path + 2 kind + 1 language + 1 LIMIT
        assert_eq!(params.len(), 8);
        assert_eq!(count_params(&sql), 8);
    }

    #[test]
    fn test_build_search_query_with_cpp_language() {
        // Test C++ language alias handling
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
            false, // has_ast_table
            &[],   // ast_kinds
            None,  // min_depth
            None,  // max_depth
            None,  // inside_kind
            None,  // contains_kind
        None,  // symbol_set_filter
    );

        // Should filter by .cpp extension
        assert!(sql.contains("f.file_path LIKE ? ESCAPE '\\'"));

        // Should have 5 params: 3 LIKE + 1 language file + 1 LIMIT
        assert_eq!(params.len(), 5);
    }
}
