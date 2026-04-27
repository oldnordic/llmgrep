//! Algorithm integration tests
//!
//! Tests for Magellan algorithm integration:
//! - SymbolSet parsing and validation
//! - Algorithm filter execution (requires magellan CLI)
//! - FQN resolution (requires magellan CLI)
//! - Temporary table optimization
//! - Error handling

use llmgrep::algorithm::{
    parse_symbol_set_file, symbol_set_filter_strategy, AlgorithmOptions, SymbolSetStrategy,
};
use llmgrep::error::LlmError;
use std::fs::File;
use std::io::Write;
use tempfile::TempDir;

// ============================================================================
// SymbolSet Parsing Tests
// ============================================================================

#[test]
fn test_symbol_set_parsing_valid() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let symbol_set_path = temp_dir.path().join("symbols.json");

    let valid_json = r#"{
        "symbol_ids": [
            "abc123def456789012345678901234ab",
            "0123456789abcdef0123456789abcdef",
            "ffffffffffffffffffffffffffffffff"
        ]
    }"#;

    let mut file = File::create(&symbol_set_path).expect("failed to create symbol set file");
    file.write_all(valid_json.as_bytes())
        .expect("failed to write valid JSON");

    let symbol_set = parse_symbol_set_file(&symbol_set_path).expect("failed to parse symbol set");
    assert_eq!(symbol_set.symbol_ids.len(), 3);
    assert_eq!(symbol_set.symbol_ids[0], "abc123def456789012345678901234ab");
}

#[test]
fn test_symbol_set_parsing_invalid_length() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let symbol_set_path = temp_dir.path().join("symbols.json");

    let invalid_json = r#"{
        "symbol_ids": ["abc123"]
    }"#;

    let mut file = File::create(&symbol_set_path).expect("failed to create symbol set file");
    file.write_all(invalid_json.as_bytes())
        .expect("failed to write valid JSON");

    let result = parse_symbol_set_file(&symbol_set_path);
    assert!(result.is_err());
    match result.unwrap_err() {
        LlmError::InvalidQuery { query } => {
            assert!(query.contains("Invalid SymbolId format"));
        }
        _ => panic!("Expected InvalidQuery error"),
    }
}

#[test]
fn test_symbol_set_parsing_invalid_chars() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let symbol_set_path = temp_dir.path().join("symbols.json");

    let invalid_json = r#"{
        "symbol_ids": ["abc123def456789012345678901234g!"]
    }"#;

    let mut file = File::create(&symbol_set_path).expect("failed to create symbol set file");
    file.write_all(invalid_json.as_bytes())
        .expect("failed to write valid JSON");

    let result = parse_symbol_set_file(&symbol_set_path);
    assert!(result.is_err());
    match result.unwrap_err() {
        LlmError::InvalidQuery { query } => {
            assert!(query.contains("Invalid SymbolId format"));
        }
        _ => panic!("Expected InvalidQuery error"),
    }
}

#[test]
fn test_symbol_set_parsing_empty_array() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let symbol_set_path = temp_dir.path().join("symbols.json");

    let empty_json = r#"{"symbol_ids": []}"#;

    let mut file = File::create(&symbol_set_path).expect("failed to create symbol set file");
    file.write_all(empty_json.as_bytes())
        .expect("failed to write empty JSON");

    let symbol_set = parse_symbol_set_file(&symbol_set_path).expect("failed to parse symbol set");
    assert!(symbol_set.symbol_ids.is_empty());
    assert!(symbol_set.is_empty());
    assert_eq!(symbol_set.len(), 0);
}

#[test]
fn test_symbol_set_parsing_missing_field() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let symbol_set_path = temp_dir.path().join("symbols.json");

    let invalid_json = r#"{"wrong_field": []}"#;

    let mut file = File::create(&symbol_set_path).expect("failed to create symbol set file");
    file.write_all(invalid_json.as_bytes())
        .expect("failed to write valid JSON");

    let result = parse_symbol_set_file(&symbol_set_path);
    assert!(result.is_err());
    match result.unwrap_err() {
        LlmError::JsonError(_) => {
            // Expected
        }
        _ => panic!("Expected JsonError"),
    }
}

// ============================================================================
// Temporary Table Optimization Tests
// ============================================================================

#[test]
fn test_temp_table_threshold_empty() {
    let symbol_ids: Vec<String> = vec![];
    let strategy = symbol_set_filter_strategy(&symbol_ids);
    assert_eq!(strategy, SymbolSetStrategy::None);
}

#[test]
fn test_temp_table_threshold_small() {
    let symbol_ids = vec!["abc123def456789012345678901234ab".to_string(); 100];
    let strategy = symbol_set_filter_strategy(&symbol_ids);
    assert_eq!(strategy, SymbolSetStrategy::InClause);
}

#[test]
fn test_temp_table_threshold_at_boundary() {
    let symbol_ids = vec!["abc123def456789012345678901234ab".to_string(); 1000];
    let strategy = symbol_set_filter_strategy(&symbol_ids);
    assert_eq!(strategy, SymbolSetStrategy::InClause);
}

#[test]
fn test_temp_table_threshold_over_boundary() {
    let symbol_ids = vec!["abc123def456789012345678901234ab".to_string(); 1001];
    let strategy = symbol_set_filter_strategy(&symbol_ids);
    assert_eq!(strategy, SymbolSetStrategy::TempTable);
}

#[test]
fn test_temp_table_threshold_large() {
    let symbol_ids = vec!["abc123def456789012345678901234ab".to_string(); 5000];
    let strategy = symbol_set_filter_strategy(&symbol_ids);
    assert_eq!(strategy, SymbolSetStrategy::TempTable);
}

// ============================================================================
// AlgorithmOptions Tests
// ============================================================================

#[test]
fn test_algorithm_options_default() {
    let options = AlgorithmOptions::default();
    assert!(options.from_symbol_set.is_none());
    assert!(options.reachable_from.is_none());
    assert!(options.dead_code_in.is_none());
    assert!(options.in_cycle.is_none());
    assert!(options.slice_backward_from.is_none());
    assert!(options.slice_forward_from.is_none());
    assert!(!options.is_active());
}

#[test]
fn test_algorithm_options_from_symbol_set_active() {
    let options = AlgorithmOptions {
        from_symbol_set: Some("test.json"),
        ..Default::default()
    };
    assert!(options.is_active());
}

#[test]
fn test_algorithm_options_reachable_from_active() {
    let options = AlgorithmOptions {
        reachable_from: Some("main"),
        ..Default::default()
    };
    assert!(options.is_active());
}

#[test]
fn test_algorithm_options_dead_code_in_active() {
    let options = AlgorithmOptions {
        dead_code_in: Some("main"),
        ..Default::default()
    };
    assert!(options.is_active());
}

#[test]
fn test_algorithm_options_in_cycle_active() {
    let options = AlgorithmOptions {
        in_cycle: Some("process"),
        ..Default::default()
    };
    assert!(options.is_active());
}

#[test]
fn test_algorithm_options_slice_backward_from_active() {
    let options = AlgorithmOptions {
        slice_backward_from: Some("handler"),
        ..Default::default()
    };
    assert!(options.is_active());
}

#[test]
fn test_algorithm_options_slice_forward_from_active() {
    let options = AlgorithmOptions {
        slice_forward_from: Some("main"),
        ..Default::default()
    };
    assert!(options.is_active());
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[test]
fn test_error_magellan_not_found() {
    // This test would require mocking or a specific environment setup
    // For now, we just verify the error exists
    let error = LlmError::MagellanNotFound;
    assert_eq!(error.error_code(), "LLM-E105");
    assert_eq!(error.severity(), "error");
}

#[test]
fn test_error_ambiguous_symbol_name() {
    let error = LlmError::AmbiguousSymbolName {
        name: "foo".to_string(),
        count: 3,
    };
    assert_eq!(error.error_code(), "LLM-E106");
    assert_eq!(error.severity(), "error");
}

#[test]
fn test_error_magellan_version_mismatch() {
    let error = LlmError::MagellanVersionMismatch {
        current: "1.0.0".to_string(),
        required: "2.0.0".to_string(),
    };
    assert_eq!(error.error_code(), "LLM-E107");
    assert_eq!(error.severity(), "error");
}

#[test]
fn test_error_magellan_execution_failed() {
    let error = LlmError::MagellanExecutionFailed {
        algorithm: "reachable".to_string(),
        stderr: "database not found".to_string(),
    };
    assert_eq!(error.error_code(), "LLM-E108");
    assert_eq!(error.severity(), "error");
}
