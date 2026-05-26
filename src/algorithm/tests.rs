use super::*;

#[test]
fn test_symbol_set_validation_valid() {
    let symbol_set = SymbolSet {
        symbol_ids: vec![
            "abc123def456789012345678901234ab".to_string(),
            "0123456789abcdef0123456789abcdef".to_string(),
            "ffffffffffffffffffffffffffffffff".to_string(),
        ],
    };
    assert!(symbol_set.validate().is_ok());
    assert_eq!(symbol_set.len(), 3);
    assert!(!symbol_set.is_empty());
}

#[test]
fn test_symbol_set_validation_invalid_length() {
    let symbol_set = SymbolSet {
        symbol_ids: vec!["abc123".to_string()],
    };
    assert!(symbol_set.validate().is_err());
}

#[test]
fn test_symbol_set_validation_invalid_chars() {
    let symbol_set = SymbolSet {
        symbol_ids: vec!["abc123def456789012345678901234g!".to_string()],
    };
    assert!(symbol_set.validate().is_err());
}

#[test]
fn test_symbol_set_empty() {
    let symbol_set = SymbolSet { symbol_ids: vec![] };
    assert!(symbol_set.validate().is_ok());
    assert_eq!(symbol_set.len(), 0);
    assert!(symbol_set.is_empty());
}

#[test]
fn test_symbol_set_json_deserialize() {
    let json = r#"{"symbol_ids": ["abc123def456789012345678901234ab"]}"#;
    let symbol_set: SymbolSet = serde_json::from_str(json).unwrap();
    assert_eq!(symbol_set.symbol_ids.len(), 1);
    assert_eq!(symbol_set.symbol_ids[0], "abc123def456789012345678901234ab");
}

#[test]
fn test_symbol_set_json_serialize() {
    let symbol_set = SymbolSet {
        symbol_ids: vec!["abc123def456789012345678901234ab".to_string()],
    };
    let json = serde_json::to_string(&symbol_set).unwrap();
    assert!(json.contains("symbol_ids"));
    assert!(json.contains("abc123def456789012345678901234ab"));
}
