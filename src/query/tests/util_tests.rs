use super::*;
use crate::algorithm::AlgorithmOptions;
use crate::error::LlmError;
use std::collections::HashMap;

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
    file.write_all(b"line1\nline2\nline3")
        .expect("failed to execute SQL");

    let mut cache = HashMap::new();
    let path_str = temp_file
        .to_str()
        .expect("failed to convert path to string");

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
        let mut file = std::fs::File::create(&fake_db)
            .expect("search_symbols should handle corrupted database");
        file.write_all(b"This is not a SQLite database")
            .expect("failed to execute SQL");
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
        coverage_filter: None,
    });

    match result {
        Err(LlmError::DatabaseCorrupted { .. }) => {}
        Err(other) => panic!("Expected DatabaseCorrupted error, got: {:?}", other),
        Ok(_) => panic!("Expected error for corrupted database"),
    }

    std::fs::remove_file(&fake_db).ok();
}
