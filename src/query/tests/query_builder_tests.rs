use super::*;
use std::path::PathBuf;

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

    assert!(sql.contains("s.name LIKE ? ESCAPE '\\'"));
    assert!(sql.contains("s.display_fqn LIKE ? ESCAPE '\\'"));
    assert!(sql.contains("s.fqn LIKE ? ESCAPE '\\'"));
    assert!(sql.contains("LIMIT ?"));
    assert_eq!(params.len(), 4);
    assert_eq!(count_params(&sql), 4);
}

#[test]
fn test_build_search_query_with_fts5() {
    use crate::query::builder::build_search_query;

    let (sql, params, _strategy) = build_search_query(
        "Mutex RwLock",
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
        false,
        &[],
        None,
        None,
        None,
        None,
        None,
        false,
        None,
        true,
    );

    assert!(sql.contains("symbol_fts MATCH ?"));
    assert!(!sql.contains("s.name LIKE ?"));
    assert_eq!(params.len(), 2);
    assert_eq!(count_params(&sql), 2);
}

#[test]
fn test_fts5_or_query_single_word() {
    use crate::query::builder::fts5_or_query;
    assert_eq!(fts5_or_query("test"), "\"test\"*");
}

#[test]
fn test_fts5_or_query_multi_word() {
    use crate::query::builder::fts5_or_query;
    assert_eq!(fts5_or_query("Mutex RwLock"), "\"Mutex\"* OR \"RwLock\"*");
}

#[test]
fn test_fts5_or_query_empty() {
    use crate::query::builder::fts5_or_query;
    assert_eq!(fts5_or_query(""), "");
    assert_eq!(fts5_or_query("   "), "");
}

#[test]
fn test_fts5_or_query_quotes() {
    use crate::query::builder::fts5_or_query;
    assert_eq!(fts5_or_query("foo\"bar"), "\"foo\"\"bar\"*");
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

    assert!(sql.contains("s.kind_normalized = ? OR s.kind = ?"));
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

    assert!(!sql.contains("LIKE ? ESCAPE '\\'"));
    assert!(sql.contains("LIMIT ?"));
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

    assert!(sql.starts_with("SELECT COUNT(*)"));
    assert!(!sql.contains("LIMIT"));
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

    assert!(sql.contains("ORDER BY"));
    assert!(sql.contains("LIMIT ?"));
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

    assert!(sql.contains("COALESCE(sm.fan_in, 0) DESC"));
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

    assert!(sql.contains("COALESCE(sm.fan_out, 0) DESC"));
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

    assert!(sql.contains("COALESCE(sm.cyclomatic_complexity, 0) DESC"));
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

    assert!(sql.contains("sm.cyclomatic_complexity >= ?"));
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

    assert!(sql.contains("sm.cyclomatic_complexity <= ?"));
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

    assert!(sql.contains("sm.fan_in >= ?"));
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

    assert!(sql.contains("LEFT JOIN symbol_metrics sm"));
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

    assert!(sql.contains("sm.cyclomatic_complexity >= ?"));
    assert!(sql.contains("sm.cyclomatic_complexity <= ?"));
    assert!(sql.contains("sm.fan_in >= ?"));
    assert_eq!(params.len(), 7);
    assert_eq!(count_params(&sql), 7);
}

#[test]
fn test_build_reference_query_basic() {
    let (sql, params) = build_reference_query("test", None, false, false, 100);

    assert!(sql.contains("r.kind = 'Reference'"));
    assert!(sql.contains("LEFT JOIN graph_edges e"));
    assert!(sql.contains("r.name LIKE ? ESCAPE '\\'"));
    assert!(sql.contains("LIMIT ?"));
    assert_eq!(params.len(), 2);
    assert_eq!(count_params(&sql), 2);
}

#[test]
fn test_build_reference_query_with_path_filter() {
    let path = PathBuf::from("/src/module");
    let (sql, params) = build_reference_query("test", Some(&path), false, false, 100);

    assert!(sql.contains("json_extract(r.data, '$.file') LIKE ? ESCAPE '\\'"));
    assert_eq!(params.len(), 3);
    assert_eq!(count_params(&sql), 3);
}

#[test]
fn test_build_reference_query_count_only() {
    let (sql, params) = build_reference_query("test", None, false, true, 0);

    assert!(sql.starts_with("SELECT COUNT(*)"));
    assert!(!sql.contains("LIMIT"));
    assert_eq!(params.len(), 1);
    assert_eq!(count_params(&sql), 1);
}

#[test]
fn test_build_call_query_basic() {
    let (sql, params) = build_call_query("test", None, false, false, 100);

    assert!(sql.contains("c.kind = 'Call'"));
    assert!(sql.contains("json_extract(c.data, '$.caller')"));
    assert!(sql.contains("json_extract(c.data, '$.callee')"));
    assert!(sql.contains("json_extract(c.data, '$.caller') LIKE ? ESCAPE '\\'"));
    assert!(sql.contains("json_extract(c.data, '$.callee') LIKE ? ESCAPE '\\'"));
    assert!(sql.contains("LIMIT ?"));
    assert_eq!(params.len(), 3);
    assert_eq!(count_params(&sql), 3);
}

#[test]
fn test_build_call_query_with_path_filter() {
    let path = PathBuf::from("/src/module");
    let (sql, params) = build_call_query("test", Some(&path), false, false, 100);

    assert!(sql.contains("json_extract(c.data, '$.file') LIKE ? ESCAPE '\\'"));
    assert_eq!(params.len(), 4);
    assert_eq!(count_params(&sql), 4);
}

#[test]
fn test_build_call_query_count_only() {
    let (sql, params) = build_call_query("test", None, false, true, 0);

    assert!(sql.starts_with("SELECT COUNT(*)"));
    assert!(!sql.contains("LIMIT"));
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
    assert_eq!(result, "/src/path\\%test%");
}

#[test]
fn test_like_prefix_with_underscore() {
    let path = PathBuf::from("/src/path_test");
    let result = like_prefix(&path);
    assert_eq!(result, "/src/path\\_test%");
}

#[test]
fn test_like_prefix_with_backslash() {
    let path = PathBuf::from("C:\\src\\path");
    let result = like_prefix(&path);
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

    assert!(sql.contains("s.name LIKE ? ESCAPE '\\'"));
    assert!(sql.contains("f.file_path LIKE ? ESCAPE '\\'"));
    assert!(sql.contains("s.kind_normalized = ? OR s.kind = ?"));
    assert_eq!(params.len(), 7);
    assert_eq!(count_params(&sql), 7);
}

#[test]
fn test_build_reference_query_regex_mode() {
    let (sql, params) = build_reference_query("test.*", None, true, false, 100);

    assert!(!sql.contains("LIKE ? ESCAPE '\\'"));
    assert!(sql.contains("LIMIT ?"));
    assert_eq!(params.len(), 1);
    assert_eq!(count_params(&sql), 1);
}

#[test]
fn test_build_call_query_regex_mode() {
    let (sql, params) = build_call_query("test.*", None, true, false, 100);

    assert!(!sql.contains("LIKE ? ESCAPE '\\'"));
    assert!(sql.contains("LIMIT ?"));
    assert_eq!(params.len(), 1);
    assert_eq!(count_params(&sql), 1);
}
